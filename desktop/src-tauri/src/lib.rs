//! QuickTranslate for Windows and Linux (issue #3). A tray-only app: no window at startup,
//! nothing in the taskbar. The translation panel is pre-created hidden (`tauri.conf.json`,
//! label `panel`) so showing it later is instant.

mod engine;
mod log;
mod panel;
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
        }))
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

            tray::create(app.handle())?;
            engine::commands::init(app.handle());
            trigger::start(app.handle());
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
