//! Tray icon and its menu: Translate clipboard / Settings… / Quit, in the interface language.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Runtime};

use crate::{i18n, log};

const TRANSLATE: &str = "translate";
const SETTINGS: &str = "settings";
const QUIT: &str = "quit";

/// The menu items whose text follows `uiLanguage`.
struct Items<R: Runtime> {
    translate: MenuItem<R>,
    settings: MenuItem<R>,
    quit: MenuItem<R>,
}

pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let locale = i18n::locale(&crate::settings::current().ui_language);
    let tr = |key: &str| i18n::t(locale, key);
    let items = Items {
        translate: MenuItem::with_id(app, TRANSLATE, tr("Translate clipboard"), true, None::<&str>)?,
        settings: MenuItem::with_id(app, SETTINGS, tr("Settings…"), true, None::<&str>)?,
        quit: MenuItem::with_id(app, QUIT, tr("Quit"), true, None::<&str>)?,
    };
    let menu = Menu::with_items(
        app,
        &[
            &items.translate,
            &items.settings,
            &PredefinedMenuItem::separator(app)?,
            &items.quit,
        ],
    )?;
    app.manage(items);

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("QuickTranslate")
        .menu(&menu)
        // Linux (AppIndicator) always opens the menu on click; match that elsewhere.
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            TRANSLATE => translate_clipboard(app),
            SETTINGS => crate::settings::open(app),
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

/// Re-labels the menu after an interface language change.
pub fn relabel<R: Runtime>(app: &AppHandle<R>, locale: &str) {
    let Some(items) = app.try_state::<Items<R>>() else {
        return;
    };
    for (item, key) in [
        (&items.translate, "Translate clipboard"),
        (&items.settings, "Settings…"),
        (&items.quit, "Quit"),
    ] {
        let _ = item.set_text(i18n::t(locale, key));
    }
}

pub fn translate_clipboard<R: Runtime>(app: &AppHandle<R>) {
    crate::trigger::fire(app, crate::trigger::Source::Tray);
}
