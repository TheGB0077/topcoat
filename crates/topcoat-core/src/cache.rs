//! The user-level Topcoat cache.
//!
//! Downloads shared across projects live in a user-level cache
//! (`topcoat/<scope>` under the platform's cache directory), so several
//! workspaces reuse one copy of a release artifact. Directories are created
//! on demand by whoever writes into them.

use std::{env, path::PathBuf};

/// The user-level Topcoat cache directory for `scope`:
/// `<cache_root>/topcoat/<scope>`.
///
/// The cache root follows each platform's convention: `%LOCALAPPDATA%` on
/// Windows, `~/Library/Caches` on macOS, and `$XDG_CACHE_HOME` (defaulting
/// to `~/.cache`) elsewhere.
///
/// Stands in for
/// [`std::fs::HomeDirs::cache_home`](https://github.com/rust-lang/rust/issues/162082),
/// which tracks the same resolution; switch to it once it stabilizes.
///
/// Returns `None` when the platform variable yields no directory.
#[must_use]
pub fn user_cache_dir(scope: &str) -> Option<PathBuf> {
    Some(user_cache_root()?.join("topcoat").join(scope))
}

/// The platform cache directory root, or `None` when its variable yields no
/// directory.
#[cfg(windows)]
fn user_cache_root() -> Option<PathBuf> {
    env_dir("LOCALAPPDATA")
}

#[cfg(target_os = "macos")]
fn user_cache_root() -> Option<PathBuf> {
    Some(env_dir("HOME")?.join("Library").join("Caches"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn user_cache_root() -> Option<PathBuf> {
    env_dir("XDG_CACHE_HOME")
        .filter(|dir| dir.is_absolute())
        .or_else(|| env_dir("HOME").map(|home| home.join(".cache")))
}

/// The value of `var` as a directory path, when set and non-empty.
fn env_dir(var: &str) -> Option<PathBuf> {
    env::var_os(var)
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}
