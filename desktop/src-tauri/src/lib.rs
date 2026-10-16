//! QuickTranslate — Tauri feasibility spike (macOS). Tray-only app, ⌘C ⌘C trigger via pasteboard
//! polling, a pre-created hidden panel shown near the cursor, and a resident Claude worker.

mod log;
#[cfg(target_os = "macos")]
mod macos;
mod worker;

use std::sync::Arc;
use std::time::Duration;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::window::{Effect, EffectState, EffectsBuilder};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use worker::Worker;

const PANEL: &str = "panel";

/// `QT_PANEL=window` keeps a plain Tauri window instead of converting it to an NSPanel.
fn use_nspanel() -> bool {
    std::env::var("QT_PANEL").map(|v| v != "window").unwrap_or(true)
}

#[tauri::command]
fn hide_panel(app: AppHandle) {
    hide(&app);
}

/// Lets the webview report its own timings (e.g. when a delta was painted).
#[tauri::command]
fn mark(what: String) {
    log::write(format!("JS {what}"));
}

fn hide(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    if use_nspanel() {
        use tauri_nspanel::ManagerExt;
        if let Ok(p) = app.get_webview_panel(PANEL) {
            p.hide();
            return;
        }
    }
    if let Some(w) = app.get_webview_window(PANEL) {
        let _ = w.hide();
    }
}

fn position_near_cursor(app: &AppHandle) {
    let Some(w) = app.get_webview_window(PANEL) else { return };
    let Ok(cur) = app.cursor_position() else { return };
    let Ok(size) = w.outer_size() else { return };
    let mon = app.monitor_from_point(cur.x, cur.y).ok().flatten().or_else(|| w.primary_monitor().ok().flatten());
    let (mut x, mut y) = (cur.x + 12.0, cur.y + 12.0);
    if let Some(m) = mon {
        let wa = m.work_area();
        let (minx, miny) = (wa.position.x as f64, wa.position.y as f64);
        let (maxx, maxy) = (minx + wa.size.width as f64, miny + wa.size.height as f64);
        x = x.min(maxx - size.width as f64).max(minx);
        y = y.min(maxy - size.height as f64).max(miny);
    }
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

fn show_panel(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let before = macos::frontmost_app();
    position_near_cursor(app);
    log::write("SHOW_BEGIN");
    #[cfg(target_os = "macos")]
    if use_nspanel() {
        use tauri_nspanel::ManagerExt;
        if let Ok(p) = app.get_webview_panel(PANEL) {
            p.show_and_make_key();
        }
    } else if let Some(w) = app.get_webview_window(PANEL) {
        let _ = w.show();
        let _ = w.set_focus();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(w) = app.get_webview_window(PANEL) {
        let _ = w.show();
        let _ = w.set_focus();
    }
    log::write("SHOW_END (orderFront returned)");
    #[cfg(target_os = "macos")]
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        log::write(format!("FRONTMOST before={before:?} after+300ms={:?}", macos::frontmost_app()));
    });
}

fn translate_clipboard(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let text = macos::clipboard_string().unwrap_or_default();
    #[cfg(not(target_os = "macos"))]
    let text = String::new();
    let text = text.trim().to_string();
    if text.is_empty() {
        log::write("clipboard empty, ignoring");
        return;
    }
    let handle = app.clone();
    let shown_text = text.clone();
    let _ = app.run_on_main_thread(move || {
        let _ = handle.emit_to(PANEL, "tr-start", shown_text);
        show_panel(&handle);
    });
    let worker = app.state::<Arc<Worker>>().inner().clone();
    tauri::async_runtime::spawn(async move { worker.translate(text).await });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    log::init();
    log::write("main() entered");
    let mut builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }
    builder
        .invoke_handler(tauri::generate_handler![hide_panel, mark])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Pre-created hidden panel.
            let window = WebviewWindowBuilder::new(app, PANEL, WebviewUrl::App("index.html".into()))
                .title("QuickTranslate")
                .inner_size(480.0, 340.0)
                .min_inner_size(380.0, 240.0)
                .decorations(false)
                .transparent(true)
                .shadow(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .visible(false)
                .resizable(true)
                .focused(false)
                .visible_on_all_workspaces(true)
                .effects(
                    EffectsBuilder::new()
                        .effect(Effect::Popover)
                        .state(EffectState::Active)
                        .radius(12.0)
                        .build(),
                )
                .build()?;
            log::write("panel window created (hidden)");
            #[cfg(target_os = "macos")]
            macos::log_view_tree(&window);
            #[cfg(target_os = "macos")]
            if use_nspanel() {
                macos::make_panel(&window)?;
                log::write("panel converted to non-activating NSPanel");
            }

            // Hide on outside click (= panel loses key) unless a translation is streaming.
            let h = app.handle().clone();
            window.on_window_event(move |e| {
                if let WindowEvent::Focused(false) = e {
                    if h.try_state::<Arc<Worker>>().is_some_and(|w| w.is_busy()) {
                        log::write("panel lost focus while streaming → keep");
                        return;
                    }
                    log::write("panel lost focus → hide");
                    hide(&h);
                }
            });

            // Tray.
            let translate_i = MenuItem::with_id(app, "translate", "Translate clipboard", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, Some("CmdOrCtrl+Q"))?;
            let menu = Menu::with_items(app, &[&translate_i, &quit_i])?;
            TrayIconBuilder::with_id("main")
                .icon(tauri::include_image!("icons/tray.png"))
                .icon_as_template(true)
                .tooltip("QuickTranslate (Tauri spike)")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "translate" => {
                        log::mark_trigger();
                        log::write("TRIGGER tray menu");
                        translate_clipboard(app);
                    }
                    "quit" => {
                        let w = app.state::<Arc<Worker>>().inner().clone();
                        tauri::async_runtime::block_on(w.shutdown());
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;
            log::write("TRAY_READY");

            // Resident Claude worker, pre-warmed.
            let worker = Worker::new(app.handle().clone());
            app.manage(worker.clone());
            if std::env::var("QT_NO_PREWARM").is_err() {
                tauri::async_runtime::spawn(async move { worker.prewarm().await });
            }

            // ⌘C ⌘C.
            #[cfg(target_os = "macos")]
            {
                let h = app.handle().clone();
                macos::spawn_pasteboard_watcher(Duration::from_millis(400), move || translate_clipboard(&h));
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(w) = app.try_state::<Arc<Worker>>() {
                    tauri::async_runtime::block_on(w.inner().clone().shutdown());
                }
            }
        });
}
