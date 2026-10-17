//! Child processes for the CLIs. Port of `macos/Sources/QuickTranslate/ProcessJob.swift`, plus the
//! process-tree handling the Swift app never needed.
//!
//! Every child gets piped UTF-8 stdio, the augmented `PATH` from `cli_locator`, and is started so
//! that cancelling kills its whole tree (the CLIs spawn helpers: `codex.js` runs the native codex
//! binary as a child, npm shims run `node`, ...):
//!
//! - **Unix**: the child calls `setsid()` before exec, so it leads a new session and process group
//!   (pgid = pid); cancel sends `SIGKILL` to the group.
//! - **Windows**: two layers.
//!   1. npm/nvm/fnm `*.cmd` shims are resolved and never run through `cmd.exe`: a shim whose target
//!      is a script (`codex.js`) becomes `node.exe <script>`, a shim whose target is a binary (the
//!      claude-code npm package ships `bin\claude.exe` since 2.x) becomes that binary, and scoop's
//!      `*.exe` + `*.shim` pairs become the target. This avoids `cmd.exe` argument re-quoting of the
//!      multi-line system prompt and keeps `--input-format stream-json` stdin a plain pipe instead
//!      of going through `cmd.exe` (the likeliest breakage per issue #3).
//!   2. Every child is also assigned to a Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, and
//!      cancel calls `TerminateJobObject`. Resolution alone is not enough: `codex.js` and the
//!      volta / bun shims (opaque `.exe` launchers) start the real CLI as a grandchild, and
//!      `TerminateProcess` on the parent would orphan it. If the job cannot be created or assigned,
//!      cancel falls back to `taskkill /T /F`.
//!
//!   All children get `CREATE_NO_WINDOW` so no console window flashes up.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

use super::cli_locator;
use crate::log;

/// `CREATE_NO_WINDOW` process creation flag (winbase.h).
#[cfg_attr(not(windows), allow(dead_code))]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// ---------------------------------------------------------------------------------------------
// Launch resolution (Windows shims)

/// What actually gets executed for a located CLI path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub program: PathBuf,
    /// Arguments placed before the caller's (the script path for `node.exe <script>`).
    pub prefix_args: Vec<OsString>,
}

impl Launch {
    fn direct(exe: &Path) -> Self {
        Self {
            program: exe.to_path_buf(),
            prefix_args: vec![],
        }
    }
}

/// Resolves `exe` to the program that should be spawned. Identity except on Windows (see the
/// module docs).
pub fn resolve_launch(exe: &Path) -> Launch {
    #[cfg(windows)]
    {
        let launch = resolve_windows(exe, 0);
        if launch.program != exe {
            log::write(format!(
                "resolved {} -> {} {:?}",
                exe.display(),
                launch.program.display(),
                launch.prefix_args
            ));
        }
        launch
    }
    #[cfg(not(windows))]
    Launch::direct(exe)
}

#[cfg(windows)]
fn resolve_windows(exe: &Path, depth: u32) -> Launch {
    let ext = exe
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let dir = exe.parent().unwrap_or(Path::new(""));
    if depth > 2 {
        return Launch::direct(exe);
    }
    match ext.as_str() {
        "cmd" | "bat" => {
            let Some(rel) = std::fs::read(exe)
                .ok()
                .and_then(|b| parse_cmd_shim(&String::from_utf8_lossy(&b)))
            else {
                return Launch::direct(exe);
            };
            let target = rel.iter().fold(dir.to_path_buf(), |p, c| p.join(c));
            if !target.is_file() {
                return Launch::direct(exe);
            }
            if is_script(&target) {
                let local_node = dir.join("node.exe");
                let node = if local_node.is_file() {
                    Some(local_node)
                } else {
                    find_on_path("node.exe")
                };
                match node {
                    Some(node) => Launch {
                        program: node,
                        prefix_args: vec![target.into_os_string()],
                    },
                    None => Launch::direct(exe),
                }
            } else {
                resolve_windows(&target, depth + 1)
            }
        }
        "exe" => {
            // scoop: `shims\claude.exe` is a generic launcher configured by `shims\claude.shim`.
            let shim = exe.with_extension("shim");
            match std::fs::read_to_string(&shim)
                .ok()
                .and_then(|s| parse_scoop_shim(&s))
            {
                Some(target) if target.is_file() => resolve_windows(&target, depth + 1),
                _ => Launch::direct(exe),
            }
        }
        _ => Launch::direct(exe),
    }
}

#[cfg(windows)]
fn is_script(p: &Path) -> bool {
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    matches!(ext.as_str(), "js" | "cjs" | "mjs")
}

#[cfg(windows)]
fn find_on_path(file: &str) -> Option<PathBuf> {
    std::env::split_paths(&cli_locator::search_path(None))
        .map(|d| d.join(file))
        .find(|p| p.is_file())
}

/// Extracts the target of an npm `cmd-shim` (relative to the shim's directory, as path
/// components). Handles both the current format (`"%dp0%\node_modules\...\cli.js" %*`) and the
/// older one (`"%~dp0\node_modules\..."`); `node.exe` references are skipped.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub fn parse_cmd_shim(content: &str) -> Option<Vec<String>> {
    content
        .split('"')
        .skip(1)
        .step_by(2) // the quoted segments
        .filter_map(|q| {
            let rest = q
                .strip_prefix("%dp0%")
                .or_else(|| q.strip_prefix("%~dp0"))?;
            let parts: Vec<String> = rest
                .split(['\\', '/'])
                .filter(|c| !c.is_empty())
                .map(str::to_string)
                .collect();
            let last = parts.last()?.to_ascii_lowercase();
            (last != "node.exe" && last != "node").then_some(parts)
        })
        .last()
}

/// Extracts `path = "..."` from a scoop `.shim` file.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub fn parse_scoop_shim(content: &str) -> Option<PathBuf> {
    content.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == "path").then(|| PathBuf::from(value.trim().trim_matches('"')))
    })
}

// ---------------------------------------------------------------------------------------------
// Process tree

#[cfg(windows)]
mod job {
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    /// A Job Object that kills its processes when terminated or when the last handle closes.
    pub struct Job(HANDLE);

    // The handle is only used through thread-safe Win32 calls.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    impl Job {
        pub fn new() -> Option<Self> {
            unsafe {
                let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if h.is_null() {
                    return None;
                }
                let job = Job(h);
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                (ok != 0).then_some(job)
            }
        }

        pub fn assign(&self, process: HANDLE) -> bool {
            unsafe { AssignProcessToJobObject(self.0, process) != 0 }
        }

        pub fn terminate(&self) {
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

/// Kills a spawned child together with everything it started.
pub struct ProcessTree {
    pid: u32,
    #[cfg(windows)]
    job: Option<job::Job>,
}

impl ProcessTree {
    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn kill(&self) {
        if self.pid == 0 {
            return;
        }
        #[cfg(unix)]
        unsafe {
            // The child ran setsid(), so its pid is also its process group id.
            libc::kill(-(self.pid as libc::pid_t), libc::SIGKILL);
        }
        #[cfg(windows)]
        match &self.job {
            Some(job) => job.terminate(),
            None => {
                let mut cmd = std::process::Command::new("taskkill");
                cmd.args(["/T", "/F", "/PID", &self.pid.to_string()]);
                let _ = cli_locator::quick_output(cmd, std::time::Duration::from_secs(5));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Spawning

pub struct Spawned {
    pub child: Child,
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
    pub stderr: ChildStderr,
    pub tree: Arc<ProcessTree>,
}

/// Spawns `exe args…` with piped stdio in `cwd`, inheriting the environment plus
/// `cli_locator::environment()` and `extra_env`. Must be called inside a tokio runtime.
pub fn spawn(
    exe: &Path,
    args: &[String],
    extra_env: &[(&str, &str)],
    cwd: &Path,
) -> io::Result<Spawned> {
    let launch = resolve_launch(exe);
    let mut cmd = Command::new(&launch.program);
    cmd.args(&launch.prefix_args)
        .args(args)
        .envs(cli_locator::environment(exe.parent()))
        .envs(
            extra_env
                .iter()
                .map(|(k, v)| (OsStr::new(k), OsStr::new(v))),
        )
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Last resort if a handle is dropped without kill(): at least the direct child dies.
        .kill_on_drop(true);

    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd.spawn()?;
    let pid = child.id().unwrap_or(0);

    #[cfg(windows)]
    let job = {
        let job = job::Job::new();
        match (&job, child.raw_handle()) {
            (Some(j), Some(h)) if j.assign(h as _) => job,
            _ => {
                log::write(format!(
                    "pid={pid}: job object unavailable, cancel will use taskkill /T"
                ));
                None
            }
        }
    };

    let tree = Arc::new(ProcessTree {
        pid,
        #[cfg(windows)]
        job,
    });
    let stdin = child.stdin.take().expect("stdin piped");
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    Ok(Spawned {
        child,
        stdin,
        stdout,
        stderr,
        tree,
    })
}

/// Exit code, or the signal number when killed by one (like `Process.terminationStatus`).
pub fn exit_code(status: &std::process::ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return sig;
        }
    }
    status.code().unwrap_or(-1)
}

/// Reads `\n`-terminated lines (without the terminator / a trailing `\r`), decoding UTF-8 lossily.
/// Returns `None` at EOF; a final unterminated line is still returned.
pub async fn next_line<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    buf: &mut Vec<u8>,
) -> Option<String> {
    buf.clear();
    match reader.read_until(b'\n', buf).await {
        Ok(0) | Err(_) => None,
        Ok(_) => {
            while matches!(buf.last(), Some(b'\n' | b'\r')) {
                buf.pop();
            }
            Some(String::from_utf8_lossy(buf).into_owned())
        }
    }
}

// ---------------------------------------------------------------------------------------------
// ProcessJob

/// A one-shot child: writes `input` to stdin and closes it, streams stdout line by line, and
/// reports the exit status with the captured stderr. Nothing is reported after `cancel()`.
pub struct ProcessJob {
    cancelled: Arc<AtomicBool>,
    tree: Arc<ProcessTree>,
}

impl ProcessJob {
    pub fn start(
        exe: &Path,
        args: &[String],
        extra_env: &[(&str, &str)],
        cwd: &Path,
        input: Option<String>,
        mut on_line: impl FnMut(&str) + Send + 'static,
        on_exit: impl FnOnce(i32, String) + Send + 'static,
    ) -> io::Result<Self> {
        let Spawned {
            mut child,
            mut stdin,
            stdout,
            mut stderr,
            tree,
        } = spawn(exe, args, extra_env, cwd)?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let pid = tree.pid();

        tokio::spawn(async move {
            if let Some(input) = input {
                let _ = stdin.write_all(input.as_bytes()).await;
            }
            let _ = stdin.shutdown().await;
            drop(stdin);
        });

        // Drain stderr concurrently so the child never blocks on a full pipe.
        let stderr_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            let _ = stderr.read_to_end(&mut buf).await;
            String::from_utf8_lossy(&buf).into_owned()
        });

        let flag = cancelled.clone();
        let keep_tree = tree.clone(); // keeps the Windows job alive until the child exits
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut buf = Vec::new();
            while let Some(line) = next_line(&mut reader, &mut buf).await {
                if !flag.load(Ordering::SeqCst) && !line.is_empty() {
                    on_line(&line);
                }
            }
            let status = child.wait().await.map(|s| exit_code(&s)).unwrap_or(-1);
            let err = stderr_task.await.unwrap_or_default();
            let was_cancelled = flag.load(Ordering::SeqCst);
            log::write(format!(
                "process exited pid={pid} status={status} cancelled={was_cancelled}"
            ));
            if !was_cancelled {
                on_exit(status, err);
            }
            drop(keep_tree);
        });

        Ok(Self { cancelled, tree })
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.tree.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_shim_current_format_script() {
        let shim = "@ECHO off\r\nGOTO start\r\n:find_dp0\r\nSET dp0=%~dp0\r\nEXIT /b\r\n:start\r\nSETLOCAL\r\nCALL :find_dp0\r\n\r\n\
IF EXIST \"%dp0%\\node.exe\" (\r\n  SET \"_prog=%dp0%\\node.exe\"\r\n) ELSE (\r\n  SET \"_prog=node\"\r\n  SET PATHEXT=%PATHEXT:;.JS;=;%\r\n)\r\n\r\n\
endLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & \"%_prog%\"  \"%dp0%\\node_modules\\@openai\\codex\\bin\\codex.js\" %*\r\n";
        assert_eq!(
            parse_cmd_shim(shim).unwrap(),
            ["node_modules", "@openai", "codex", "bin", "codex.js"]
        );
    }

    #[test]
    fn cmd_shim_binary_target() {
        let shim = "@ECHO off\r\nGOTO start\r\n:find_dp0\r\nSET dp0=%~dp0\r\nEXIT /b\r\n:start\r\nSETLOCAL\r\nCALL :find_dp0\r\n\
\"%dp0%\\node_modules\\@anthropic-ai\\claude-code\\bin\\claude.exe\"   %*\r\n";
        assert_eq!(
            parse_cmd_shim(shim).unwrap(),
            [
                "node_modules",
                "@anthropic-ai",
                "claude-code",
                "bin",
                "claude.exe"
            ]
        );
    }

    #[test]
    fn cmd_shim_legacy_format() {
        let shim = "@IF EXIST \"%~dp0\\node.exe\" (\r\n  \"%~dp0\\node.exe\"  \"%~dp0\\node_modules\\@anthropic-ai\\claude-code\\cli.js\" %*\r\n\
) ELSE (\r\n  @SETLOCAL\r\n  @SET PATHEXT=%PATHEXT:;.JS;=;%\r\n  node  \"%~dp0\\node_modules\\@anthropic-ai\\claude-code\\cli.js\" %*\r\n)";
        assert_eq!(
            parse_cmd_shim(shim).unwrap(),
            ["node_modules", "@anthropic-ai", "claude-code", "cli.js"]
        );
    }

    #[test]
    fn cmd_shim_unrelated_batch_file() {
        assert_eq!(
            parse_cmd_shim("@echo off\r\nC:\\tools\\claude.exe %*\r\n"),
            None
        );
    }

    #[test]
    fn scoop_shim() {
        let s = "path = \"C:\\Users\\me\\scoop\\apps\\nodejs\\current\\bin\\claude.cmd\"\r\nargs = \r\n";
        assert_eq!(
            parse_scoop_shim(s).unwrap(),
            PathBuf::from("C:\\Users\\me\\scoop\\apps\\nodejs\\current\\bin\\claude.cmd")
        );
        assert_eq!(parse_scoop_shim("args = x"), None);
    }

    #[tokio::test]
    async fn line_reader_handles_crlf_and_partial_last_line() {
        let data: &[u8] = b"one\r\ntwo\n\nlast";
        let mut r = BufReader::new(data);
        let mut buf = Vec::new();
        let mut lines = vec![];
        while let Some(l) = next_line(&mut r, &mut buf).await {
            lines.push(l);
        }
        assert_eq!(lines, ["one", "two", "", "last"]);
    }

    /// Cancel must take down grandchildren too (the reason for setsid / job objects).
    #[cfg(unix)]
    #[tokio::test]
    async fn cancel_kills_the_whole_tree() {
        let marker = std::env::temp_dir().join(format!("qt-tree-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let script = format!("sleep 30 & echo $! > {}; wait", marker.display());
        let job = ProcessJob::start(
            Path::new("/bin/sh"),
            &["-c".into(), script],
            &[],
            &std::env::temp_dir(),
            None,
            |_| {},
            |_, _| panic!("on_exit must not run after cancel"),
        )
        .unwrap();
        let mut grandchild = 0;
        for _ in 0..100 {
            if let Some(pid) = std::fs::read_to_string(&marker)
                .ok()
                .and_then(|s| s.trim().parse::<i32>().ok())
            {
                grandchild = pid;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert!(grandchild > 0, "grandchild did not start");
        job.cancel();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        let alive = unsafe { libc::kill(grandchild, 0) } == 0;
        let _ = std::fs::remove_file(&marker);
        assert!(!alive, "grandchild {grandchild} survived cancel");
    }
}
