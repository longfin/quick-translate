//! macOS preview build: the translation window as a non-activating `NSPanel`, like the Swift
//! app's `.nonactivatingPanel` style. Showing it does not activate the app, so the frontmost app
//! keeps its focus and its menu bar; the panel still becomes key, so Esc / Cmd+W reach it and a
//! click elsewhere takes the key status away ("close when clicking outside").
//!
//! The shipped macOS app is still the Swift one in `macos/`; this exists so the Tauri app can be
//! tried on a Mac as a preview (see the README).

use tauri::{AppHandle, Manager, Runtime};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt as _, PanelLevel, StyleMask, WebviewWindowExt as _,
};

use crate::log;

tauri_panel! {
    panel!(TranslationPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
            is_floating_panel: true,
            hides_on_deactivate: false
        }
    })
}

/// Converts the pre-created window into the panel: floating, on every Space including
/// full-screen ones, non-activating. Call once, after the windows exist.
pub fn make_panel<R: Runtime>(app: &AppHandle<R>, label: &str) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(label) else {
        return Ok(());
    };
    let panel = window.to_panel::<TranslationPanel<R>>()?;
    if let Err(e) = panel.add_style_mask(StyleMask::empty().nonactivating_panel().value()) {
        log::write(format!("panel: non-activating style mask rejected: {e:?}"));
    }
    panel.set_level(PanelLevel::Floating.value());
    panel.set_collection_behavior(
        CollectionBehavior::new().can_join_all_spaces().full_screen_auxiliary().value(),
    );
    log::write("panel: converted to a non-activating NSPanel");
    Ok(())
}

/// Shows the panel and makes it key without activating the app. AppKit calls must run on the
/// main thread, and the panel is shown from the trigger's worker thread, so this queues the work
/// there (asynchronously, like Tauri's own window calls from other threads). Falls back to the
/// plain window calls if the window was never converted.
pub fn show<R: Runtime>(app: &AppHandle<R>, label: &str) {
    let handle = app.clone();
    let label = label.to_string();
    let queued = app.run_on_main_thread(move || match handle.get_webview_panel(&label) {
        Ok(panel) => panel.show_and_make_key(),
        Err(_) => {
            if let Some(w) = handle.get_webview_window(&label) {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
    });
    if let Err(e) = queued {
        log::write(format!("panel: could not reach the main thread: {e}"));
    }
}

/// Brings the app to the front for a regular window (Settings). As an Accessory app, showing a
/// window does not activate it, so the window would open behind the frontmost app.
pub fn activate<R: Runtime>(app: &AppHandle<R>) {
    let queued = app.run_on_main_thread(|| {
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            #[allow(deprecated)] // `activate()` (macOS 14+) ignores the request from a background app
            objc2_app_kit::NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
        }
    });
    if let Err(e) = queued {
        log::write(format!("settings: could not reach the main thread: {e}"));
    }
}
