//! Append-only, timestamped log file. Mirrors `macos/Sources/QuickTranslate/Log.swift`:
//! one `yyyy-MM-dd HH:mm:ss.SSS message` line per call, also echoed to stderr.
//!
//! Location:
//! - Windows: `%LOCALAPPDATA%\QuickTranslate\QuickTranslate.log`
//! - Linux:   `$XDG_STATE_HOME/QuickTranslate/QuickTranslate.log` (default `~/.local/state/...`)
//! - macOS (dev builds only): `~/Library/Logs/QuickTranslate-desktop.log`, so it never mixes
//!   with the Swift app's `QuickTranslate.log`.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
static FILE: Mutex<Option<File>> = Mutex::new(None);

/// Where the log file lives on this platform, or `None` if the base directory is unknown.
pub fn path() -> Option<&'static PathBuf> {
    PATH.get_or_init(resolve_path).as_ref()
}

fn resolve_path() -> Option<PathBuf> {
    let env_dir = |key: &str| std::env::var_os(key).filter(|v| !v.is_empty()).map(PathBuf::from);

    #[cfg(target_os = "windows")]
    let path = env_dir("LOCALAPPDATA").map(|d| d.join("QuickTranslate").join("QuickTranslate.log"));

    #[cfg(target_os = "macos")]
    let path = env_dir("HOME").map(|h| h.join("Library/Logs/QuickTranslate-desktop.log"));

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let path = env_dir("XDG_STATE_HOME")
        .or_else(|| env_dir("HOME").map(|h| h.join(".local/state")))
        .map(|d| d.join("QuickTranslate").join("QuickTranslate.log"));

    path
}

fn open() -> Option<File> {
    let path = path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).ok()?;
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}

pub fn write(message: impl AsRef<str>) {
    let message = message.as_ref();
    eprintln!("[QuickTranslate] {message}");
    let line = format!("{} {message}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"));

    let mut guard = FILE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = open();
    }
    if let Some(file) = guard.as_mut() {
        if file.write_all(line.as_bytes()).is_err() {
            // The file may have been deleted or rotated; reopen on the next call.
            *guard = None;
        }
    }
}
