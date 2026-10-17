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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Must be the first plugin registered.
        .plugin(tauri_plugin_single_instance::init(|_app, args, _cwd| {
            // Phase 2 routes `--translate` here as a scripting trigger.
            log::write(format!("second instance launched, args={:?}", args.get(1..).unwrap_or(&[])));
        }))
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
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app, event| match event {
        // Closing the last window must not quit a tray app; only the tray's Quit (`exit(0)`,
        // which sets a code) ends it.
        RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
        RunEvent::Exit => log::write("exit"),
        _ => {}
    });
}
