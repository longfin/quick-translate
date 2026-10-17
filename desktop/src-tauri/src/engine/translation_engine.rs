//! Talks to the locally installed `claude` (Claude subscription) or `codex` (ChatGPT
//! subscription) CLI. Port of `macos/Sources/QuickTranslate/TranslationEngine.swift`: same
//! prompts, same arguments, same user-facing error messages.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use super::claude_worker::{self, ClaudeWorker, StreamLine};
use super::cli_locator;
use super::process_job::ProcessJob;
use super::settings::{Backend, Settings};
use super::{Sink, TranslationEvent, TranslationJob};
use crate::log;

impl TranslationJob for ProcessJob {
    fn cancel(&self) {
        ProcessJob::cancel(self);
    }
}

/// Spawn the resident Claude process ahead of time (shuts it down when disabled or not using
/// Claude).
pub fn prewarm(settings: &Settings) {
    let exe = (settings.backend == Backend::Claude && settings.keep_claude_warm)
        .then(|| cli_locator::find("claude", &settings.claude_path))
        .flatten();
    match exe {
        Some(exe) => ClaudeWorker::shared().prewarm(exe, settings.claude_model.trim().to_string()),
        None => ClaudeWorker::shared().shutdown_async(),
    }
}

/// The instruction line must end with a colon: with a full stop before the blank line, haiku
/// (without thinking) tends to answer "please provide the text" instead of translating.
pub fn instruction(source: Option<&str>, target: &str, fallback: &str) -> String {
    match source {
        Some(source) if source != target => format!("Translate from {source} into {target}:"),
        _ => format!(
            "Translate into {target}, or into {fallback} if the text is already mostly {target}:"
        ),
    }
}

pub fn system_prompt(source: Option<&str>, target: &str, fallback: &str) -> String {
    let task = match source {
        Some(source) if source != target => {
            format!("Translate the text provided by the user from {source} into {target}.")
        }
        _ => format!(
            "Translate the text provided by the user into {target}.\n\
             If the text is already written mostly in {target}, translate it into {fallback} instead."
        ),
    };
    format!(
        "You are a professional translation engine.\n\
         {task}\n\
         Preserve the original meaning, tone, formatting, line breaks, markdown and code blocks. \
         Do not translate code identifiers, URLs or proper nouns that are normally left as-is.\n\
         Output ONLY the translated text. No explanations, no notes, no quotes, no preamble."
    )
}

/// Starts a translation; events go to `sink` from a background task. Returns `None` when it could
/// not start (the failure has already been sent to `sink`). Must be called inside a tokio runtime.
pub fn translate(
    text: &str,
    source: Option<&str>,
    target: &str,
    fallback: &str,
    settings: &Settings,
    sink: Sink,
) -> Option<Box<dyn TranslationJob>> {
    let prompt = system_prompt(source, target, fallback);
    let backend = settings.backend;
    let name = backend.executable_name();

    let Some(exe) = cli_locator::find(name, settings.current_path_override()) else {
        sink(TranslationEvent::Failed(format!(
            "`{name}` CLI not found. Install it or set its path in Settings."
        )));
        return None;
    };

    log::write(format!("using {name} at {}", exe.display()));
    match backend {
        Backend::Claude => {
            let model = settings.claude_model.trim().to_string();
            if settings.keep_claude_warm {
                let instruction = instruction(source, target, fallback);
                return Some(Box::new(ClaudeWorker::shared().translate(
                    exe,
                    model,
                    &instruction,
                    text,
                    sink,
                )));
            }
            run_claude(&exe, &prompt, text, &model, sink)
        }
        Backend::Codex => run_codex(&exe, &prompt, text, settings.codex_model.trim(), sink),
    }
}

// ---- Claude Code CLI (one-shot) -------------------------------------------------------------

fn run_claude(
    exe: &Path,
    prompt: &str,
    text: &str,
    model: &str,
    sink: Sink,
) -> Option<Box<dyn TranslationJob>> {
    let args = claude_worker::base_args(prompt, model);
    let got_result = Arc::new(AtomicBool::new(false));
    let input = if text.ends_with('\n') {
        text.to_string()
    } else {
        format!("{text}\n")
    };

    let (line_sink, exit_sink, flag) = (sink.clone(), sink.clone(), got_result.clone());
    let started = ProcessJob::start(
        exe,
        &args,
        claude_worker::CLAUDE_ENV,
        &scratch_directory(),
        Some(input),
        move |line| match claude_worker::parse_line(line) {
            StreamLine::TextDelta(t) => line_sink(TranslationEvent::Delta(t)),
            StreamLine::Result {
                is_error,
                result,
                errors,
                ..
            } => {
                flag.store(true, Ordering::SeqCst);
                line_sink(claude_worker::result_event(is_error, result, &errors));
            }
            StreamLine::Other => {}
        },
        move |status, stderr| {
            if !got_result.load(Ordering::SeqCst) {
                let t = tail(&stderr, 6);
                exit_sink(TranslationEvent::Failed(if t.is_empty() {
                    format!("claude exited with code {status}")
                } else {
                    t
                }));
            }
        },
    );
    match started {
        Ok(job) => Some(Box::new(job)),
        Err(e) => {
            sink(TranslationEvent::Failed(format!(
                "Failed to launch claude: {e}"
            )));
            None
        }
    }
}

// ---- Codex CLI ------------------------------------------------------------------------------

pub fn codex_args(
    dir: &Path,
    out_file: &Path,
    model: &str,
    prompt: &str,
    text: &str,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "exec".into(),
        "--skip-git-repo-check".into(),
        "-s".into(),
        "read-only".into(),
        "-C".into(),
        dir.display().to_string(),
        "-o".into(),
        out_file.display().to_string(),
    ];
    if !model.is_empty() {
        args.extend(["-m".into(), model.into()]);
    }
    args.push(format!("{prompt}\n\nText to translate:\n\n{text}"));
    args
}

fn run_codex(
    exe: &Path,
    prompt: &str,
    text: &str,
    model: &str,
    sink: Sink,
) -> Option<Box<dyn TranslationJob>> {
    let dir = scratch_directory().join(format!("codex-{}", unique_id()));
    let _ = std::fs::create_dir_all(&dir);
    let out_file = dir.join("last-message.txt");
    let args = codex_args(&dir, &out_file, model, prompt, text);

    let exit_sink = sink.clone();
    let cleanup = dir.clone();
    let started = ProcessJob::start(
        exe,
        &args,
        &[],
        &dir,
        None,
        |_| {},
        move |status, stderr| {
            let result = std::fs::read_to_string(&out_file)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            let _ = std::fs::remove_dir_all(&cleanup);
            if !result.is_empty() {
                exit_sink(TranslationEvent::Finished(result));
            } else {
                let t = tail(&stderr, 6);
                exit_sink(TranslationEvent::Failed(if t.is_empty() {
                    format!("codex exited with code {status}")
                } else {
                    t
                }));
            }
        },
    );
    match started {
        Ok(job) => Some(Box::new(job)),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            sink(TranslationEvent::Failed(format!(
                "Failed to launch codex: {e}"
            )));
            None
        }
    }
}

// ---- Helpers --------------------------------------------------------------------------------

pub fn scratch_directory() -> PathBuf {
    let dir = std::env::temp_dir().join("QuickTranslate");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn unique_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(
        "{}-{nanos:x}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

/// The last `lines` non-blank lines of `s`.
pub fn tail(s: &str, lines: usize) -> String {
    let all: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_with_detected_source() {
        assert_eq!(
            instruction(Some("English"), "Korean", "English"),
            "Translate from English into Korean:"
        );
    }

    #[test]
    fn instruction_without_or_same_source() {
        let expected =
            "Translate into Korean, or into English if the text is already mostly Korean:";
        assert_eq!(instruction(None, "Korean", "English"), expected);
        assert_eq!(instruction(Some("Korean"), "Korean", "English"), expected);
    }

    #[test]
    fn instruction_always_ends_with_colon() {
        for s in [None, Some("Japanese"), Some("Korean")] {
            assert!(instruction(s, "Korean", "English").ends_with(':'));
        }
    }

    #[test]
    fn system_prompt_with_source() {
        assert_eq!(
            system_prompt(Some("English"), "Korean", "Japanese"),
            "You are a professional translation engine.\n\
             Translate the text provided by the user from English into Korean.\n\
             Preserve the original meaning, tone, formatting, line breaks, markdown and code blocks. Do not translate code identifiers, URLs or proper nouns that are normally left as-is.\n\
             Output ONLY the translated text. No explanations, no notes, no quotes, no preamble."
        );
    }

    #[test]
    fn system_prompt_with_fallback() {
        assert_eq!(
            system_prompt(None, "Korean", "English"),
            "You are a professional translation engine.\n\
             Translate the text provided by the user into Korean.\n\
             If the text is already written mostly in Korean, translate it into English instead.\n\
             Preserve the original meaning, tone, formatting, line breaks, markdown and code blocks. Do not translate code identifiers, URLs or proper nouns that are normally left as-is.\n\
             Output ONLY the translated text. No explanations, no notes, no quotes, no preamble."
        );
    }

    #[test]
    fn worker_system_prompt_mentions_the_colon() {
        assert!(claude_worker::SYSTEM_PROMPT.contains("instruction line ending in a colon"));
    }

    #[test]
    fn codex_arguments() {
        let args = codex_args(
            Path::new("/tmp/d"),
            Path::new("/tmp/d/last-message.txt"),
            "gpt-5",
            "P",
            "hi",
        );
        assert_eq!(
            args,
            [
                "exec",
                "--skip-git-repo-check",
                "-s",
                "read-only",
                "-C",
                "/tmp/d",
                "-o",
                "/tmp/d/last-message.txt",
                "-m",
                "gpt-5",
                "P\n\nText to translate:\n\nhi"
            ]
        );
        assert!(
            !codex_args(Path::new("/d"), Path::new("/d/o"), "", "P", "x")
                .contains(&"-m".to_string())
        );
    }

    #[test]
    fn tail_skips_blank_lines() {
        assert_eq!(tail("a\n\n b \n  \nc\nd\n", 3), " b \nc\nd");
        assert_eq!(tail("", 6), "");
    }
}
