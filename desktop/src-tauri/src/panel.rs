//! The translation panel (Phase 3, issue #3): the pre-created hidden `panel` window
//! (`tauri.conf.json`), shown near the cursor for every trigger. Port of
//! `macos/Sources/QuickTranslate/TranslationPanel.swift` and the language rules of
//! `TranslationViewModel.swift`.
//!
//! The panel's state (text, languages, pin) lives here; the web UI (`src/main.ts`) renders it.
//! Every (re)translation goes through [`restart`], which announces the engine's id with
//! `panel://start` before any `translate://*` event of that id can reach the webview.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, Once};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime, WebviewWindow, WindowEvent};

use crate::engine::commands::{self as engine, load_settings, settings_path};
use crate::engine::settings::Backend;
use crate::trigger::Trigger;
use crate::{log, PANEL};

/// Target languages, in menu order. Same list (and English names, which go into the prompt) as
/// `AppSettings.languages` in the Swift app. The web UI gets it from [`panel_init`].
pub const LANGUAGES: [&str; 16] = [
    "Korean",
    "English",
    "Japanese",
    "Chinese (Simplified)",
    "Chinese (Traditional)",
    "Spanish",
    "French",
    "German",
    "Portuguese",
    "Italian",
    "Russian",
    "Vietnamese",
    "Thai",
    "Indonesian",
    "Hindi",
    "Arabic",
];

/// Distance between the cursor and the panel's top-left corner, in logical pixels (Swift: 12).
const CURSOR_OFFSET: f64 = 12.0;

/// Focus changes this soon after showing are the window manager settling, not a click outside.
const BLUR_GRACE_MS: u128 = 300;

/// The language of `text`, as one of [`LANGUAGES`], or `None` when unsure (the model then
/// detects it itself).
pub fn source_language(_text: &str) -> Option<String> {
    // TODO(Phase 5): engine::language::detect
    None
}

/// Whether an unpinned panel hides when it loses focus (Swift `closeOnOutsideClick`).
fn close_on_outside_click() -> bool {
    // TODO(Phase 4): read `closeOnOutsideClick` from the settings store (default true).
    true
}

// ---------------------------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct Panel {
    /// Clipboard text as triggered (shown untrimmed, translated trimmed).
    text: String,
    /// Source picked in the panel; `None` = auto (detected or left to the model).
    source: Option<String>,
    detected: Option<String>,
    target: String,
    /// Engine id of the current translation; 0 = none.
    id: u64,
    shown_at: Option<Instant>,
}

static PANEL_STATE: Mutex<Panel> = Mutex::new(Panel {
    text: String::new(),
    source: None,
    detected: None,
    target: String::new(),
    id: 0,
    shown_at: None,
});
static PINNED: AtomicBool = AtomicBool::new(false);
/// Kept alive for the app's lifetime: on X11 the process serves the clipboard it wrote, and
/// arboard stops doing so when the last `Clipboard` is dropped.
static CLIPBOARD: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);
static WINDOW_EVENTS: Once = Once::new();

fn state() -> MutexGuard<'static, Panel> {
    PANEL_STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// What `panel://start` carries and `panel_init` returns.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// Engine id whose `translate://*` events belong to this view; 0 when nothing runs.
    id: u64,
    text: String,
    source: Option<String>,
    detected: Option<String>,
    target: String,
    fallback: String,
    /// Nothing to translate (empty clipboard): the engine was not called.
    empty: bool,
    /// "Claude · haiku" for the footer.
    engine: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Init {
    languages: &'static [&'static str],
    pinned: bool,
    is_mac: bool,
    /// The current view, if a trigger came before the webview loaded.
    view: Option<View>,
}

// ---------------------------------------------------------------------------------------------
// Languages
// ---------------------------------------------------------------------------------------------

/// The `targetLanguage` / `fallbackLanguage` keys of the Swift `AppSettings`.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct LanguageSettings {
    target_language: Option<String>,
    fallback_language: Option<String>,
}

/// (target, fallback): the settings file's, else derived from the system languages like Swift's
/// `AppSettings.systemDefaultLanguages()`.
fn language_defaults<R: Runtime>(app: &AppHandle<R>) -> (String, String) {
    // TODO(Phase 4): read these from the settings store with the other keys.
    let file: LanguageSettings = settings_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let known = |l: Option<String>| l.filter(|l| LANGUAGES.contains(&l.as_str()));
    let (sys_target, sys_fallback) = system_default_languages(sys_locale::get_locales());
    let target = known(file.target_language).unwrap_or(sys_target);
    let fallback = known(file.fallback_language).unwrap_or(sys_fallback);
    (target, fallback)
}

/// Swift `systemDefaultLanguages()`: translate into the first preferred system language we know;
/// its fallback is the next one, else English (or Korean when the target is English).
fn system_default_languages(locales: impl IntoIterator<Item = String>) -> (String, String) {
    let mut found: Vec<&'static str> = Vec::new();
    for locale in locales {
        if let Some(name) = language_for_locale(&locale) {
            if !found.contains(&name) {
                found.push(name);
            }
        }
    }
    let target = found.first().copied().unwrap_or("English");
    let fallback = found.get(1).copied().unwrap_or(if target == "English" {
        "Korean"
    } else {
        "English"
    });
    (target.into(), fallback.into())
}

/// `ko-KR`, `ko_KR.UTF-8`, `zh-Hant-TW`, `zh_TW` → a [`LANGUAGES`] name.
fn language_for_locale(locale: &str) -> Option<&'static str> {
    let locale = locale
        .split(['.', '@'])
        .next()?
        .replace('_', "-")
        .to_ascii_lowercase();
    let mut parts = locale.split('-');
    let lang = parts.next()?;
    Some(match lang {
        "ko" => "Korean",
        "en" => "English",
        "ja" => "Japanese",
        "zh" => {
            let rest: Vec<&str> = parts.collect();
            let traditional = rest
                .iter()
                .any(|p| matches!(*p, "hant" | "tw" | "hk" | "mo"))
                && !rest.contains(&"hans");
            if traditional {
                "Chinese (Traditional)"
            } else {
                "Chinese (Simplified)"
            }
        }
        "es" => "Spanish",
        "fr" => "French",
        "de" => "German",
        "pt" => "Portuguese",
        "it" => "Italian",
        "ru" => "Russian",
        "vi" => "Vietnamese",
        "th" => "Thai",
        "id" => "Indonesian",
        "hi" => "Hindi",
        "ar" => "Arabic",
        _ => return None,
    })
}

/// The fallback handed to the engine: the other of (target, fallback) settings, like Swift.
fn fallback_for(target: &str, defaults: &(String, String)) -> String {
    if target == defaults.1 {
        defaults.0.clone()
    } else {
        defaults.1.clone()
    }
}

/// Swift `translate(text:)`: translate into the target language unless the text is already in
/// it, then into the fallback.
fn target_for(detected: Option<&str>, defaults: &(String, String)) -> String {
    if detected == Some(defaults.0.as_str()) {
        defaults.1.clone()
    } else {
        defaults.0.clone()
    }
}

fn engine_label<R: Runtime>(app: &AppHandle<R>) -> String {
    let settings = load_settings(app);
    let name = match settings.backend {
        Backend::Claude => "Claude",
        Backend::Codex => "ChatGPT",
    };
    match settings.current_model().trim() {
        "" => name.into(),
        model => format!("{name} · {model}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Showing and hiding
// ---------------------------------------------------------------------------------------------

/// Entry point for every trigger (via [`crate::on_trigger`]): show the panel near the cursor
/// and translate `trigger.text`. While the panel is visible, a new trigger replaces the text and
/// restarts the translation without moving the panel.
pub fn show_for<R: Runtime>(app: &AppHandle<R>, trigger: Trigger) {
    let started = Instant::now();
    let detected = if trigger.text.trim().is_empty() {
        None
    } else {
        source_language(&trigger.text)
    };
    let defaults = language_defaults(app);
    {
        let mut st = state();
        st.target = target_for(detected.as_deref(), &defaults);
        st.text = trigger.text;
        st.source = None;
        st.detected = detected;
    }
    restart(app, Some(started));
}

/// (Re)starts the translation of the current state and announces it with `panel://start`.
/// With `show`, also shows the panel once the webview has the new text (`show` = trigger time).
fn restart<R: Runtime>(app: &AppHandle<R>, show: Option<Instant>) {
    let app = app.clone();
    // The engine spawns tokio tasks: run inside the async runtime (triggers come from threads).
    tauri::async_runtime::spawn(async move {
        let defaults = language_defaults(&app);
        let (text, source, detected, target) = {
            let st = state();
            (
                st.text.clone(),
                st.source.clone(),
                st.detected.clone(),
                st.target.clone(),
            )
        };
        let fallback = fallback_for(&target, &defaults);
        let view = |id: u64, empty: bool| View {
            id,
            text: text.clone(),
            source: source.clone(),
            detected: detected.clone(),
            target: target.clone(),
            fallback: fallback.clone(),
            empty,
            engine: engine_label(&app),
        };
        let announce = |view: View| {
            state().id = view.id;
            let _ = app.emit_to(PANEL, "panel://start", view);
            if let Some(started) = show {
                show_window(&app, started);
            }
        };

        let trimmed = text.trim();
        if trimmed.is_empty() {
            engine::cancel_current(&app);
            log::write("panel: nothing to translate");
            announce(view(0, true));
            return;
        }
        let source = source.as_deref().or(detected.as_deref());
        engine::start(&app, trimmed, source, &target, &fallback, |id| {
            announce(view(id, false))
        });
    });
}

fn window<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    let w = app.get_webview_window(PANEL);
    if w.is_none() {
        log::write("panel: window missing");
    }
    w
}

fn show_window<R: Runtime>(app: &AppHandle<R>, started: Instant) {
    let Some(w) = window(app) else { return };
    WINDOW_EVENTS.call_once(|| {
        let handle = app.clone();
        w.on_window_event(move |event| {
            if let WindowEvent::Focused(false) = event {
                on_blur(&handle);
            }
        });
    });

    let visible = w.is_visible().unwrap_or(false);
    if !visible {
        position_near_cursor(app, &w);
    }
    // Raise above whatever has focus (Windows refuses a plain SetForegroundWindow from a
    // background process), then keep "always on top" only while pinned.
    let pinned = PINNED.load(Ordering::SeqCst);
    let _ = w.set_always_on_top(true);
    let _ = w.show();
    let _ = w.unminimize();
    // Focus is needed for Esc / Ctrl+W and for "close when clicking outside" (blur).
    let _ = w.set_focus();
    if !pinned {
        let _ = w.set_always_on_top(false);
    }
    state().shown_at = Some(Instant::now());
    log::write(format!(
        "panel: shown ({}) {} ms after trigger",
        if visible {
            "already visible"
        } else {
            "near cursor"
        },
        started.elapsed().as_millis()
    ));
}

/// Hides the panel and cancels its translation (Esc, Ctrl+W, click outside).
pub fn hide<R: Runtime>(app: &AppHandle<R>, why: &str) {
    engine::cancel_current(app);
    if let Some(w) = window(app) {
        let _ = w.hide();
    }
    log::write(format!("panel: hidden ({why})"));
}

fn on_blur<R: Runtime>(app: &AppHandle<R>) {
    if PINNED.load(Ordering::SeqCst) || !close_on_outside_click() {
        return;
    }
    if state()
        .shown_at
        .is_some_and(|t| t.elapsed().as_millis() < BLUR_GRACE_MS)
    {
        return;
    }
    if !window(app).is_some_and(|w| w.is_visible().unwrap_or(false)) {
        return;
    }
    // Like Swift: keep the panel up until the translation is done.
    if engine::is_running(app) {
        log::write("panel: lost focus while translating, keeping it");
        return;
    }
    hide(app, "click outside");
}

/// Swift `positionNearMouse()`: top-left 12 px right of and below the cursor, clamped into the
/// work area of the monitor under the cursor. All in physical pixels; the window's size is
/// rescaled to that monitor's DPI since the OS keeps its logical size when it changes monitor.
fn position_near_cursor<R: Runtime>(app: &AppHandle<R>, w: &WebviewWindow<R>) {
    let Ok(cursor) = app.cursor_position() else {
        log::write("panel: cursor position unavailable, keeping the last position");
        return;
    };
    let monitor = app
        .monitor_from_point(cursor.x, cursor.y)
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    let (Some(monitor), Ok(size)) = (monitor, w.outer_size()) else {
        return;
    };
    let scale = monitor.scale_factor();
    let rescale = scale / w.scale_factor().unwrap_or(scale);
    let area = monitor.work_area();
    let (x, y) = place(
        (cursor.x, cursor.y),
        (size.width as f64 * rescale, size.height as f64 * rescale),
        (
            area.position.x as f64,
            area.position.y as f64,
            area.size.width as f64,
            area.size.height as f64,
        ),
        CURSOR_OFFSET * scale,
    );
    let _ = w.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

/// Top-left corner for a window of `size` placed `offset` right of and below `cursor`, clamped
/// into `area` (x, y, width, height). A window larger than the area sticks to its top-left.
fn place(
    cursor: (f64, f64),
    size: (f64, f64),
    area: (f64, f64, f64, f64),
    offset: f64,
) -> (f64, f64) {
    let (ax, ay, aw, ah) = area;
    let x = (cursor.0 + offset).min(ax + aw - size.0).max(ax);
    let y = (cursor.1 + offset).min(ay + ah - size.1).max(ay);
    (x, y)
}

// ---------------------------------------------------------------------------------------------
// Commands (registered in `crate::handler`)
// ---------------------------------------------------------------------------------------------

/// The language list, pin state and the current view (if a trigger beat the webview's load).
#[tauri::command]
pub fn panel_init<R: Runtime>(app: AppHandle<R>) -> Init {
    let view = {
        let st = state();
        (!st.target.is_empty()).then(|| {
            (
                st.id,
                st.text.clone(),
                st.source.clone(),
                st.detected.clone(),
                st.target.clone(),
            )
        })
    };
    let view = view.map(|(id, text, source, detected, target)| {
        let defaults = language_defaults(&app);
        View {
            id,
            empty: text.trim().is_empty(),
            fallback: fallback_for(&target, &defaults),
            text,
            source,
            detected,
            target,
            engine: engine_label(&app),
        }
    });
    Init {
        languages: &LANGUAGES,
        pinned: PINNED.load(Ordering::SeqCst),
        is_mac: cfg!(target_os = "macos"),
        view,
    }
}

/// Picker change or swap: translate the same text with `source` (`None` = auto) → `target`.
#[tauri::command]
pub fn panel_retranslate<R: Runtime>(app: AppHandle<R>, source: Option<String>, target: String) {
    if !LANGUAGES.contains(&target.as_str()) {
        return;
    }
    {
        let mut st = state();
        st.source = source.filter(|s| LANGUAGES.contains(&s.as_str()));
        st.target = target;
    }
    restart(&app, None);
}

#[tauri::command]
pub fn panel_hide<R: Runtime>(app: AppHandle<R>) {
    hide(&app, "key");
}

/// Pinned: always on top and not hidden by clicking outside.
#[tauri::command]
pub fn panel_set_pinned<R: Runtime>(app: AppHandle<R>, pinned: bool) {
    PINNED.store(pinned, Ordering::SeqCst);
    if let Some(w) = window(&app) {
        let _ = w.set_always_on_top(pinned);
    }
    log::write(format!("panel: pinned={pinned}"));
}

/// The copy button. Our own write must not count as the first Ctrl+C of a double copy.
#[tauri::command]
pub fn panel_copy(text: String) -> Result<(), String> {
    let mut slot = CLIPBOARD.lock().unwrap_or_else(|e| e.into_inner());
    if slot.is_none() {
        *slot = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
    }
    crate::trigger::suppress_next(1);
    let result = slot.as_mut().map(|c| c.set_text(text));
    match result {
        Some(Ok(())) => Ok(()),
        Some(Err(e)) => {
            log::write(format!("panel: copy failed: {e}"));
            Err(e.to_string())
        }
        None => Err("clipboard unavailable".into()),
    }
}

/// Lets the webview log what it painted, for the trigger → first delta timing.
#[tauri::command]
pub fn panel_mark(what: String) {
    let shown = state().shown_at;
    match shown {
        Some(t) => log::write(format!(
            "panel: {what} ({} ms after show)",
            t.elapsed().as_millis()
        )),
        None => log::write(format!("panel: {what}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: (f64, f64, f64, f64) = (0.0, 25.0, 1440.0, 875.0);

    #[test]
    fn placed_below_right_of_cursor() {
        assert_eq!(
            place((100.0, 100.0), (480.0, 340.0), AREA, 12.0),
            (112.0, 112.0)
        );
    }

    #[test]
    fn clamped_into_the_work_area() {
        // Bottom-right corner: pushed left and up.
        assert_eq!(
            place((1430.0, 890.0), (480.0, 340.0), AREA, 12.0),
            (960.0, 560.0)
        );
        // Above the work area (menu bar / top taskbar).
        assert_eq!(place((10.0, 0.0), (480.0, 340.0), AREA, 12.0), (22.0, 25.0));
    }

    #[test]
    fn secondary_monitor_with_negative_origin() {
        let area = (-1920.0, 0.0, 1920.0, 1040.0);
        assert_eq!(
            place((-100.0, 500.0), (960.0, 680.0), area, 24.0),
            (-960.0, 360.0)
        );
    }

    #[test]
    fn too_large_sticks_to_top_left() {
        assert_eq!(
            place((50.0, 50.0), (2000.0, 2000.0), AREA, 12.0),
            (0.0, 25.0)
        );
    }

    #[test]
    fn locales_map_to_languages() {
        assert_eq!(language_for_locale("ko-KR"), Some("Korean"));
        assert_eq!(language_for_locale("ko_KR.UTF-8"), Some("Korean"));
        assert_eq!(language_for_locale("en"), Some("English"));
        assert_eq!(
            language_for_locale("zh-Hant-TW"),
            Some("Chinese (Traditional)")
        );
        assert_eq!(language_for_locale("zh_TW"), Some("Chinese (Traditional)"));
        assert_eq!(
            language_for_locale("zh-Hans-HK"),
            Some("Chinese (Simplified)")
        );
        assert_eq!(language_for_locale("zh-CN"), Some("Chinese (Simplified)"));
        assert_eq!(language_for_locale("C"), None);
        assert_eq!(language_for_locale("nl-NL"), None);
    }

    #[test]
    fn system_defaults_like_swift() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(
            system_default_languages(s(&["ko-KR", "en-US"])),
            ("Korean".into(), "English".into())
        );
        assert_eq!(
            system_default_languages(s(&["en-US", "en-GB"])),
            ("English".into(), "Korean".into())
        );
        assert_eq!(
            system_default_languages(s(&["ja-JP"])),
            ("Japanese".into(), "English".into())
        );
        assert_eq!(
            system_default_languages(s(&["nl-NL"])),
            ("English".into(), "Korean".into())
        );
        assert_eq!(
            system_default_languages(s(&["nl-NL", "de-DE", "fr-FR"])),
            ("German".into(), "French".into())
        );
    }

    #[test]
    fn target_and_fallback_rule() {
        let d = ("Korean".to_string(), "English".to_string());
        // Already in the target language → the fallback.
        assert_eq!(target_for(Some("Korean"), &d), "English");
        assert_eq!(target_for(Some("Japanese"), &d), "Korean");
        assert_eq!(target_for(None, &d), "Korean");
        // The engine's fallback is the other one of the pair.
        assert_eq!(fallback_for("Korean", &d), "English");
        assert_eq!(fallback_for("English", &d), "Korean");
        assert_eq!(fallback_for("Japanese", &d), "English");
    }

    #[test]
    fn languages_match_swift_list() {
        assert_eq!(LANGUAGES.len(), 16);
        assert_eq!(LANGUAGES[0], "Korean");
        assert_eq!(LANGUAGES[15], "Arabic");
    }
}
