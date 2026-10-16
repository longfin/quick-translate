//! Resident `claude -p --input-format stream-json` process — a port of
//! `Sources/QuickTranslate/ClaudeWorker.swift`. One turn at a time; `/clear` after each turn so the
//! next translation starts from a fresh context (handled locally by the CLI, no API call).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;

use crate::log;

const SYSTEM_PROMPT: &str = "You are a professional translation engine.
Each user message starts with an instruction line ending in a colon, followed by the text to translate.
Preserve the original meaning, tone, formatting, line breaks, markdown and code blocks. Do not translate code identifiers, URLs or proper nouns that are normally left as-is.
Output ONLY the translated text. No explanations, no notes, no quotes, no preamble.";

/// Hardcoded for the spike (Swift: `instruction(source: nil, target: "Korean", fallback: "English")`).
const INSTRUCTION: &str = "Translate into Korean, or into English if the text is already mostly Korean:";

const MODEL: &str = "haiku";

#[derive(Default)]
struct Inner {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
}

pub struct Worker {
    app: AppHandle,
    exe: Option<PathBuf>,
    inner: Mutex<Inner>,
    /// Incremented on every spawn; reader tasks of older processes ignore their output.
    generation: AtomicU64,
    busy: AtomicBool,
    first_delta_seen: AtomicBool,
}

impl Worker {
    pub fn new(app: AppHandle) -> Arc<Self> {
        let exe = locate("claude");
        log::write(format!("claude CLI: {exe:?}"));
        Arc::new(Self {
            app,
            exe,
            inner: Mutex::new(Inner::default()),
            generation: AtomicU64::new(0),
            busy: AtomicBool::new(false),
            first_delta_seen: AtomicBool::new(false),
        })
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    pub async fn prewarm(self: &Arc<Self>) {
        let mut inner = self.inner.lock().await;
        self.ensure(&mut inner).await;
    }

    pub async fn shutdown(&self) {
        let mut inner = self.inner.lock().await;
        kill(&mut inner).await;
    }

    pub async fn translate(self: &Arc<Self>, text: String) {
        let mut inner = self.inner.lock().await;
        if self.busy.swap(false, Ordering::SeqCst) {
            // A stream-json turn cannot be interrupted: start over, like the Swift worker.
            kill(&mut inner).await;
        }
        self.ensure(&mut inner).await;
        let Some(stdin) = inner.stdin.as_mut() else {
            let _ = self.app.emit("tr-error", "Failed to launch claude");
            return;
        };
        self.first_delta_seen.store(false, Ordering::SeqCst);
        self.busy.store(true, Ordering::SeqCst);
        let content = format!("{INSTRUCTION}\n\n{text}");
        if let Err(e) = send(stdin, &json!({"type": "user", "message": {"role": "user", "content": content}})).await {
            self.busy.store(false, Ordering::SeqCst);
            let _ = self.app.emit("tr-error", format!("write failed: {e}"));
            return;
        }
        log::write("turn sent to claude");
    }

    async fn ensure(self: &Arc<Self>, inner: &mut Inner) {
        if let Some(child) = inner.child.as_mut() {
            if matches!(child.try_wait(), Ok(None)) {
                return;
            }
        }
        let Some(exe) = self.exe.clone() else { return };
        let mut cmd = Command::new(&exe);
        cmd.args([
            "-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose",
            "--include-partial-messages", "--tools", "", "--strict-mcp-config", "--setting-sources", "",
            "--no-session-persistence", "--system-prompt", SYSTEM_PROMPT, "--model", MODEL,
        ])
        .env("PATH", search_path())
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .env("MAX_THINKING_TOKENS", "0")
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                log::write(format!("claude spawn failed: {e}"));
                return;
            }
        };
        let gen_id = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        log::write(format!("claude worker spawned pid={:?}", child.id()));
        let stdout = child.stdout.take().expect("stdout piped");
        inner.stdin = child.stdin.take();
        inner.child = Some(child);

        let this = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if this.generation.load(Ordering::SeqCst) != gen_id {
                    break;
                }
                this.handle_line(&line).await;
            }
            log::write(format!("claude worker gen={gen_id} stdout closed"));
            if this.generation.load(Ordering::SeqCst) == gen_id && this.busy.swap(false, Ordering::SeqCst) {
                let _ = this.app.emit("tr-error", "claude exited");
            }
        });
    }

    async fn handle_line(self: &Arc<Self>, line: &str) {
        let Ok(obj) = serde_json::from_str::<Value>(line) else { return };
        match obj["type"].as_str() {
            Some("stream_event") => {
                if !self.busy.load(Ordering::SeqCst) {
                    return;
                }
                let ev = &obj["event"];
                if ev["type"] == "content_block_delta" && ev["delta"]["type"] == "text_delta" {
                    if let Some(t) = ev["delta"]["text"].as_str() {
                        if !self.first_delta_seen.swap(true, Ordering::SeqCst) {
                            log::write("FIRST_DELTA");
                        }
                        let _ = self.app.emit("tr-delta", t);
                    }
                }
            }
            Some("result") => {
                if !self.busy.swap(false, Ordering::SeqCst) {
                    return;
                }
                let result = obj["result"].as_str().unwrap_or("").trim().to_string();
                if obj["is_error"].as_bool().unwrap_or(false) {
                    log::write(format!("RESULT_ERROR {result}"));
                    let _ = self.app.emit("tr-error", result);
                } else {
                    log::write(format!("RESULT_DONE chars={}", result.chars().count()));
                    let _ = self.app.emit("tr-done", result);
                }
                let mut inner = self.inner.lock().await;
                if let Some(stdin) = inner.stdin.as_mut() {
                    let _ = send(stdin, &json!({"type": "user", "message": {"role": "user", "content": "/clear"}})).await;
                }
            }
            _ => {}
        }
    }
}

async fn send(stdin: &mut ChildStdin, v: &Value) -> std::io::Result<()> {
    let mut data = serde_json::to_vec(v)?;
    data.push(b'\n');
    stdin.write_all(&data).await?;
    stdin.flush().await
}

async fn kill(inner: &mut Inner) {
    inner.stdin = None;
    if let Some(mut c) = inner.child.take() {
        let _ = c.kill().await;
    }
}

// ---- CLI locator (port of CLILocator.swift) ----

fn candidate_dirs() -> Vec<PathBuf> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let mut dirs: Vec<PathBuf> = [
        ".local/bin", ".npm/bin", ".npm-global/bin", ".claude/local", ".claude/local/node_modules/.bin",
    ]
    .iter()
    .map(|d| home.join(d))
    .collect();
    dirs.push("/opt/homebrew/bin".into());
    dirs.push("/usr/local/bin".into());
    for d in [".volta/bin", ".bun/bin", ".cargo/bin", ".yarn/bin"] {
        dirs.push(home.join(d));
    }
    let nvm = home.join(".nvm/versions/node");
    if let Ok(rd) = std::fs::read_dir(&nvm) {
        let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        v.sort();
        v.reverse();
        dirs.extend(v.into_iter().map(|p| p.join("bin")));
    }
    dirs.push("/usr/bin".into());
    dirs.push("/bin".into());
    dirs
}

fn search_path() -> String {
    let mut parts: Vec<String> = candidate_dirs().iter().map(|p| p.display().to_string()).collect();
    parts.push(std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()));
    parts.join(":")
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

pub fn locate(name: &str) -> Option<PathBuf> {
    for dir in candidate_dirs() {
        let p = dir.join(name);
        if is_executable(&p) {
            return Some(p);
        }
    }
    let out = std::process::Command::new("/bin/zsh")
        .args(["-lc", &format!("command -v {name}")])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    s.starts_with('/').then(|| PathBuf::from(s))
}
