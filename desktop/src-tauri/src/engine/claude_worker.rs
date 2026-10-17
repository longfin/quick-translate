//! A resident `claude -p --input-format stream-json` process. Port of
//! `macos/Sources/QuickTranslate/ClaudeWorker.swift`.
//!
//! Keeping it alive skips the CLI startup and keeps the HTTP connection and prompt cache warm. After
//! every turn we send `/clear`, which resets the conversation locally (no API call), so each
//! translation still starts from a fresh context. One turn at a time: a stream-json turn cannot be
//! interrupted, so a new request while one is running (or a cancel) kills the process and starts
//! over.
//!
//! The Swift class serialises everything on a private `DispatchQueue`; here a single actor task
//! owns all state and processes [`Msg`]s in order (requests, stdout lines, process exit).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::ChildStdin;
use tokio::sync::{mpsc, oneshot};

use super::process_job::{self, ProcessTree};
use super::{Sink, TranslationEvent, TranslationJob};
use crate::log;

pub const SYSTEM_PROMPT: &str = "You are a professional translation engine.
Each user message starts with an instruction line ending in a colon, followed by the text to translate.
Preserve the original meaning, tone, formatting, line breaks, markdown and code blocks. Do not translate code identifiers, URLs or proper nouns that are normally left as-is.
Output ONLY the translated text. No explanations, no notes, no quotes, no preamble.";

/// `claude` arguments shared by the warm worker and the one-shot path (minus the input format).
pub fn base_args(system_prompt: &str, model: &str) -> Vec<String> {
    let mut args: Vec<String> = [
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--tools",
        "",                    // no tools: pure text generation
        "--strict-mcp-config", // skip user MCP servers (fast startup)
        "--setting-sources",
        "", // skip user settings / hooks
        "--no-session-persistence",
        "--system-prompt",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    args.push(system_prompt.to_string());
    if !model.is_empty() {
        args.extend(["--model".to_string(), model.to_string()]);
    }
    args
}

/// Translation needs no extended thinking; this halves latency.
pub const CLAUDE_ENV: &[(&str, &str)] = &[("MAX_THINKING_TOKENS", "0")];

// ---------------------------------------------------------------------------------------------
// stream-json parsing (pure)

/// The parts of a `--output-format stream-json` line we care about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamLine {
    /// `stream_event` → `content_block_delta` → `text_delta`.
    TextDelta(String),
    /// The end of a turn (also emitted for local commands like `/clear`, with `num_turns: 0`).
    Result {
        is_error: bool,
        result: String,
        errors: Vec<String>,
        num_turns: Option<u64>,
    },
    Other,
}

pub fn parse_line(line: &str) -> StreamLine {
    let Ok(obj) = serde_json::from_str::<Value>(line) else {
        return StreamLine::Other;
    };
    match obj["type"].as_str() {
        Some("stream_event") => {
            let ev = &obj["event"];
            if ev["type"] == "content_block_delta" && ev["delta"]["type"] == "text_delta" {
                if let Some(t) = ev["delta"]["text"].as_str() {
                    return StreamLine::TextDelta(t.to_string());
                }
            }
            StreamLine::Other
        }
        Some("result") => StreamLine::Result {
            is_error: obj["is_error"].as_bool().unwrap_or(false),
            result: obj["result"].as_str().unwrap_or("").trim().to_string(),
            errors: obj["errors"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|e| e.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            num_turns: obj["num_turns"].as_u64(),
        },
        _ => StreamLine::Other,
    }
}

/// The event a `result` line turns into (same messages as Swift).
pub fn result_event(is_error: bool, result: String, errors: &[String]) -> TranslationEvent {
    if !is_error {
        return TranslationEvent::Finished(result);
    }
    if !result.is_empty() {
        return TranslationEvent::Failed(result);
    }
    let errors = errors.join("\n");
    TranslationEvent::Failed(if errors.is_empty() {
        "Claude returned an error.".into()
    } else {
        errors
    })
}

fn user_message(content: &str) -> Value {
    json!({"type": "user", "message": {"role": "user", "content": content}})
}

// ---------------------------------------------------------------------------------------------
// Worker

#[derive(Debug, Clone, PartialEq, Eq)]
struct Config {
    exe: PathBuf,
    model: String,
}

enum Msg {
    Prewarm(Config),
    Shutdown(Option<oneshot::Sender<()>>),
    Translate {
        id: u64,
        cfg: Config,
        content: String,
        sink: Sink,
    },
    Cancel(u64),
    Line {
        gen: u64,
        line: String,
    },
    Stderr {
        gen: u64,
        text: String,
    },
    Exited {
        gen: u64,
        status: i32,
    },
}

pub struct ClaudeWorker {
    tx: OnceLock<mpsc::UnboundedSender<Msg>>,
    next_turn_id: AtomicU64,
}

/// Returned to the caller; cancelling drops the process (a turn cannot be interrupted).
pub struct TurnHandle {
    id: u64,
}

impl TranslationJob for TurnHandle {
    fn cancel(&self) {
        ClaudeWorker::shared().send(Msg::Cancel(self.id));
    }
}

impl ClaudeWorker {
    pub fn shared() -> &'static ClaudeWorker {
        static SHARED: OnceLock<ClaudeWorker> = OnceLock::new();
        SHARED.get_or_init(|| ClaudeWorker {
            tx: OnceLock::new(),
            next_turn_id: AtomicU64::new(0),
        })
    }

    /// Starts the actor on first use, so the first call must come from inside a tokio runtime.
    fn send(&self, msg: Msg) {
        let tx = self.tx.get_or_init(|| {
            let (tx, rx) = mpsc::unbounded_channel();
            tokio::spawn(Actor::new(tx.clone()).run(rx));
            tx
        });
        let _ = tx.send(msg);
    }

    /// Spawn ahead of time so the first translation is fast too.
    pub fn prewarm(&self, exe: PathBuf, model: String) {
        self.send(Msg::Prewarm(Config { exe, model }));
    }

    /// Kills the process. Awaiting the result guarantees it is gone (used at app exit).
    pub async fn shutdown(&self) {
        if self.tx.get().is_none() {
            return; // never started
        }
        let (tx, rx) = oneshot::channel();
        self.send(Msg::Shutdown(Some(tx)));
        let _ = rx.await;
    }

    pub fn shutdown_async(&self) {
        if self.tx.get().is_some() {
            self.send(Msg::Shutdown(None));
        }
    }

    pub fn translate(
        &self,
        exe: PathBuf,
        model: String,
        instruction: &str,
        text: &str,
        sink: Sink,
    ) -> TurnHandle {
        let id = self.next_turn_id.fetch_add(1, Ordering::SeqCst) + 1;
        let content = format!("{instruction}\n\n{text}");
        self.send(Msg::Translate {
            id,
            cfg: Config { exe, model },
            content,
            sink,
        });
        TurnHandle { id }
    }
}

struct Proc {
    gen: u64,
    stdin: ChildStdin,
    tree: Arc<ProcessTree>,
}

struct Turn {
    id: u64,
    sink: Sink,
}

struct Actor {
    tx: mpsc::UnboundedSender<Msg>,
    process: Option<Proc>,
    config: Option<Config>,
    current: Option<Turn>,
    generation: u64,
    stderr_tail: String,
    /// `/clear` commands sent whose (empty, `num_turns: 0`) `result` has not arrived yet. Without
    /// this, a request sent right after a turn would be "finished" by `/clear`'s result.
    pending_clears: u32,
}

impl Actor {
    fn new(tx: mpsc::UnboundedSender<Msg>) -> Self {
        Self {
            tx,
            process: None,
            config: None,
            current: None,
            generation: 0,
            stderr_tail: String::new(),
            pending_clears: 0,
        }
    }

    async fn run(mut self, mut rx: mpsc::UnboundedReceiver<Msg>) {
        while let Some(msg) = rx.recv().await {
            match msg {
                Msg::Prewarm(cfg) => self.ensure_process(cfg),
                Msg::Shutdown(ack) => {
                    self.current = None;
                    self.kill_process();
                    if let Some(ack) = ack {
                        let _ = ack.send(());
                    }
                }
                Msg::Translate {
                    id,
                    cfg,
                    content,
                    sink,
                } => self.translate(id, cfg, content, sink).await,
                Msg::Cancel(id) => {
                    if self.current.as_ref().is_some_and(|c| c.id == id) {
                        self.current = None;
                        self.kill_process();
                        // Respawn so the next request is warm.
                        if let Some(cfg) = self.config.clone() {
                            self.ensure_process(cfg);
                        }
                    }
                }
                Msg::Line { gen, line } => self.handle_line(gen, &line).await,
                Msg::Stderr { gen, text } => {
                    if self.process.as_ref().is_some_and(|p| p.gen == gen) {
                        self.stderr_tail.push_str(&text);
                        let excess = self.stderr_tail.len().saturating_sub(2000);
                        if excess > 0 {
                            let cut = (excess..=self.stderr_tail.len())
                                .find(|&i| self.stderr_tail.is_char_boundary(i))
                                .unwrap_or(0);
                            self.stderr_tail.drain(..cut);
                        }
                    }
                }
                Msg::Exited { gen, status } => self.process_exited(gen, status),
            }
        }
    }

    async fn translate(&mut self, id: u64, cfg: Config, content: String, sink: Sink) {
        if self.current.is_some() {
            // A turn is still running: a stream-json session can't be interrupted, so start over.
            self.current = None;
            self.kill_process();
        }
        self.ensure_process(cfg);
        if self.process.is_none() {
            let why = if self.stderr_tail.is_empty() {
                "spawn failed".to_string()
            } else {
                self.stderr_tail.clone()
            };
            sink(TranslationEvent::Failed(format!(
                "Failed to launch claude: {why}"
            )));
            return;
        }
        self.current = Some(Turn { id, sink });
        if let Some(p) = self.process.as_mut() {
            // A write error means the process died; its exit then fails the turn.
            let _ = send(&mut p.stdin, &user_message(&content)).await;
        }
    }

    fn ensure_process(&mut self, cfg: Config) {
        if self.process.is_some() && self.config.as_ref() == Some(&cfg) {
            return;
        }
        self.kill_process();
        self.config = Some(cfg.clone());

        let mut args = base_args(SYSTEM_PROMPT, &cfg.model);
        // Same order as Swift (`claude -p --input-format stream-json ...`), so `pgrep -f "claude -p"` finds it.
        args.splice(
            1..1,
            ["--input-format".to_string(), "stream-json".to_string()],
        );
        let spawned = match process_job::spawn(&cfg.exe, &args, CLAUDE_ENV, &std::env::temp_dir()) {
            Ok(s) => s,
            Err(e) => {
                log::write(format!("claude worker spawn failed: {e}"));
                self.stderr_tail = e.to_string();
                return;
            }
        };
        self.generation += 1;
        let gen = self.generation;
        self.stderr_tail.clear();
        self.pending_clears = 0;
        log::write(format!(
            "claude worker spawned pid={} model={}",
            spawned.tree.pid(),
            cfg.model
        ));

        let process_job::Spawned {
            mut child,
            stdin,
            stdout,
            mut stderr,
            tree,
        } = spawned;
        self.process = Some(Proc { gen, stdin, tree });

        let tx = self.tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut buf = Vec::new();
            while let Some(line) = process_job::next_line(&mut reader, &mut buf).await {
                if !line.is_empty() {
                    let _ = tx.send(Msg::Line { gen, line });
                }
            }
            let status = child
                .wait()
                .await
                .map(|s| process_job::exit_code(&s))
                .unwrap_or(-1);
            let _ = tx.send(Msg::Exited { gen, status });
        });
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let mut chunk = [0u8; 4096];
            while let Ok(n) = stderr.read(&mut chunk).await {
                if n == 0 {
                    break;
                }
                let text = String::from_utf8_lossy(&chunk[..n]).into_owned();
                let _ = tx.send(Msg::Stderr { gen, text });
            }
        });
    }

    fn kill_process(&mut self) {
        if let Some(p) = self.process.take() {
            drop(p.stdin);
            p.tree.kill();
        }
    }

    async fn handle_line(&mut self, gen: u64, line: &str) {
        if !self.process.as_ref().is_some_and(|p| p.gen == gen) {
            return; // an old process we already replaced
        }
        match parse_line(line) {
            StreamLine::TextDelta(t) => {
                if let Some(cur) = &self.current {
                    (cur.sink)(TranslationEvent::Delta(t));
                }
            }
            StreamLine::Result {
                is_error,
                result,
                errors,
                num_turns,
            } => {
                if self.pending_clears > 0 && num_turns == Some(0) && !is_error {
                    self.pending_clears -= 1;
                    return;
                }
                let Some(cur) = self.current.take() else {
                    return;
                };
                (cur.sink)(result_event(is_error, result, &errors));
                // Fresh context for the next translation; handled locally by the CLI, no API call.
                if let Some(p) = self.process.as_mut() {
                    if send(&mut p.stdin, &user_message("/clear")).await.is_ok() {
                        self.pending_clears += 1;
                    }
                }
            }
            StreamLine::Other => {}
        }
    }

    fn process_exited(&mut self, gen: u64, status: i32) {
        if !self.process.as_ref().is_some_and(|p| p.gen == gen) {
            return; // an old process we already replaced
        }
        log::write(format!("claude worker exited status={status}"));
        self.process = None;
        if let Some(cur) = self.current.take() {
            let lines: Vec<&str> = self.stderr_tail.lines().collect();
            let tail = lines[lines.len().saturating_sub(4)..].join("\n");
            (cur.sink)(TranslationEvent::Failed(if tail.is_empty() {
                format!("claude exited with code {status}")
            } else {
                tail
            }));
        }
    }
}

async fn send(stdin: &mut ChildStdin, v: &Value) -> std::io::Result<()> {
    let mut data = serde_json::to_vec(v)?;
    data.push(b'\n');
    stdin.write_all(&data).await?;
    stdin.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_delta() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"안녕"}},"session_id":"x"}"#;
        assert_eq!(parse_line(line), StreamLine::TextDelta("안녕".into()));
    }

    #[test]
    fn ignores_other_stream_events() {
        for line in [
            r#"{"type":"stream_event","event":{"type":"message_start","message":{}}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"hm"}}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{"}}}"#,
            r#"{"type":"system","subtype":"init","model":"claude-haiku"}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"x"}]}}"#,
            r#"{"type":"conversation_reset","trigger":"clear"}"#,
            "not json",
            "",
        ] {
            assert_eq!(parse_line(line), StreamLine::Other, "{line}");
        }
    }

    #[test]
    fn parses_success_result() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":1,"result":"  안녕하세요\n","session_id":"x"}"#;
        let StreamLine::Result {
            is_error,
            result,
            errors,
            num_turns,
        } = parse_line(line)
        else {
            panic!()
        };
        assert!(!is_error);
        assert_eq!(num_turns, Some(1));
        assert_eq!(
            result_event(is_error, result, &errors),
            TranslationEvent::Finished("안녕하세요".into())
        );
    }

    #[test]
    fn clear_result_is_recognisable() {
        // What `/clear` produces (observed with claude 2.1.302).
        let line = r#"{"is_error":false,"duration_api_ms":0,"num_turns":0,"stop_reason":null,"result":"","type":"result","subtype":"success"}"#;
        assert_eq!(
            parse_line(line),
            StreamLine::Result {
                is_error: false,
                result: String::new(),
                errors: vec![],
                num_turns: Some(0)
            }
        );
    }

    #[test]
    fn error_result_messages() {
        let StreamLine::Result {
            is_error,
            result,
            errors,
            ..
        } = parse_line(
            r#"{"type":"result","is_error":true,"result":"Invalid API key · Please run /login"}"#,
        )
        else {
            panic!()
        };
        assert_eq!(
            result_event(is_error, result, &errors),
            TranslationEvent::Failed("Invalid API key · Please run /login".into())
        );

        let StreamLine::Result {
            is_error,
            result,
            errors,
            ..
        } = parse_line(
            r#"{"type":"result","subtype":"error_during_execution","is_error":true,"errors":["a",1,"b"]}"#,
        )
        else {
            panic!()
        };
        assert_eq!(
            result_event(is_error, result, &errors),
            TranslationEvent::Failed("a\nb".into())
        );

        assert_eq!(
            result_event(true, String::new(), &[]),
            TranslationEvent::Failed("Claude returned an error.".into())
        );
    }

    #[test]
    fn args_match_swift() {
        let args = base_args("SYS", "haiku");
        assert_eq!(
            args,
            [
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--include-partial-messages",
                "--tools",
                "",
                "--strict-mcp-config",
                "--setting-sources",
                "",
                "--no-session-persistence",
                "--system-prompt",
                "SYS",
                "--model",
                "haiku"
            ]
        );
        assert!(!base_args("SYS", "").contains(&"--model".to_string()));
    }

    #[test]
    fn user_message_shape() {
        let expected: Value =
            serde_json::from_str(r#"{"type":"user","message":{"role":"user","content":"/clear"}}"#)
                .unwrap();
        assert_eq!(user_message("/clear"), expected);
    }
}
