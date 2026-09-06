//! Headless CLI composition root. The production binary remains in src-tauri during migration.
pub fn crate_boundary() -> &'static str { "mikomai-core -> mikomai-cli" }
