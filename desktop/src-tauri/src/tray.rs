//! Tray icon and its menu: Translate clipboard / Settings… / Quit, in the interface language.

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Runtime};

use crate::{i18n, log};

const TRANSLATE: &str = "translate";
const SETTINGS: &str = "settings";
const QUIT: &str = "quit";

/// Monochrome tray glyph (white + alpha, from `icons/tray.svg`; the full-colour icon stays on the
/// windows and installers). Windows' notification area and most Linux panels are dark, so one
/// white glyph is used everywhere; on a light Linux panel it is hard to see. Decoded at compile
/// time, so the app needs no image decoder. Windows draws it at 16 px (32 px at 200 %); Linux
/// panels scale the larger one.
#[cfg(windows)]
const TRAY_ICON: Image<'static> = tauri::include_image!("icons/tray-32.png");
#[cfg(not(windows))]
const TRAY_ICON: Image<'static> = tauri::include_image!("icons/tray-64.png");

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
    tray = tray.icon(TRAY_ICON);
    // Dev builds on macOS: let the menu bar tint the glyph for light / dark.
    #[cfg(target_os = "macos")]
    {
        tray = tray.icon_as_template(true);
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
