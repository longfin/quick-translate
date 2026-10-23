//! Translation engine (Phase 1, issue #3). Not implemented yet.
//!
//! Planned modules, ported from `macos/Sources/QuickTranslate/`:
//! - `cli_locator`: find `claude` / `codex` (npm shims, native installers, version managers).
//! - `process_job`: spawn with piped UTF-8 stdio, stream lines, kill the whole tree on cancel.
//! - `claude_worker`: resident stream-json process, prewarm, `/clear` between turns.
//! - `translation_engine`: Claude (warm / one-shot) and Codex paths, same prompts and errors.
