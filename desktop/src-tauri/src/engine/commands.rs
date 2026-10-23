//! Tauri glue for the engine: commands invoked by the panel and the `translate://*` events sent
//! back to it.
//!
//! Events (all to the `panel` window), each tagged with the id `translate` returned:
//! - `translate://delta`    `{ id, text }`    streamed chunk
//! - `translate://finished` `{ id, text }`    full translation (may be empty: keep the deltas)
//! - `translate://failed`   `{ id, message }`
//!
//! Like `TranslationViewModel.swift`, only the latest translation reports: starting a new one or
//! calling `cancel` cancels the previous job and drops its in-flight events.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::io::AsyncReadExt;

use super::claude_worker::ClaudeWorker;
use super::settings::{Backend, Settings};
use super::{cli_locator, process_job, translation_engine, TranslationEvent, TranslationJob};
use crate::{log, PANEL};

#[derive(Default)]
pub struct EngineState {
    next_id: AtomicU64,
    /// Id of the translation whose events reach the panel; 0 = none.
    current_id: AtomicU64,
    job: Mutex<Option<(u64, Box<dyn TranslationJob>)>>,
}

impl EngineState {
    fn cancel_current(&self) {
        self.current_id.store(0, Ordering::SeqCst);
        let job = self.job.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some((_, job)) = job {
            job.cancel();
        }
    }
}

#[derive(Clone, Serialize)]
struct TextPayload {
    id: u64,
    text: String,
}

#[derive(Clone, Serialize)]
struct FailedPayload {
    id: u64,
    message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliInfo {
    pub name: String,
    pub path: String,
    pub version: String,
}

/// The current settings (kept by `crate::settings`, which also reloads the file when it changes).
pub fn load_settings<R: Runtime>(_app: &AppHandle<R>) -> Settings {
    crate::settings::current()
}

/// Called from `setup`: registers the state. The warm Claude process starts when the settings
/// are applied (`crate::settings::init`).
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    app.manage(EngineState::default());
}

/// Called on `RunEvent::Exit`: makes sure no `claude` process outlives the app.
pub fn shutdown() {
    tauri::async_runtime::block_on(async {
        let _ =
            tokio::time::timeout(Duration::from_secs(2), ClaudeWorker::shared().shutdown()).await;
    });
}

/// Starts a translation and returns its id. Events arrive as `translate://*`.
#[tauri::command]
pub async fn translate<R: Runtime>(
    app: AppHandle<R>,
    text: String,
    source: Option<String>,
    target: String,
    fallback: String,
) -> Result<u64, String> {
    let id = start(&app, &text, source.as_deref(), &target, &fallback, |_| {});
    Ok(id)
}

/// The body of the `translate` command, for Rust callers (the panel). `on_id` runs with the new
/// id after the previous translation is cancelled and before the engine can emit anything, so a
/// caller can announce the id first. Must run inside the async runtime.
pub fn start<R: Runtime>(
    app: &AppHandle<R>,
    text: &str,
    source: Option<&str>,
    target: &str,
    fallback: &str,
    on_id: impl FnOnce(u64),
) -> u64 {
    let state = app.state::<EngineState>();
    state.cancel_current();
    let id = state.next_id.fetch_add(1, Ordering::SeqCst) + 1;
    state.current_id.store(id, Ordering::SeqCst);
    on_id(id);

    let settings = load_settings(app);
    log::write(format!(
        "translate start id={id} backend={:?} source={} target={target} chars={}",
        settings.backend,
        source.unwrap_or("?"),
        text.chars().count()
    ));

    let handle = app.clone();
    let sink = Arc::new(move |event: TranslationEvent| {
        let state = handle.state::<EngineState>();
        if state.current_id.load(Ordering::SeqCst) != id {
            return; // cancelled or superseded
        }
        let _ = match event {
            TranslationEvent::Delta(text) => {
                handle.emit_to(PANEL, "translate://delta", TextPayload { id, text })
            }
            TranslationEvent::Finished(text) => {
                log::write(format!(
                    "translate finished id={id} chars={}",
                    text.chars().count()
                ));
                finish(&state, id);
                handle.emit_to(PANEL, "translate://finished", TextPayload { id, text })
            }
            TranslationEvent::Failed(message) => {
                log::write(format!("translate failed id={id}: {message}"));
                finish(&state, id);
                handle.emit_to(PANEL, "translate://failed", FailedPayload { id, message })
            }
        };
    });

    let job = translation_engine::translate(text, source, target, fallback, &settings, sink);
    if let Some(job) = job {
        let mut slot = state.job.lock().unwrap_or_else(|e| e.into_inner());
        if state.current_id.load(Ordering::SeqCst) == id {
            *slot = Some((id, job));
        } else {
            drop(slot);
            job.cancel(); // superseded (or already done) while starting
        }
    }
    id
}

/// Whether a translation is still running (neither finished, failed nor cancelled).
pub fn is_running<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.state::<EngineState>().current_id.load(Ordering::SeqCst) != 0
}

/// Cancels the running translation, if any (the `cancel` command without the log line).
pub fn cancel_current<R: Runtime>(app: &AppHandle<R>) {
    app.state::<EngineState>().cancel_current();
}

fn finish(state: &EngineState, id: u64) {
    let _ = state
        .current_id
        .compare_exchange(id, 0, Ordering::SeqCst, Ordering::SeqCst);
    let mut slot = state.job.lock().unwrap_or_else(|e| e.into_inner());
    if slot.as_ref().is_some_and(|(j, _)| *j == id) {
        *slot = None;
    }
}

/// Stops the running translation; the panel keeps whatever was streamed.
#[tauri::command]
pub async fn cancel<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    app.state::<EngineState>().cancel_current();
    log::write("translate cancelled");
    Ok(())
}

/// Re-reads the settings and (re)starts or stops the warm Claude process accordingly.
#[tauri::command]
pub async fn prewarm<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    translation_engine::prewarm(&load_settings(&app));
    Ok(())
}

/// Locates the CLI (`path` overrides the settings) and runs `--version`.
#[tauri::command]
pub async fn test_cli<R: Runtime>(
    app: AppHandle<R>,
    path: Option<String>,
    backend: Option<Backend>,
) -> Result<CliInfo, String> {
    let settings = load_settings(&app);
    let backend = backend.unwrap_or(settings.backend);
    let override_path = path.unwrap_or_else(|| settings.path_override(backend).to_string());
    check_cli(backend, &override_path).await
}

pub async fn check_cli(backend: Backend, override_path: &str) -> Result<CliInfo, String> {
    let name = backend.executable_name();
    cli_locator::reset_cache();
    let exe = cli_locator::find(name, override_path).ok_or_else(|| {
        format!("`{name}` CLI not found. Install it or set its path in Settings.")
    })?;
    let spawned = process_job::spawn(&exe, &["--version".into()], &[], &std::env::temp_dir())
        .map_err(|e| format!("Failed to launch {name}: {e}"))?;
    let process_job::Spawned {
        mut child,
        stdin,
        mut stdout,
        stderr,
        tree,
    } = spawned;
    drop(stdin);
    let _stderr = stderr; // keep the pipe open so a chatty CLI doesn't get SIGPIPE
    let read = async {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out).await;
        let status = child.wait().await.ok();
        (out, status)
    };
    let (out, status) = match tokio::time::timeout(Duration::from_secs(15), read).await {
        Ok(r) => r,
        Err(_) => {
            tree.kill();
            return Err(format!("`{name} --version` timed out"));
        }
    };
    if !status.is_some_and(|s| s.success()) {
        return Err(format!("`{name} --version` failed"));
    }
    Ok(CliInfo {
        name: name.into(),
        path: exe.display().to_string(),
        version: out.trim().to_string(),
    })
}
