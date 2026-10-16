//! All macOS-specific glue for the spike: pasteboard polling, frontmost-app probe, NSPanel conversion.

use std::time::{Duration, Instant};

use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString, NSWorkspace};
use tauri::WebviewWindow;
use tauri_nspanel::{tauri_panel, CollectionBehavior, PanelLevel, StyleMask, WebviewWindowExt};

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

/// Convert the pre-created Tauri window into a non-activating NSPanel (like the Swift app's
/// `.nonactivatingPanel` style), floating on all Spaces including full-screen ones.
pub fn make_panel(window: &WebviewWindow) -> tauri::Result<()> {
    let panel = window.to_panel::<TranslationPanel>()?;
    if let Err(e) = panel.add_style_mask(StyleMask::empty().nonactivating_panel().value()) {
        crate::log::write(format!("nonactivating style mask rejected: {e:?}"));
    }
    panel.set_level(PanelLevel::Floating.value());
    panel.set_collection_behavior(
        CollectionBehavior::new().can_join_all_spaces().full_screen_auxiliary().value(),
    );
    Ok(())
}

pub fn change_count() -> isize {
    NSPasteboard::generalPasteboard().changeCount()
}

/// The writer bumps `changeCount` in `clearContents` *before* it writes the string, so a poll can
/// land in between; retry briefly instead of reporting an empty clipboard.
pub fn clipboard_string() -> Option<String> {
    let pb = NSPasteboard::generalPasteboard();
    for attempt in 0..10 {
        if let Some(s) = unsafe { pb.stringForType(NSPasteboardTypeString) } {
            if attempt > 0 {
                crate::log::write(format!("clipboard string available after {attempt} retries"));
            }
            return Some(s.to_string());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

/// Debug: log the native view hierarchy of a window (to confirm NSVisualEffectView is installed).
pub fn log_view_tree(window: &WebviewWindow) {
    let Ok(ptr) = window.ns_window() else { return };
    let ns_window: &objc2_app_kit::NSWindow = unsafe { &*(ptr as *const objc2_app_kit::NSWindow) };
    fn walk(v: &objc2_app_kit::NSView, depth: usize, out: &mut Vec<String>) {
        out.push(format!("{}{}", "  ".repeat(depth), v.class().name().to_string_lossy()));
        for sub in v.subviews().iter() {
            walk(&sub, depth + 1, out);
        }
    }
    let mut out = vec![];
    if let Some(cv) = ns_window.contentView() {
        walk(&cv, 0, &mut out);
    }
    crate::log::write(format!("view tree:\n{}", out.join("\n")));
}

pub fn frontmost_app() -> String {
    NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .and_then(|a| a.localizedName())
        .map(|n| n.to_string())
        .unwrap_or_else(|| "?".into())
}

/// ⌘C ⌘C = `changeCount` bumped twice within `window` (port of PasteboardWatcher.swift).
/// Runs on its own thread with a 50 ms poll; NSPasteboard is safe to query off the main thread.
pub fn spawn_pasteboard_watcher(window: Duration, on_trigger: impl Fn() + Send + 'static) {
    std::thread::Builder::new()
        .name("pasteboard-watcher".into())
        .spawn(move || {
            let mut last_count = change_count();
            let mut last_change: Option<Instant> = None;
            // Measurement aid: fire on every single change so scripted `pbcopy` runs don't also
            // trigger the installed Swift app (which needs two changes within 0.4 s).
            let single_change = std::env::var("QT_TRIGGER").is_ok_and(|v| v == "single");
            loop {
                std::thread::sleep(Duration::from_millis(50));
                let count = change_count();
                if count == last_count {
                    continue;
                }
                last_count = count;
                let now = Instant::now();
                if single_change {
                    crate::log::mark_trigger();
                    crate::log::write("TRIGGER pasteboard changed once (QT_TRIGGER=single)");
                    on_trigger();
                    continue;
                }
                match last_change {
                    Some(prev) if now.duration_since(prev) <= window => {
                        last_change = None;
                        crate::log::mark_trigger();
                        crate::log::write(format!(
                            "TRIGGER pasteboard changed twice (Δ {:.0} ms)",
                            now.duration_since(prev).as_secs_f64() * 1000.0
                        ));
                        on_trigger();
                    }
                    _ => last_change = Some(now),
                }
            }
        })
        .expect("spawn watcher");
}
