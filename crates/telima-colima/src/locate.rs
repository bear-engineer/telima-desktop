//! Find the `colima` binary. GUI apps get a minimal PATH, so fixed fallbacks are checked too.
use std::path::{Path, PathBuf};

const FALLBACK_DIRS: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin"];
const HOME_BIN: &str = ".local/bin";
const LINUXBREW_DIR: &str = "/home/linuxbrew/.linuxbrew/bin";

/// Locate `colima`. `get_env` reads an environment variable; `exists` tests a file path.
/// Search order: each PATH entry, then /opt/homebrew/bin, /usr/local/bin, ~/.local/bin,
/// /home/linuxbrew/.linuxbrew/bin.
pub fn locate_colima(
    get_env: impl Fn(&str) -> Option<String>,
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = get_env("PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .collect();
    dirs.extend(FALLBACK_DIRS.iter().map(PathBuf::from));
    if let Some(home) = get_env("HOME").filter(|h| !h.is_empty()) {
        dirs.push(Path::new(&home).join(HOME_BIN));
    }
    dirs.push(PathBuf::from(LINUXBREW_DIR));
    dirs.into_iter()
        .map(|d| d.join("colima"))
        .find(|candidate| exists(candidate))
}
