//! The single-press global shortcut, used when the trigger is "hotkey" instead of Ctrl+C Ctrl+C
//! (`hotkeyDoublePress: false` in the settings). The binding comes from the settings (`hotkey`,
//! e.g. `Ctrl+Shift+T`) and is recorded in the Settings window. Pressing it translates what is
//! already on the clipboard; copying the selection first (the Swift app's
//! `copySelectionAndTranslate`) is not done yet.
//!
//! On Wayland the underlying `global-hotkey` crate only works through XWayland; a registration
//! failure is kept in [`last_error`] for the Settings window and the app keeps running.

use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Runtime};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use super::{fire, Source};
use crate::log;

static PLUGIN: AtomicBool = AtomicBool::new(false);
/// The registered shortcut, if any.
static CURRENT: Mutex<Option<Shortcut>> = Mutex::new(None);
/// Why the last registration failed; cleared by a successful one or by turning the hotkey off.
static ERROR: Mutex<Option<String>> = Mutex::new(None);

fn current() -> std::sync::MutexGuard<'static, Option<Shortcut>> {
    CURRENT.lock().unwrap_or_else(|e| e.into_inner())
}

fn set_error(error: Option<String>) {
    *ERROR.lock().unwrap_or_else(|e| e.into_inner()) = error;
}

/// Why the shortcut isn't registered although it should be, if that's the case.
pub fn last_error() -> Option<String> {
    ERROR.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Parses `Ctrl+Shift+T` style text (also `KeyT`, `ctrl+shift+t`, `Super`, `Alt`) and returns
/// the shortcut with its canonical text. Like the Swift `isUsable`: a shortcut needs Ctrl, Alt
/// or Super unless it is a function key, or it would fire while typing.
pub fn parse(binding: &str) -> Result<(Shortcut, String), String> {
    let shortcut = Shortcut::from_str(binding.trim())
        .map_err(|e| format!("Invalid shortcut {binding:?}: {e}"))?;
    let is_function_key = shortcut.key.to_string().strip_prefix('F').is_some_and(|n| {
        n.parse::<u8>().is_ok()
    });
    let has_command_modifier = shortcut
        .mods
        .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER);
    if !is_function_key && !has_command_modifier {
        return Err(format!(
            "{} would fire while typing: add Ctrl, Alt or {}",
            display(&shortcut),
            SUPER_LABEL
        ));
    }
    Ok((shortcut, display(&shortcut)))
}

#[cfg(target_os = "macos")]
const SUPER_LABEL: &str = "Cmd";
#[cfg(not(target_os = "macos"))]
const SUPER_LABEL: &str = "Super";

/// `Ctrl+Alt+Shift+Super+T`: the modifiers in a fixed order, then the key without the `Key` /
/// `Digit` prefix of its `KeyboardEvent.code` name.
pub fn display(shortcut: &Shortcut) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (m, label) in [
        (Modifiers::CONTROL, "Ctrl"),
        (Modifiers::ALT, "Alt"),
        (Modifiers::SHIFT, "Shift"),
        (Modifiers::SUPER, SUPER_LABEL),
    ] {
        if shortcut.mods.contains(m) {
            parts.push(label.into());
        }
    }
    parts.push(key_label(shortcut.key));
    parts.join("+")
}

fn key_label(code: Code) -> String {
    let name = code.to_string();
    if let Some(rest) = name.strip_prefix("Key").filter(|r| r.len() == 1) {
        return rest.into();
    }
    if let Some(rest) = name.strip_prefix("Digit").filter(|r| r.len() == 1) {
        return rest.into();
    }
    name
}

fn ensure_plugin<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if PLUGIN.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let plugin = tauri_plugin_global_shortcut::Builder::<R>::new()
        .with_handler(|app, shortcut, event| {
            let ours = current().is_some_and(|s| s.id() == shortcut.id());
            if ours && event.state() == ShortcutState::Pressed {
                log::write("trigger: hotkey pressed");
                fire(app, Source::Hotkey);
            }
        })
        .build();
    app.plugin(plugin).map_err(|e| {
        PLUGIN.store(false, Ordering::SeqCst);
        format!("global shortcut plugin failed: {e}")
    })
}

fn unregister_current<R: Runtime>(app: &AppHandle<R>) {
    let old = current().take();
    if let Some(old) = old {
        match app.global_shortcut().unregister(old) {
            Ok(()) => log::write(format!("trigger: hotkey {} off", display(&old))),
            Err(e) => log::write(format!("trigger: unregistering hotkey {} failed: {e}", display(&old))),
        }
    }
}

fn register<R: Runtime>(app: &AppHandle<R>, shortcut: Shortcut) -> Result<(), String> {
    ensure_plugin(app)?;
    app.global_shortcut()
        .register(shortcut)
        .map_err(|e| e.to_string())?;
    *current() = Some(shortcut);
    Ok(())
}

/// Turns the shortcut on with `binding`, or off. Failures are logged and kept in
/// [`last_error`]; the result says whether the hotkey is now live as asked.
/// The first call (from `setup`, on the main thread) also installs the plugin: on Windows its
/// hidden message window belongs to the thread that creates it.
pub fn configure<R: Runtime>(app: &AppHandle<R>, enabled: bool, binding: &str) -> Result<(), String> {
    if let Err(e) = ensure_plugin(app) {
        log::write(format!("trigger: {e}"));
        set_error(Some(e.clone()));
        return Err(e);
    }
    if enabled {
        let wanted = parse(binding).ok().map(|(s, _)| s.id());
        if wanted.is_some() && current().map(|s| s.id()) == wanted {
            return Ok(()); // already live
        }
    }
    unregister_current(app);
    if !enabled {
        set_error(None);
        return Ok(());
    }
    let result = parse(binding).and_then(|(shortcut, text)| {
        register(app, shortcut).map_err(|e| format!("{text}: {e}"))?;
        Ok(text)
    });
    match result {
        Ok(text) => {
            set_error(None);
            log::write(format!("trigger: hotkey {text} on"));
            Ok(())
        }
        Err(e) => {
            log::write(format!("trigger: hotkey unavailable: {e}"));
            set_error(Some(e.clone()));
            Err(e)
        }
    }
}

/// Replaces the live shortcut with `binding` (the Settings recorder). On failure the previous
/// one is registered again and the error returned, so a bad recording loses nothing.
pub fn set_binding<R: Runtime>(app: &AppHandle<R>, binding: &str) -> Result<String, String> {
    let (shortcut, text) = parse(binding)?;
    let previous = *current();
    if previous.is_some_and(|p| p.id() == shortcut.id()) {
        return Ok(text);
    }
    unregister_current(app);
    match register(app, shortcut) {
        Ok(()) => {
            set_error(None);
            log::write(format!("trigger: hotkey changed to {text}"));
            Ok(text)
        }
        Err(e) => {
            log::write(format!("trigger: hotkey {text} unavailable: {e}"));
            if let Some(previous) = previous {
                if let Err(e2) = register(app, previous) {
                    set_error(Some(e2));
                }
            }
            Err(format!("{text}: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_canonicalizes() {
        let (_, text) = parse("ctrl+shift+t").unwrap();
        assert_eq!(text, "Ctrl+Shift+T");
        let (_, text) = parse("Shift+Ctrl+KeyT").unwrap();
        assert_eq!(text, "Ctrl+Shift+T");
        let (_, text) = parse("Alt+Digit1").unwrap();
        assert_eq!(text, "Alt+1");
        let (_, text) = parse("Ctrl+Alt+Space").unwrap();
        assert_eq!(text, "Ctrl+Alt+Space");
        let (_, text) = parse("F8").unwrap();
        assert_eq!(text, "F8");
        let (_, text) = parse("Shift+F12").unwrap();
        assert_eq!(text, "Shift+F12");
    }

    #[test]
    fn rejects_typing_keys_and_garbage() {
        assert!(parse("Shift+T").is_err());
        assert!(parse("T").is_err());
        assert!(parse("Ctrl+").is_err());
        assert!(parse("Ctrl+Shift+Banana").is_err());
    }

    #[test]
    fn same_text_same_id() {
        let a = parse("Ctrl+Shift+T").unwrap().0;
        let b = parse("shift+control+KeyT").unwrap().0;
        assert_eq!(a.id(), b.id());
    }
}
