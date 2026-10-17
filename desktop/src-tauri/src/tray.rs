//! Tray icon and its menu: Translate clipboard / Settings… / Quit.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Runtime};

use crate::log;

const TRANSLATE: &str = "translate";
const SETTINGS: &str = "settings";
const QUIT: &str = "quit";

pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, TRANSLATE, "Translate clipboard", true, None::<&str>)?,
            &MenuItem::with_id(app, SETTINGS, "Settings…", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, QUIT, "Quit", true, None::<&str>)?,
        ],
    )?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("QuickTranslate")
        .menu(&menu)
        // Linux (AppIndicator) always opens the menu on click; match that elsewhere.
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            TRANSLATE => translate_clipboard(app),
            SETTINGS => open_settings(app),
            QUIT => {
                log::write("quit from tray");
                app.exit(0);
            }
            other => log::write(format!("unknown tray menu item: {other}")),
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    log::write("tray ready");
    Ok(())
}

pub fn translate_clipboard<R: Runtime>(app: &AppHandle<R>) {
    crate::trigger::fire(app, crate::trigger::Source::Tray);
}

/// Phase 4 opens the settings window.
fn open_settings<R: Runtime>(_app: &AppHandle<R>) {
    log::write("TODO open settings");
}
