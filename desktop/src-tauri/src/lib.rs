//! QuickTranslate for Windows and Linux (issue #3). A tray-only app: no window at startup,
//! nothing in the taskbar. The translation panel is pre-created hidden (`tauri.conf.json`,
//! label `panel`) so showing it later is instant.

mod engine;
mod log;
mod trigger;
mod tray;

use tauri::RunEvent;

/// Label of the pre-created, hidden translation window.
pub const PANEL: &str = "panel";

/// Every trigger (double copy, hotkey, tray, `--translate`) ends here.
/// Stub until the panel (Phase 3) wires it to `engine::commands::translate`: logs, shows nothing.
pub fn on_trigger<R: tauri::Runtime>(_app: &tauri::AppHandle<R>, trigger: trigger::Trigger) {
    log::write(format!(
        "TRIGGER source={:?} chars={}",
        trigger.source,
        trigger.text.chars().count()
    ));
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
        .invoke_handler(engine::commands::handler())
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
