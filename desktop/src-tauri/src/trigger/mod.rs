//! Translation triggers (Phase 2, issue #3). Everything ends in [`crate::on_trigger`]:
//!
//! - **Double copy** (default): the clipboard changes twice within the double-press interval
//!   (400 ms, like `doublePressInterval` in the Swift app). Port of
//!   `macos/Sources/QuickTranslate/PasteboardWatcher.swift`, but event-driven where the platform
//!   allows it. The platform backends only report "the clipboard changed" ([`Change`]); the
//!   state machine ([`DoubleCopy`]) and the text read live here.
//!   - Windows: `AddClipboardFormatListener` / `WM_CLIPBOARDUPDATE` ([`windows`]).
//!   - Linux X11: XFixes selection notify, polling fallback ([`x11`]).
//!   - Linux Wayland: `ext-data-control` / `wlr-data-control` where the compositor has it
//!     ([`wayland`]); otherwise XWayland's clipboard via [`x11`], otherwise
//!     [`Backend::Unsupported`] so Settings can say so.
//!   - macOS: dev-only polling watcher so the state machine can be exercised on a Mac.
//! - **Single-press hotkey** (optional, off by default): [`shortcut`].
//! - **Tray** "Translate clipboard" and **`--translate`** from a second instance: [`fire`].

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Runtime};

use crate::log;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod x11;

pub mod shortcut;

/// What asked for a translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Clipboard changed twice within the interval (Ctrl+C Ctrl+C).
    DoubleCopy,
    /// The optional single-press global shortcut.
    Hotkey,
    /// Tray menu "Translate clipboard".
    Tray,
    /// `quicktranslate --translate` while an instance is running (scripting trigger).
    SecondInstance,
}

/// A translation request handed to [`crate::on_trigger`].
#[derive(Debug, Clone)]
pub struct Trigger {
    pub source: Source,
    /// Clipboard text at trigger time. May be empty for explicit sources (tray, hotkey,
    /// `--translate`) so the UI can say "clipboard is empty"; never empty for `DoubleCopy`.
    pub text: String,
}

/// Which clipboard watcher is running. Settings (Phase 4) shows a notice for `Unsupported`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // each platform constructs only its own variants
pub enum Backend {
    Windows,
    X11,
    /// X server without XFixes: owner/text polling, can miss a re-copy of identical text.
    X11Polling,
    Wayland,
    /// Wayland session without data-control: watching the XWayland clipboard instead.
    XWayland,
    /// Dev builds on macOS only.
    MacPolling,
    /// No clipboard notifications (e.g. GNOME without XWayland). Use the hotkey or the tray.
    Unsupported,
}

// ---------------------------------------------------------------------------------------------
// Backend → state machine plumbing
// ---------------------------------------------------------------------------------------------

/// What a backend could tell about one clipboard change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // the macOS dev watcher only reports `Unknown`
pub(crate) enum Kind {
    /// Offers a text format.
    Text,
    /// Offers formats, none of them text (image, files, …).
    NonText,
    /// The backend can't tell cheaply; the text read decides.
    Unknown,
}

/// Called by a backend for every clipboard change, from the backend's own thread.
pub(crate) type Sink = Arc<dyn Fn(Kind) + Send + Sync>;

/// Reads the clipboard text once, without retrying. Set by the backend that owns the clipboard
/// connection (Wayland), otherwise [`arboard`].
type Reader = fn() -> Option<String>;

static READER: OnceLock<Reader> = OnceLock::new();
static BACKEND: OnceLock<Backend> = OnceLock::new();
static INTERVAL_MS: AtomicU64 = AtomicU64::new(DEFAULT_INTERVAL.as_millis() as u64);
static MACHINE: Mutex<DoubleCopy> = Mutex::new(DoubleCopy::new());

pub const DEFAULT_INTERVAL: Duration = Duration::from_millis(400);

/// Changes closer together than this are one copy: some apps (Office, some terminals) put
/// several formats on the clipboard in separate transactions and produce 2–3 notifications per
/// Ctrl+C. Nobody presses Ctrl+C twice within 50 ms.
const MERGE_WINDOW: Duration = Duration::from_millis(50);

/// How long a [`suppress_next`] stays armed if the write never shows up.
const SUPPRESS_TTL: Duration = Duration::from_secs(1);

/// Start the clipboard watcher for this platform. Call once from `setup`.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    // Dev override until Settings (Phase 4) owns the interval.
    if let Some(ms) = std::env::var("QUICKTRANSLATE_INTERVAL_MS").ok().and_then(|v| v.parse().ok()) {
        set_interval(Duration::from_millis(ms));
    }
    let handle = app.clone();
    let sink: Sink = Arc::new(move |kind| on_change(&handle, kind));
    let backend = start_backend(sink);
    let _ = BACKEND.set(backend);
    log::write(format!(
        "trigger: clipboard watcher = {backend:?}, interval = {} ms",
        interval().as_millis()
    ));
    if backend == Backend::Unsupported {
        log::write(
            "trigger: clipboard notifications unavailable on this compositor; \
             use the tray item or the hotkey",
        );
    }
    shortcut::start(app);
}

#[cfg(windows)]
fn start_backend(sink: Sink) -> Backend {
    match windows::start(sink) {
        Ok(()) => Backend::Windows,
        Err(e) => {
            log::write(format!("trigger: clipboard listener failed: {e}"));
            Backend::Unsupported
        }
    }
}

/// Decided by the session's display server at runtime: a Wayland session prefers data-control
/// and falls back to XWayland's CLIPBOARD; a plain X session uses XFixes.
#[cfg(target_os = "linux")]
fn start_backend(sink: Sink) -> Backend {
    let has = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    if has("WAYLAND_DISPLAY") {
        match wayland::start(sink.clone()) {
            Ok(()) => {
                let _ = READER.set(wayland::read_text);
                return Backend::Wayland;
            }
            Err(e) => log::write(format!("trigger: wayland data-control unavailable: {e}")),
        }
        if has("DISPLAY") {
            return match x11::start(sink) {
                Ok(_) => Backend::XWayland,
                Err(e) => {
                    log::write(format!("trigger: XWayland clipboard unavailable: {e}"));
                    Backend::Unsupported
                }
            };
        }
        return Backend::Unsupported;
    }
    match x11::start(sink) {
        Ok(x11::Mode::XFixes) => Backend::X11,
        Ok(x11::Mode::Polling) => Backend::X11Polling,
        Err(e) => {
            log::write(format!("trigger: X11 clipboard watcher failed: {e}"));
            Backend::Unsupported
        }
    }
}

#[cfg(target_os = "macos")]
fn start_backend(sink: Sink) -> Backend {
    macos::start(sink);
    Backend::MacPolling
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn start_backend(_sink: Sink) -> Backend {
    Backend::Unsupported
}

/// The running watcher, once [`start`] has run.
#[allow(dead_code)] // read by Settings in Phase 4
pub fn backend() -> Option<Backend> {
    BACKEND.get().copied()
}

/// The double-press interval (default 400 ms).
pub fn interval() -> Duration {
    Duration::from_millis(INTERVAL_MS.load(Ordering::Relaxed))
}

/// Change the double-press interval (Settings, Phase 4).
pub fn set_interval(interval: Duration) {
    INTERVAL_MS.store(interval.as_millis() as u64, Ordering::Relaxed);
}

/// Ignore the next `n` clipboard changes (within one second): call right before the app writes
/// the clipboard itself (the panel's copy button), so that write isn't counted as a Ctrl+C.
#[allow(dead_code)] // used by the copy button in Phase 3
pub fn suppress_next(n: u32) {
    lock().suppress(n, Instant::now());
}

fn lock() -> std::sync::MutexGuard<'static, DoubleCopy> {
    MACHINE.lock().unwrap_or_else(|e| e.into_inner())
}

fn on_change<R: Runtime>(app: &AppHandle<R>, kind: Kind) {
    let now = Instant::now();
    let decision = lock().on_change(now, kind, interval());
    match decision {
        Decision::Trigger(delta) => {
            log::write(format!(
                "trigger: clipboard changed twice (Δ {} ms)",
                delta.as_millis()
            ));
            fire(app, Source::DoubleCopy);
        }
        Decision::First | Decision::Merged => {}
        Decision::Ignored(reason) => log::write(format!("trigger: clipboard change ignored ({reason})")),
    }
}

/// Read the clipboard text (with a short retry) on a worker thread and call
/// [`crate::on_trigger`]. Used by every trigger source.
pub fn fire<R: Runtime>(app: &AppHandle<R>, source: Source) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("trigger".into())
        .spawn(move || {
            let text = read_text_with_retry();
            if source == Source::DoubleCopy && text.is_none() {
                log::write("trigger: double copy but the clipboard has no text, ignoring");
                return;
            }
            crate::on_trigger(&app, Trigger { source, text: text.unwrap_or_default() });
        });
    if let Err(e) = spawned {
        log::write(format!("trigger: could not spawn reader thread: {e}"));
    }
}

/// The writer announces the change (Windows `EmptyClipboard`, macOS `clearContents`, an X11
/// owner change) before the text is actually readable, so a fast read can come back empty.
/// Retry briefly instead of reporting an empty clipboard (same trick as the Tauri spike).
fn read_text_with_retry() -> Option<String> {
    const ATTEMPTS: u32 = 10;
    const PAUSE: Duration = Duration::from_millis(20);
    let read = READER.get().copied().unwrap_or(read_text_arboard);
    for attempt in 0..ATTEMPTS {
        if let Some(text) = read().filter(|t| !t.trim().is_empty()) {
            if attempt > 0 {
                log::write(format!("trigger: clipboard text available after {attempt} retries"));
            }
            return Some(text);
        }
        if attempt + 1 < ATTEMPTS {
            std::thread::sleep(PAUSE);
        }
    }
    None
}

fn read_text_arboard() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}

// ---------------------------------------------------------------------------------------------
// State machine
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision {
    /// First change of a potential pair.
    First,
    /// Within [`MERGE_WINDOW`] of the previous change: same copy.
    Merged,
    /// Second change within the interval; the gap since the first.
    Trigger(Duration),
    Ignored(&'static str),
}

/// Double-copy detector, independent of time sources and platforms so it can be unit tested.
#[derive(Debug)]
struct DoubleCopy {
    /// Time of the previous counted change, if it can still pair up.
    last: Option<Instant>,
    /// Time of the most recent change of any kind (for merging bursts).
    last_any: Option<Instant>,
    suppress: u32,
    suppress_until: Option<Instant>,
}

impl DoubleCopy {
    const fn new() -> Self {
        Self { last: None, last_any: None, suppress: 0, suppress_until: None }
    }

    fn suppress(&mut self, n: u32, now: Instant) {
        self.suppress = self.suppress.saturating_add(n);
        self.suppress_until = Some(now + SUPPRESS_TTL);
    }

    fn on_change(&mut self, now: Instant, kind: Kind, interval: Duration) -> Decision {
        let burst = self.last_any.is_some_and(|t| now.saturating_duration_since(t) < MERGE_WINDOW);
        self.last_any = Some(now);

        if self.suppress > 0 {
            if self.suppress_until.is_some_and(|until| now <= until) {
                if !burst {
                    self.suppress -= 1;
                }
                return Decision::Ignored("our own write");
            }
            self.suppress = 0;
            self.suppress_until = None;
        }
        if burst {
            return Decision::Merged;
        }
        if kind == Kind::NonText {
            // An image or file copy breaks a pending pair.
            self.last = None;
            return Decision::Ignored("not text");
        }
        match self.last {
            Some(prev) if now.saturating_duration_since(prev) <= interval => {
                self.last = None;
                Decision::Trigger(now.saturating_duration_since(prev))
            }
            _ => {
                self.last = Some(now);
                Decision::First
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const I: Duration = DEFAULT_INTERVAL;
    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn two_changes_within_interval_trigger() {
        let t0 = Instant::now();
        let mut m = DoubleCopy::new();
        assert_eq!(m.on_change(t0, Kind::Text, I), Decision::First);
        assert_eq!(m.on_change(t0 + ms(150), Kind::Text, I), Decision::Trigger(ms(150)));
    }

    #[test]
    fn exactly_the_interval_still_triggers() {
        let t0 = Instant::now();
        let mut m = DoubleCopy::new();
        m.on_change(t0, Kind::Text, I);
        assert_eq!(m.on_change(t0 + I, Kind::Text, I), Decision::Trigger(I));
    }

    #[test]
    fn slow_changes_do_not_trigger() {
        let t0 = Instant::now();
        let mut m = DoubleCopy::new();
        assert_eq!(m.on_change(t0, Kind::Text, I), Decision::First);
        assert_eq!(m.on_change(t0 + ms(401), Kind::Text, I), Decision::First);
        // ...but the late one starts a new pair.
        assert_eq!(m.on_change(t0 + ms(600), Kind::Text, I), Decision::Trigger(ms(199)));
    }

    #[test]
    fn third_change_does_not_retrigger() {
        let t0 = Instant::now();
        let mut m = DoubleCopy::new();
        m.on_change(t0, Kind::Text, I);
        assert!(matches!(m.on_change(t0 + ms(100), Kind::Text, I), Decision::Trigger(_)));
        assert_eq!(m.on_change(t0 + ms(200), Kind::Text, I), Decision::First);
    }

    #[test]
    fn interval_is_configurable() {
        let t0 = Instant::now();
        let mut m = DoubleCopy::new();
        m.on_change(t0, Kind::Text, ms(200));
        assert_eq!(m.on_change(t0 + ms(300), Kind::Text, ms(200)), Decision::First);
        assert!(matches!(m.on_change(t0 + ms(450), Kind::Text, ms(200)), Decision::Trigger(_)));
    }

    #[test]
    fn burst_from_one_copy_is_merged() {
        let t0 = Instant::now();
        let mut m = DoubleCopy::new();
        assert_eq!(m.on_change(t0, Kind::Text, I), Decision::First);
        assert_eq!(m.on_change(t0 + ms(5), Kind::Text, I), Decision::Merged);
        assert_eq!(m.on_change(t0 + ms(30), Kind::Text, I), Decision::Merged);
        assert_eq!(m.on_change(t0 + ms(200), Kind::Text, I), Decision::Trigger(ms(200)));
    }

    #[test]
    fn non_text_is_ignored_and_breaks_a_pair() {
        let t0 = Instant::now();
        let mut m = DoubleCopy::new();
        assert_eq!(m.on_change(t0, Kind::NonText, I), Decision::Ignored("not text"));
        assert_eq!(m.on_change(t0 + ms(100), Kind::Text, I), Decision::First);
        assert_eq!(m.on_change(t0 + ms(200), Kind::NonText, I), Decision::Ignored("not text"));
        assert_eq!(m.on_change(t0 + ms(300), Kind::Text, I), Decision::First);
    }

    #[test]
    fn unknown_kind_counts_like_text() {
        let t0 = Instant::now();
        let mut m = DoubleCopy::new();
        m.on_change(t0, Kind::Unknown, I);
        assert!(matches!(m.on_change(t0 + ms(100), Kind::Unknown, I), Decision::Trigger(_)));
    }

    #[test]
    fn own_writes_are_suppressed_without_breaking_a_pair() {
        let t0 = Instant::now();
        let mut m = DoubleCopy::new();
        assert_eq!(m.on_change(t0, Kind::Text, I), Decision::First);
        m.suppress(1, t0 + ms(80));
        assert_eq!(m.on_change(t0 + ms(100), Kind::Text, I), Decision::Ignored("our own write"));
        assert!(matches!(m.on_change(t0 + ms(250), Kind::Text, I), Decision::Trigger(_)));
    }

    #[test]
    fn suppression_counts_down() {
        let t0 = Instant::now();
        let mut m = DoubleCopy::new();
        m.suppress(2, t0);
        assert_eq!(m.on_change(t0 + ms(100), Kind::Text, I), Decision::Ignored("our own write"));
        // A burst belonging to the same write doesn't use up the second suppression.
        assert_eq!(m.on_change(t0 + ms(110), Kind::Text, I), Decision::Ignored("our own write"));
        assert_eq!(m.on_change(t0 + ms(300), Kind::Text, I), Decision::Ignored("our own write"));
        assert_eq!(m.on_change(t0 + ms(500), Kind::Text, I), Decision::First);
    }

    #[test]
    fn suppression_expires() {
        let t0 = Instant::now();
        let mut m = DoubleCopy::new();
        m.suppress(1, t0);
        assert_eq!(m.on_change(t0 + ms(1500), Kind::Text, I), Decision::First);
        assert!(matches!(m.on_change(t0 + ms(1600), Kind::Text, I), Decision::Trigger(_)));
    }
}
