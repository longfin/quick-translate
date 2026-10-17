//! QuickTranslate for Windows and Linux (issue #3). A tray-only app: no window at startup,
//! nothing in the taskbar. The translation panel and the Settings window are pre-created hidden
//! (`tauri.conf.json`, labels `panel` and `settings`) so showing them later is instant.

mod engine;
mod i18n;
mod log;
mod panel;
mod settings;
mod trigger;
mod tray;

use tauri::RunEvent;

/// Label of the pre-created, hidden translation window.
pub const PANEL: &str = "panel";

/// Every trigger (double copy, hotkey, tray, `--translate`) ends here: shows the panel near the
/// cursor and translates the text (or says there is nothing to translate).
pub fn on_trigger<R: tauri::Runtime>(app: &tauri::AppHandle<R>, trigger: trigger::Trigger) {
    log::write(format!(
        "TRIGGER source={:?} chars={}",
        trigger.source,
        trigger.text.chars().count()
    ));
    panel::show_for(app, trigger);
}

/// Every Tauri command the webviews may invoke, in one list.
fn handler<R: tauri::Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        engine::commands::translate,
        engine::commands::cancel,
        engine::commands::prewarm,
        engine::commands::test_cli,
        panel::panel_init,
        panel::panel_retranslate,
        panel::panel_hide,
        panel::panel_set_pinned,
        panel::panel_copy,
        panel::panel_mark,
        settings::settings_get,
        settings::settings_update,
        settings::settings_set_hotkey,
        settings::settings_detect_cli,
        settings::settings_hide,
    ]
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Must be the first plugin registered.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            log::write(format!("second instance launched, args={:?}", args.get(1..).unwrap_or(&[])));
            // Scripting trigger: `quicktranslate --translate`.
            if args.iter().skip(1).any(|a| a == "--translate") {
                trigger::fire(app, trigger::Source::SecondInstance);
            }
            if args.iter().skip(1).any(|a| a == "--settings") {
                settings::open(app);
            }
        }))
        // Run at startup: HKCU Run key (Windows), XDG autostart `.desktop` (Linux).
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .invoke_handler(handler())
        .setup(|app| {
            // Runs only in the primary instance; a second one exits inside the plugin above.
            log::write(format!(
                "QuickTranslate {} started (log: {})",
                app.package_info().version,
                log::path().map(|p| p.display().to_string()).unwrap_or_else(|| "none".into())
            ));

            // Dev builds only: keep the macOS dock icon away, like the Swift app (LSUIElement).
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            engine::commands::init(app.handle());
            settings::init(app.handle());
            tray::create(app.handle())?;
            trigger::start(app.handle());
            if std::env::args().skip(1).any(|a| a == "--settings") {
                settings::open(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app, event| match event {
        // Closing the last window must not quit a tray app; only the tray's Quit (`exit(0)`,
        // which sets a code) ends it.
        RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
        RunEvent::Exit => {
            engine::commands::shutdown();
            log::write("exit");
        }
        _ => {}
    });
}
