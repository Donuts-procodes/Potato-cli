use anyhow::{anyhow, Result};

/// Lightweight AST & syntax validator that guards filesystem writes.
/// Catches broken delimiters, unclosed brackets, and syntax malformations in microseconds.
pub struct AstValidator;

impl AstValidator {
    /// Validates raw file content against language-specific structural rules.
    pub fn validate(path: &str, content: &str) -> Result<()> {
        if path.ends_with(".rs") {
            Self::validate_rust(content)
        } else if path.ends_with(".json") {
            Self::validate_json(content)
        } else if path.ends_with(".toml") {
            Self::validate_toml(content)
        } else {
            // General bracket-matching heuristic for other languages
            Self::validate_balanced_delimiters(content)
        }
    }

    fn validate_rust(content: &str) -> Result<()> {
        Self::validate_balanced_delimiters(content)?;

        // Ensure functions have bodies or semicolons
        if content.contains("fn ") && !content.contains('{') && !content.contains(';') {
            return Err(anyhow!("Syntax error: Rust function definition without body or semicolon"));
        }
        Ok(())
    }

    fn validate_json(content: &str) -> Result<()> {
        serde_json::from_str::<serde_json::Value>(content)
            .map(|_| ())
            .map_err(|e| anyhow!("Invalid JSON syntax: {}", e))
    }

    fn validate_toml(content: &str) -> Result<()> {
        toml::from_str::<toml::Value>(content)
            .map(|_| ())
            .map_err(|e| anyhow!("Invalid TOML syntax: {}", e))
    }

    fn validate_balanced_delimiters(content: &str) -> Result<()> {
        let mut stack = Vec::new();
        let mut in_string = false;
        let mut string_char = '"';
        let mut escape_next = false;

        for (idx, ch) in content.chars().enumerate() {
            if escape_next {
                escape_next = false;
                continue;
            }
            if ch == '\\' {
                escape_next = true;
                continue;
            }
            if (ch == '"' || ch == '`') && (!in_string || string_char == ch) {
                in_string = !in_string;
                string_char = ch;
                continue;
            }
            if in_string {
                continue;
            }

            match ch {
                '{' | '(' | '[' => stack.push((ch, idx)),
                '}' => match stack.pop() {
                    Some(('{', _)) => {}
                    _ => return Err(anyhow!("Unmatched '}}' at character offset {}", idx)),
                },
                ')' => match stack.pop() {
                    Some(('(', _)) => {}
                    _ => return Err(anyhow!("Unmatched ')' at character offset {}", idx)),
                },
                ']' => match stack.pop() {
                    Some(('[', _)) => {}
                    _ => return Err(anyhow!("Unmatched ']' at character offset {}", idx)),
                },
                _ => {}
            }
        }

        if let Some((unclosed, idx)) = stack.pop() {
            return Err(anyhow!("Unclosed '{}' opened at character offset {}", unclosed, idx));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_json() {
        let content = r#"{"name": "potato", "version": 1}"#;
        assert!(AstValidator::validate("config.json", content).is_ok());
    }

    #[test]
    fn test_invalid_json() {
        let content = r#"{"name": "potato", "version": }"#;
        assert!(AstValidator::validate("config.json", content).is_err());
    }

    #[test]
    fn test_valid_rust() {
        let content = "fn main() { println!(\"hello\"); }";
        assert!(AstValidator::validate("src/main.rs", content).is_ok());
    }

    #[test]
    fn test_unbalanced_rust_delimiters() {
        let content = "fn main() { println!(\"hello\"); ";
        assert!(AstValidator::validate("src/main.rs", content).is_err());
    }

    #[test]
    fn test_valid_toml() {
        let content = "[package]\nname = \"potato\"\nversion = \"0.1.0\"\n";
        assert!(AstValidator::validate("Cargo.toml", content).is_ok());
    }
}
