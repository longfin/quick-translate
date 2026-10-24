//! Finds the `claude` / `codex` CLIs. Port of the Swift app's `CLILocator.swift`.
//!
//! Apps started from a desktop launcher (or at login) get a minimal `PATH`, so we probe the usual
//! install locations of each platform first and only then ask the system (`where.exe` on Windows,
//! a login shell's `command -v` elsewhere). A manual path from Settings always wins.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

static CACHE: Mutex<Option<HashMap<String, PathBuf>>> = Mutex::new(None);

#[cfg(windows)]
pub const PATH_SEPARATOR: &str = ";";
#[cfg(not(windows))]
pub const PATH_SEPARATOR: &str = ":";

/// File extensions tried in each candidate directory. Windows: native binaries and npm/nvm shims
/// (`claude.cmd`); the extension-less sh shims npm also writes there are not runnable.
#[cfg(windows)]
const EXTENSIONS: &[&str] = &["exe", "cmd", "bat"];
#[cfg(not(windows))]
const EXTENSIONS: &[&str] = &[""];

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

pub fn home() -> PathBuf {
    #[cfg(windows)]
    let h = env_path("USERPROFILE");
    #[cfg(not(windows))]
    let h = env_path("HOME");
    h.unwrap_or_default()
}

/// `~` / `~/x` → home-relative; anything else unchanged.
pub fn expand(path: &str) -> PathBuf {
    if path == "~" {
        return home();
    }
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        return home().join(rest);
    }
    PathBuf::from(path)
}

/// `<base>/<v>/<suffix>` for every version directory under `base`, newest first (numeric compare,
/// so `v22.10.0` sorts above `v22.9.0`).
fn versioned_dirs(base: &Path, suffix: &str) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(base) else {
        return vec![];
    };
    let mut versions: Vec<(Vec<u64>, PathBuf)> = rd
        .flatten()
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let key = name
                .trim_start_matches('v')
                .split(|c: char| !c.is_ascii_digit())
                .filter_map(|p| p.parse().ok())
                .collect();
            (key, e.path())
        })
        .collect();
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    versions
        .into_iter()
        .map(|(_, p)| if suffix.is_empty() { p } else { p.join(suffix) })
        .collect()
}

/// Install locations, most specific first.
#[cfg(windows)]
pub fn candidate_dirs() -> Vec<PathBuf> {
    let home = home();
    let appdata = env_path("APPDATA").unwrap_or_else(|| home.join("AppData\\Roaming"));
    let local = env_path("LOCALAPPDATA").unwrap_or_else(|| home.join("AppData\\Local"));
    let mut dirs = vec![
        // Native installer (`irm https://claude.ai/install.ps1 | iex`).
        home.join(".local\\bin"),
        // `npm i -g` with the default prefix.
        appdata.join("npm"),
        local.join("Programs\\claude"),
        local.join("Programs\\claude\\bin"),
    ];
    // Volta shims.
    dirs.push(
        env_path("VOLTA_HOME")
            .unwrap_or_else(|| local.join("Volta"))
            .join("bin"),
    );
    // fnm: globals live in the node prefix of the default alias.
    if let Some(fnm) = env_path("FNM_DIR") {
        dirs.push(fnm.join("aliases\\default"));
    }
    dirs.push(appdata.join("fnm\\aliases\\default"));
    dirs.push(local.join("fnm\\aliases\\default"));
    // nvm-windows: the active version is symlinked at NVM_SYMLINK (default C:\Program Files\nodejs).
    if let Some(link) = env_path("NVM_SYMLINK") {
        dirs.push(link);
    }
    if let Some(pf) = env_path("ProgramFiles") {
        dirs.push(pf.join("nodejs"));
    }
    if let Some(nvm) = env_path("NVM_HOME") {
        dirs.extend(versioned_dirs(&nvm, ""));
    }
    // bun and scoop shims.
    dirs.push(
        env_path("BUN_INSTALL")
            .unwrap_or_else(|| home.join(".bun"))
            .join("bin"),
    );
    dirs.push(
        env_path("SCOOP")
            .unwrap_or_else(|| home.join("scoop"))
            .join("shims"),
    );
    dirs
}

#[cfg(target_os = "linux")]
pub fn candidate_dirs() -> Vec<PathBuf> {
    let home = home();
    let mut dirs: Vec<PathBuf> = [
        ".local/bin",
        ".npm-global/bin",
        ".npm/bin",
        ".claude/local",
        ".claude/local/node_modules/.bin",
        ".volta/bin",
        ".bun/bin",
    ]
    .iter()
    .map(|d| home.join(d))
    .collect();
    // fnm default alias (XDG location, legacy ~/.fnm, or FNM_DIR).
    if let Some(fnm) = env_path("FNM_DIR") {
        dirs.push(fnm.join("aliases/default/bin"));
    }
    let xdg_data = env_path("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"));
    dirs.push(xdg_data.join("fnm/aliases/default/bin"));
    dirs.push(home.join(".fnm/aliases/default/bin"));
    // nvm: every installed version, newest first.
    let nvm = env_path("NVM_DIR").unwrap_or_else(|| home.join(".nvm"));
    dirs.extend(versioned_dirs(&nvm.join("versions/node"), "bin"));
    dirs.extend([home.join(".yarn/bin"), home.join(".cargo/bin")]);
    dirs.extend(["/usr/local/bin", "/usr/bin", "/bin", "/snap/bin"].map(PathBuf::from));
    dirs
}

/// macOS: same list as the Swift app had.
#[cfg(target_os = "macos")]
pub fn candidate_dirs() -> Vec<PathBuf> {
    let home = home();
    let mut dirs: Vec<PathBuf> = [
        ".local/bin",
        ".npm/bin",
        ".npm-global/bin",
        ".claude/local",
        ".claude/local/node_modules/.bin",
    ]
    .iter()
    .map(|d| home.join(d))
    .collect();
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    for d in [".volta/bin", ".bun/bin", ".cargo/bin", ".yarn/bin"] {
        dirs.push(home.join(d));
    }
    dirs.extend(versioned_dirs(&home.join(".nvm/versions/node"), "bin"));
    dirs.extend(["/usr/bin", "/bin"].map(PathBuf::from));
    dirs
}

pub fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        p.metadata()
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

/// Locates `name` (`claude` / `codex`). A non-blank `override_path` wins and is not cached; it
/// must point at an existing executable, otherwise the result is `None` (like the Swift app).
pub fn find(name: &str, override_path: &str) -> Option<PathBuf> {
    let manual = override_path.trim();
    if !manual.is_empty() {
        let p = expand(manual);
        return is_executable(&p).then_some(p);
    }
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some(p) = cache.get(name) {
        return Some(p.clone());
    }
    let found = candidate_dirs()
        .into_iter()
        .flat_map(|dir| {
            EXTENSIONS.iter().map(move |ext| {
                if ext.is_empty() {
                    dir.join(name)
                } else {
                    dir.join(format!("{name}.{ext}"))
                }
            })
        })
        .find(|p| is_executable(p))
        .or_else(|| system_lookup(name))?;
    cache.insert(name.to_string(), found.clone());
    Some(found)
}

pub fn reset_cache() {
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Runs a short helper command and returns its stdout, or `None` on failure / after `timeout`
/// (a login shell with a slow or interactive rc file must not hang the app).
pub fn quick_output(mut cmd: Command, timeout: Duration) -> Option<String> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(super::process_job::CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut s);
        s
    });
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    reader
        .join()
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

#[cfg(windows)]
fn system_lookup(name: &str) -> Option<PathBuf> {
    let mut cmd = Command::new("where.exe");
    cmd.arg(name);
    let out = quick_output(cmd, Duration::from_secs(5))?;
    out.lines().map(|l| PathBuf::from(l.trim())).find(|p| {
        let ext = p
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());
        matches!(ext.as_deref(), Some("exe" | "cmd" | "bat")) && p.is_file()
    })
}

#[cfg(not(windows))]
fn system_lookup(name: &str) -> Option<PathBuf> {
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    let shell = env_path("SHELL").unwrap_or_else(|| {
        if cfg!(target_os = "macos") {
            "/bin/zsh".into()
        } else {
            "/bin/sh".into()
        }
    });
    let mut cmd = Command::new(shell);
    cmd.args(["-lc", &format!("command -v {name}")]);
    let out = quick_output(cmd, Duration::from_secs(5))?;
    // rc files may print banners; the answer is the last line.
    let s = out.lines().map(str::trim).rfind(|l| !l.is_empty())?;
    s.starts_with('/')
        .then(|| PathBuf::from(s))
        .filter(|p| is_executable(p))
}

/// `PATH` for child processes: the candidate directories, then the inherited `PATH`. `extra` (the
/// directory of the resolved executable) goes first so a shim finds its own `node`.
pub fn search_path(extra: Option<&Path>) -> OsString {
    let mut parts: Vec<OsString> = Vec::new();
    if let Some(e) = extra {
        parts.push(e.as_os_str().to_owned());
    }
    parts.extend(candidate_dirs().into_iter().map(PathBuf::into_os_string));
    match std::env::var_os("PATH") {
        Some(p) if !p.is_empty() => parts.push(p),
        _ if cfg!(not(windows)) => parts.push("/usr/bin:/bin".into()),
        _ => {}
    }
    let mut joined = OsString::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            joined.push(PATH_SEPARATOR);
        }
        joined.push(p);
    }
    joined
}

/// Environment overrides for child processes (on top of the inherited environment), like
/// `CLILocator.environment()`.
pub fn environment(exe_dir: Option<&Path>) -> Vec<(OsString, OsString)> {
    vec![
        ("PATH".into(), search_path(exe_dir)),
        ("NO_COLOR".into(), "1".into()),
        ("TERM".into(), "dumb".into()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde() {
        assert_eq!(expand("~"), home());
        assert_eq!(expand("~/x/claude"), home().join("x/claude"));
        assert_eq!(expand("/usr/bin/claude"), PathBuf::from("/usr/bin/claude"));
    }

    #[test]
    fn override_must_exist() {
        assert_eq!(find("claude", "/definitely/not/here/claude"), None);
    }

    #[test]
    fn versions_sort_numerically() {
        let dir = std::env::temp_dir().join(format!("qt-versions-{}", std::process::id()));
        for v in ["v9.1.0", "v22.9.0", "v22.10.0"] {
            std::fs::create_dir_all(dir.join(v)).unwrap();
        }
        let got: Vec<_> = versioned_dirs(&dir, "bin")
            .into_iter()
            .map(|p| {
                p.parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(got, ["v22.10.0", "v22.9.0", "v9.1.0"]);
    }
}
