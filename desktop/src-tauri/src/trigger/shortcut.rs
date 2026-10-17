//! Optional single-press global shortcut, off by default. For now it is the fixed `Ctrl+Shift+T`
//! and translates what is already on the clipboard; the recorder in Settings (and copying the
//! selection first, like the Swift app's `copySelectionAndTranslate`) is Phase 4.
//!
//! Enable it for development with `QUICKTRANSLATE_HOTKEY=1`. On Wayland the underlying
//! `global-hotkey` crate only works through XWayland; registration failures are logged and the
//! app keeps running (double copy and the tray still work).

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Runtime};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use super::{fire, Source};
use crate::log;

static PLUGIN: AtomicBool = AtomicBool::new(false);

pub(super) fn start<R: Runtime>(app: &AppHandle<R>) {
    let enabled = std::env::var("QUICKTRANSLATE_HOTKEY").is_ok_and(|v| v == "1");
    if enabled {
        set_enabled(app, true);
    }
}

/// Turn the single-press shortcut on or off (Settings, Phase 4).
pub fn set_enabled<R: Runtime>(app: &AppHandle<R>, on: bool) {
    let result = if on {
        if !PLUGIN.swap(true, Ordering::SeqCst) {
            let plugin = tauri_plugin_global_shortcut::Builder::<R>::new()
                .with_handler(|app, shortcut, event| {
                    if shortcut.id() == hotkey().id() && event.state() == ShortcutState::Pressed {
                        log::write("trigger: hotkey pressed");
                        fire(app, Source::Hotkey);
                    }
                })
                .build();
            if let Err(e) = app.plugin(plugin) {
                log::write(format!("trigger: global shortcut plugin failed: {e}"));
                return;
            }
        }
        app.global_shortcut().register(hotkey())
    } else if PLUGIN.load(Ordering::SeqCst) {
        app.global_shortcut().unregister(hotkey())
    } else {
        Ok(())
    };
    match result {
        Ok(()) => log::write(format!("trigger: hotkey {} {}", hotkey(), if on { "on" } else { "off" })),
        Err(e) => log::write(format!("trigger: hotkey {} unavailable: {e}", hotkey())),
    }
}

fn hotkey() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyT)
}
