//! Find the `colima` binary. GUI apps get a minimal PATH, so fixed fallbacks are checked too.
use std::path::{Path, PathBuf};

/// Locate `colima`. `get_env` reads an environment variable; `exists` tests a file path.
/// Search order: each PATH entry, then /opt/homebrew/bin, /usr/local/bin, ~/.local/bin,
/// /home/linuxbrew/.linuxbrew/bin.
pub fn locate_colima(
    _get_env: impl Fn(&str) -> Option<String>,
    _exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    todo!("RED: locate_colima")
}
