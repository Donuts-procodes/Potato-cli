use std::path::{Path, PathBuf};
use tracing::info;

/// A soft sandbox that restricts all file operations to a declared root directory.
/// All relative paths are resolved against this root, and absolute paths are validated
/// to ensure they don't escape the sandbox boundary.
#[derive(Debug, Clone)]
pub struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    /// Creates a new sandbox rooted at the given directory.
    /// The root is canonicalized to resolve symlinks and `..' components.
    pub fn new(root: &Path) -> std::io::Result<Self> {
        let canonical = std::fs::canonicalize(root).or_else(|_| {
            // If directory doesn't exist yet, create it then canonicalize
            std::fs::create_dir_all(root)?;
            std::fs::canonicalize(root)
        })?;
        info!(root = %canonical.display(), "Sandbox initialized");
        Ok(Self { root: canonical })
    }

    /// Returns the sandbox root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves a user-provided path (relative or absolute) to a safe absolute path
    /// within the sandbox. Returns `Err` if the resolved path escapes the sandbox.
    pub fn resolve(&self, user_path: &str) -> Result<PathBuf, SandboxError> {
        let candidate = if Path::new(user_path).is_absolute() {
            PathBuf::from(user_path)
        } else {
            self.root.join(user_path)
        };

        // Normalize the path by resolving `.` and `..` components without requiring
        // the path to exist (unlike canonicalize which requires existence).
        let normalized = normalize_path(&candidate);

        // Check containment: the normalized path must start with the sandbox root
        if normalized.starts_with(&self.root) {
            Ok(normalized)
        } else {
            Err(SandboxError::EscapeAttempt {
                requested: user_path.to_string(),
                resolved: normalized.display().to_string(),
                root: self.root.display().to_string(),
            })
        }
    }

    /// Convenience: resolve and return as a String for tool dispatch.
    pub fn resolve_str(&self, user_path: &str) -> Result<String, SandboxError> {
        self.resolve(user_path).map(|p| p.display().to_string())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error(
        "Path escape blocked: \"{requested}\" resolves to \"{resolved}\" which is outside sandbox root \"{root}\""
    )]
    EscapeAttempt {
        requested: String,
        resolved: String,
        root: String,
    },
}

/// Normalizes a path by resolving `.` and `..` components lexically,
/// without touching the filesystem (unlike `std::fs::canonicalize`).
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                // Pop last component if possible (don't go above root)
                if !components.is_empty() {
                    components.pop();
                }
            }
            std::path::Component::CurDir => {
                // Skip `.` — no-op
            }
            other => {
                components.push(other);
            }
        }
    }
    components.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_relative_path_resolves_inside_sandbox() {
        let tmp = TempDir::new().unwrap();
        let sandbox = Sandbox::new(tmp.path()).unwrap();

        let result = sandbox.resolve("src/main.rs");
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert!(resolved.starts_with(sandbox.root()));
        assert!(resolved.ends_with("src/main.rs"));
    }

    #[test]
    fn test_parent_traversal_blocked() {
        let tmp = TempDir::new().unwrap();
        let sandbox = Sandbox::new(tmp.path()).unwrap();

        let result = sandbox.resolve("../../etc/passwd");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("escape blocked"));
    }

    #[test]
    fn test_absolute_path_inside_sandbox_allowed() {
        let tmp = TempDir::new().unwrap();
        let sandbox = Sandbox::new(tmp.path()).unwrap();

        let inside = sandbox.root().join("allowed.txt");
        let result = sandbox.resolve(&inside.display().to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_absolute_path_outside_sandbox_blocked() {
        let tmp = TempDir::new().unwrap();
        let sandbox = Sandbox::new(tmp.path()).unwrap();

        let result = sandbox.resolve("C:\\Windows\\System32\\cmd.exe");
        // This should fail unless tmp happens to be under C:\Windows
        // which it won't be
        assert!(result.is_err());
    }
}
