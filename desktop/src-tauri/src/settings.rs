//! Settings (Phase 4, issue #3): the in-memory copy of `settings.json`, applying changes as they
//! happen, and the Settings window (`settings`, pre-created hidden in `tauri.conf.json`, UI in
//! `src/settings.ts`).
//!
//! Every change goes through [`commit`]: from the window ([`settings_update`],
//! [`settings_set_hotkey`]) or from the file itself, which a watcher re-reads when it is edited
//! by hand. `commit` saves (atomically), applies the difference to the running app (trigger
//! interval and mode, hotkey, warm Claude process, autostart, tray language) and emits
//! `settings://changed` with a [`View`] to every window. The panel reads the rest
//! (close-on-outside-click, target / fallback, interface language) from [`current`] when it
//! needs it.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime};

use serde::Serialize;
use serde_json::{Map, Value};
use tauri::{AppHandle, Emitter, Manager, Runtime, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;

use crate::engine::settings::{Backend, Settings};
use crate::engine::{cli_locator, translation_engine};
use crate::trigger::{self, shortcut};
use crate::{i18n, log, panel, tray};

/// Label of the pre-created, hidden Settings window.
pub const WINDOW: &str = "settings";

/// How often the watcher looks at the file's modification time.
const WATCH_INTERVAL: Duration = Duration::from_secs(1);

struct Store {
    path: Option<PathBuf>,
    settings: Mutex<Settings>,
    /// The text of our last write, so the watcher doesn't reload our own save.
    written: Mutex<Option<String>>,
    /// Serializes commits (window commands and the watcher).
    commit: Mutex<()>,
}

static STORE: OnceLock<Store> = OnceLock::new();

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The current settings (defaults before [`init`]).
pub fn current() -> Settings {
    STORE
        .get()
        .map(|s| lock(&s.settings).clone())
        .unwrap_or_default()
}

/// `<app config dir>/settings.json` (e.g. `%APPDATA%\dev.longfin.quicktranslate\settings.json`,
/// `~/.config/dev.longfin.quicktranslate/settings.json`).
pub fn path<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("settings.json"))
}

/// What the Settings window shows; the payload of `settings://changed`.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    settings: Settings,
    /// Effective target / fallback (the system languages while the settings leave them unset).
    target: String,
    fallback: String,
    languages: &'static [&'static str],
    /// The clipboard watcher in use (`trigger::Backend`), e.g. "Windows", "X11", "Unsupported".
    clipboard: String,
    /// Why the hotkey isn't registered although it is the trigger.
    hotkey_error: Option<String>,
    path: String,
    /// "windows", "linux" or "macos".
    platform: &'static str,
}

fn view<R: Runtime>(app: &AppHandle<R>) -> View {
    let settings = current();
    let (target, fallback) = panel::language_defaults();
    View {
        settings,
        target,
        fallback,
        languages: &panel::LANGUAGES,
        clipboard: trigger::backend()
            .map(|b| format!("{b:?}"))
            .unwrap_or_default(),
        hotkey_error: shortcut::last_error(),
        path: path(app).map(|p| p.display().to_string()).unwrap_or_default(),
        platform: std::env::consts::OS,
    }
}

// ---------------------------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------------------------

/// Called from `setup` (main thread) before the trigger starts: loads the file, applies it and
/// starts the watcher.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    let path = path(app);
    let mut settings = path.as_deref().map(Settings::load).unwrap_or_default();
    let mut written = None;
    // The OS autostart entry is the truth (the user may have removed it); keep the file in step.
    if let Some(real) = autostart_enabled(app) {
        if real != settings.launch_at_login {
            log::write(format!("settings: launchAtLogin follows the system: {real}"));
            settings.launch_at_login = real;
            written = path.as_deref().and_then(|p| save(&settings, p));
        }
    }
    log::write(format!(
        "settings: {} backend={:?} model={:?} keepWarm={} trigger={} interval={}s",
        path.as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "none".into()),
        settings.backend,
        settings.current_model(),
        settings.keep_claude_warm,
        if settings.hotkey_double_press {
            "double copy".to_string()
        } else {
            format!("hotkey {}", settings.hotkey)
        },
        settings.double_press_interval
    ));
    let _ = STORE.set(Store {
        path: path.clone(),
        settings: Mutex::new(settings.clone()),
        written: Mutex::new(written),
        commit: Mutex::new(()),
    });
    apply(app, None, &settings);
    watch_window(app);
    if let Some(path) = path {
        spawn_watcher(app.clone(), path);
    }
}

// ---------------------------------------------------------------------------------------------
// Changing settings
// ---------------------------------------------------------------------------------------------

fn save(settings: &Settings, path: &std::path::Path) -> Option<String> {
    match settings.save(path) {
        Ok(text) => Some(text),
        Err(e) => {
            log::write(format!("settings: saving {} failed: {e}", path.display()));
            None
        }
    }
}

/// Where a change comes from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin {
    /// The Settings window: save it.
    Window,
    /// The file, edited by hand: already saved.
    File,
    /// The OS autostart state: save it, but don't touch autostart.
    System,
}

/// Makes `new` the current settings: autostart first (it can fail), then save, apply, and tell
/// the windows. An autostart failure keeps the old value and comes back as the error, after the
/// other changes are committed.
fn commit<R: Runtime>(app: &AppHandle<R>, mut new: Settings, origin: Origin) -> Result<View, String> {
    let Some(store) = STORE.get() else {
        return Err("settings not loaded".into());
    };
    let _serial = lock(&store.commit);
    let old = current();
    let mut error = None;
    let mut needs_save = origin != Origin::File;
    if new.launch_at_login != old.launch_at_login && origin != Origin::System {
        match set_autostart(app, new.launch_at_login) {
            Ok(real) if real == new.launch_at_login => {}
            Ok(real) => {
                new.launch_at_login = real;
                needs_save = true;
            }
            Err(e) => {
                error = Some(e);
                new.launch_at_login = old.launch_at_login;
                needs_save = true;
            }
        }
    }
    if needs_save {
        if let Some(path) = &store.path {
            match new.save(path) {
                Ok(text) => *lock(&store.written) = Some(text),
                Err(e) => {
                    log::write(format!("settings: saving {} failed: {e}", path.display()));
                    error.get_or_insert(format!("Couldn't save {}: {e}", path.display()));
                }
            }
        }
    }
    *lock(&store.settings) = new.clone();
    apply(app, Some(&old), &new);
    let view = view(app);
    let _ = app.emit("settings://changed", view.clone());
    match error {
        Some(e) => Err(e),
        None => Ok(view),
    }
}

/// Applies what differs between `old` and `new` (everything when `old` is `None`, at startup).
fn apply<R: Runtime>(app: &AppHandle<R>, old: Option<&Settings>, new: &Settings) {
    let changed = |f: fn(&Settings) -> String| old.is_none_or(|o| f(o) != f(new));

    trigger::set_interval(Duration::from_secs_f64(new.double_press_interval));

    if changed(|s| format!("{}{}", s.hotkey_double_press, s.hotkey)) {
        trigger::set_double_copy(new.hotkey_double_press);
        let _ = shortcut::configure(app, !new.hotkey_double_press, &new.hotkey);
    }

    if old.is_none_or(|o| o.warm_worker_changed(new)) {
        let settings = new.clone();
        // `prewarm` may look for the CLI with a login shell: off the main thread.
        tauri::async_runtime::spawn(async move { translation_engine::prewarm(&settings) });
    }

    if old.is_some_and(|o| o.ui_language != new.ui_language) {
        let locale = i18n::locale(&new.ui_language);
        log::write(format!("settings: interface language {} ({locale})", new.ui_language));
        tray::relabel(app, locale);
        if let Some(w) = app.get_webview_window(WINDOW) {
            let _ = w.set_title(&i18n::t(locale, "QuickTranslate Settings"));
        }
    }
}

/// Polls the file's modification time; an edit by hand (or another tool) is loaded and applied
/// like a change from the window.
fn spawn_watcher<R: Runtime>(app: AppHandle<R>, path: PathBuf) {
    let stamp = |p: &PathBuf| -> Option<(SystemTime, u64)> {
        let meta = std::fs::metadata(p).ok()?;
        Some((meta.modified().ok()?, meta.len()))
    };
    let mut last = stamp(&path);
    let spawned = std::thread::Builder::new()
        .name("settings-watch".into())
        .spawn(move || loop {
            std::thread::sleep(WATCH_INTERVAL);
            let now = stamp(&path);
            if now == last {
                continue;
            }
            last = now;
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue; // deleted: keep the current settings
            };
            let ours = STORE
                .get()
                .is_some_and(|s| lock(&s.written).as_deref() == Some(text.as_str()));
            if ours {
                continue;
            }
            let loaded = Settings::load(&path);
            if loaded == current() {
                continue;
            }
            log::write(format!("settings: {} changed on disk, reloading", path.display()));
            if let Err(e) = commit(&app, loaded, Origin::File) {
                log::write(format!("settings: reload: {e}"));
            }
        });
    if let Err(e) = spawned {
        log::write(format!("settings: could not start the file watcher: {e}"));
    }
}

// ---------------------------------------------------------------------------------------------
// Autostart (HKCU Run key on Windows, XDG autostart `.desktop` on Linux)
// ---------------------------------------------------------------------------------------------

fn autostart_enabled<R: Runtime>(app: &AppHandle<R>) -> Option<bool> {
    match app.autolaunch().is_enabled() {
        Ok(on) => Some(on),
        Err(e) => {
            log::write(format!("settings: autostart state unknown: {e}"));
            None
        }
    }
}

/// Turns autostart on or off and returns the state the OS reports afterwards.
fn set_autostart<R: Runtime>(app: &AppHandle<R>, on: bool) -> Result<bool, String> {
    let manager = app.autolaunch();
    let result = if on { manager.enable() } else { manager.disable() };
    match result {
        Ok(()) => {
            let real = manager.is_enabled().unwrap_or(on);
            log::write(format!("settings: run at startup {}", if real { "on" } else { "off" }));
            Ok(real)
        }
        Err(e) => {
            log::write(format!("settings: changing run at startup failed: {e}"));
            Err(e.to_string())
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The window
// ---------------------------------------------------------------------------------------------

/// Shows and focuses the Settings window (tray "Settings…", `quicktranslate --settings`).
pub fn open<R: Runtime>(app: &AppHandle<R>) {
    let Some(w) = app.get_webview_window(WINDOW) else {
        log::write("settings: window missing");
        return;
    };
    let locale = i18n::locale(&current().ui_language);
    let _ = w.set_title(&i18n::t(locale, "QuickTranslate Settings"));
    // Reflect the OS autostart state (the user may have changed it elsewhere) before showing.
    if let Some(real) = autostart_enabled(app) {
        if real != current().launch_at_login {
            let mut s = current();
            s.launch_at_login = real;
            let _ = commit(app, s, Origin::System); // emits the view
        }
    }
    let _ = app.emit_to(WINDOW, "settings://changed", view(app));
    let _ = w.unminimize();
    let _ = w.show();
    let _ = w.set_focus();
    log::write("settings: window shown");
}

/// Closing the window hides it (it is pre-created and reused).
fn watch_window<R: Runtime>(app: &AppHandle<R>) {
    let Some(w) = app.get_webview_window(WINDOW) else {
        return;
    };
    let handle = w.clone();
    w.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = handle.hide();
            log::write("settings: window hidden");
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Commands (registered in `crate::handler`)
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn settings_get<R: Runtime>(app: AppHandle<R>) -> View {
    view(&app)
}

/// Changes the given keys (Swift names, e.g. `{"doublePressInterval": 0.5}`); `null` resets a
/// key to its default. Returns the new view, or the first problem (a bad value, autostart).
#[tauri::command]
pub async fn settings_update<R: Runtime>(
    app: AppHandle<R>,
    patch: Map<String, Value>,
) -> Result<View, String> {
    let mut problems = Vec::new();
    let new = current().merged(&patch, |key, e| problems.push(format!("{key}: {e}")));
    let keys: Vec<&String> = patch.keys().collect();
    log::write(format!("settings: update {keys:?}"));
    let result = commit(&app, new, Origin::Window);
    match problems.into_iter().next() {
        Some(p) => Err(p),
        None => result,
    }
}

/// The hotkey recorder: checks and (in hotkey mode) registers `binding` before saving it, so a
/// shortcut another app holds is reported and the previous one stays.
#[tauri::command]
pub async fn settings_set_hotkey<R: Runtime>(
    app: AppHandle<R>,
    binding: String,
) -> Result<View, String> {
    let (_, text) = shortcut::parse(&binding)?;
    if !current().hotkey_double_press {
        shortcut::set_binding(&app, &text)?;
    }
    let mut new = current();
    new.hotkey = text;
    commit(&app, new, Origin::Window)
}

/// The CLI the engine would use for `backend` with `path` as override (blank = auto-detect),
/// without running it; `None` when not found.
#[tauri::command]
pub async fn settings_detect_cli(backend: Backend, path: String) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || {
        cli_locator::reset_cache();
        cli_locator::find(backend.executable_name(), &path).map(|p| p.display().to_string())
    })
    .await
    .ok()
    .flatten()
}

#[tauri::command]
pub fn settings_hide<R: Runtime>(app: AppHandle<R>) {
    if let Some(w) = app.get_webview_window(WINDOW) {
        let _ = w.hide();
        log::write("settings: window hidden (key)");
    }
}
