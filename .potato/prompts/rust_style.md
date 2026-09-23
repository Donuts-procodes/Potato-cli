# Rust Architecture & Style Rules
- Always use `thiserror` for library error types and `anyhow` for application binaries.
- Avoid `.unwrap()` or `.expect()` in production paths; return structured `Result`.
- Ensure all public functions have doc comments.
