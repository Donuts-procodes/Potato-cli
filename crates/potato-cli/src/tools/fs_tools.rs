use std::fs;
use std::path::Path;

use crate::types::ExecutionResult;

pub fn read_file(path_str: &str, start_line: usize, end_line: usize) -> ExecutionResult {
    let path = Path::new(path_str);
    if !path.exists() {
        return ExecutionResult::err(1, format!("File not found: {}", path_str));
    }

    match fs::read_to_string(path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let total_lines = lines.len();

            if total_lines == 0 {
                return ExecutionResult::ok(String::new());
            }

            let start_idx = if start_line == 0 { 0 } else { start_line.saturating_sub(1) };
            let end_idx = end_line.min(total_lines);

            if start_idx >= total_lines {
                return ExecutionResult::err(
                    1,
                    format!("start_line {} exceeds total line count {}", start_line, total_lines),
                );
            }

            let slice = &lines[start_idx..end_idx];
            let mut formatted = String::new();
            for (idx, line) in slice.iter().enumerate() {
                let current_line_num = start_idx + idx + 1;
                formatted.push_str(&format!("{:4} | {}\n", current_line_num, line));
            }

            ExecutionResult::ok(formatted)
        }
        Err(e) => ExecutionResult::err(1, format!("Failed to read file {}: {}", path_str, e)),
    }
}

pub fn write_file(path_str: &str, content: &str) -> ExecutionResult {
    let path = Path::new(path_str);

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = fs::create_dir_all(parent) {
                return ExecutionResult::err(
                    1,
                    format!("Failed to create parent directories for {}: {}", path_str, e),
                );
            }
        }
    }

    match fs::write(path, content) {
        Ok(_) => ExecutionResult::ok(format!("Successfully wrote {} bytes to {}", content.len(), path_str)),
        Err(e) => ExecutionResult::err(1, format!("Failed to write to file {}: {}", path_str, e)),
    }
}

pub fn apply_patch(path_str: &str, search: &str, replace: &str) -> ExecutionResult {
    let path = Path::new(path_str);
    if !path.exists() {
        return ExecutionResult::err(1, format!("File not found: {}", path_str));
    }

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return ExecutionResult::err(1, format!("Failed to read file {}: {}", path_str, e)),
    };

    let occurrences: Vec<_> = content.match_indices(search).collect();
    match occurrences.len() {
        0 => ExecutionResult::err(
            1,
            format!(
                "Search block not found in {}. Ensure exact whitespace and context match.",
                path_str
            ),
        ),
        1 => {
            let updated = content.replacen(search, replace, 1);
            match fs::write(path, updated) {
                Ok(_) => ExecutionResult::ok(format!("Successfully applied surgical patch to {}", path_str)),
                Err(e) => ExecutionResult::err(1, format!("Failed to save patched file {}: {}", path_str, e)),
            }
        }
        n => ExecutionResult::err(
            1,
            format!(
                "Search block matched {} times in {}. Search block must be unique; include 2-3 lines of surrounding context.",
                n, path_str
            ),
        ),
    }
}

pub fn list_dir(path_str: &str) -> ExecutionResult {
    let path = Path::new(path_str);
    if !path.exists() {
        return ExecutionResult::err(1, format!("Directory not found: {}", path_str));
    }

    match fs::read_dir(path) {
        Ok(entries) => {
            let mut list = Vec::new();
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                let file_type = entry.file_type();
                let is_dir = file_type.map(|t| t.is_dir()).unwrap_or(false);
                let suffix = if is_dir { "/" } else { "" };
                list.push(format!("{}{}", file_name, suffix));
            }
            list.sort();
            ExecutionResult::ok(list.join("\n"))
        }
        Err(e) => ExecutionResult::err(1, format!("Failed to list directory {}: {}", path_str, e)),
    }
}
