//! DEV ONLY. macOS ships the Swift app (`macos/`); this watcher exists so the shared state
//! machine can be run on a Mac while developing the Windows/Linux app. Polls
//! `NSPasteboard.changeCount` every 60 ms, like `PasteboardWatcher.swift`.

use std::time::Duration;

use objc2_app_kit::NSPasteboard;

use super::{Kind, Sink};

pub fn start(sink: Sink) {
    let spawned = std::thread::Builder::new().name("clipboard-watch".into()).spawn(move || {
        // NSPasteboard's changeCount is safe to query off the main thread.
        let count = || NSPasteboard::generalPasteboard().changeCount();
        let mut last = count();
        loop {
            std::thread::sleep(Duration::from_millis(60));
            let now = count();
            if now != last {
                last = now;
                // Text or not is decided by the read; the dev watcher doesn't inspect types.
                sink(Kind::Unknown);
            }
        }
    });
    if let Err(e) = spawned {
        crate::log::write(format!("trigger: could not spawn macOS dev watcher: {e}"));
    }
}
