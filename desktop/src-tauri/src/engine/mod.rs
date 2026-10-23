//! Translation engine (Phase 1, issue #3), ported 1:1 in behaviour from
//! `macos/Sources/QuickTranslate/`:
//! - `cli_locator`: find `claude` / `codex` (npm shims, native installers, version managers).
//! - `process_job`: spawn with piped UTF-8 stdio, stream lines, kill the whole tree on cancel.
//! - `claude_worker`: resident stream-json process, prewarm, `/clear` between turns.
//! - `translation_engine`: Claude (warm / one-shot) and Codex paths, same prompts and errors.
//! - `settings`: the engine keys of the Swift `AppSettings`, from a JSON file.
//! - `commands`: Tauri commands and `translate://*` events for the panel.
//!
//! The core (everything but `commands`) does not touch Tauri; it reports through a [`Sink`] and
//! must be driven from inside a tokio runtime (Tauri's async runtime in the app).

pub mod claude_worker;
pub mod cli_locator;
pub mod commands;
pub mod language;
pub mod process_job;
pub mod settings;
pub mod translation_engine;

#[cfg(test)]
mod live_tests;

use std::sync::Arc;

/// Mirrors Swift's `TranslationEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslationEvent {
    Delta(String),
    /// The full translation (may be empty when only deltas carried text).
    Finished(String),
    Failed(String),
}

/// Receives the events of one translation, from any thread.
pub type Sink = Arc<dyn Fn(TranslationEvent) + Send + Sync>;

/// A running translation (Swift's `TranslationJob` protocol).
pub trait TranslationJob: Send + Sync {
    fn cancel(&self);
}
