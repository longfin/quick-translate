//! Tiny timestamped logger used for the spike's measurements.
//! Writes to stderr and to `~/Library/Logs/QuickTranslateSpike.log`.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

static START: OnceLock<Instant> = OnceLock::new();
static TRIGGER: Mutex<Option<Instant>> = Mutex::new(None);
static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

pub fn init() {
    START.get_or_init(Instant::now);
    let path = std::env::var("QT_LOG").unwrap_or_else(|_| {
        format!("{}/Library/Logs/QuickTranslateSpike.log", std::env::var("HOME").unwrap_or_default())
    });
    if let Ok(f) = OpenOptions::new().create(true).append(true).open(path) {
        *FILE.lock().unwrap() = Some(f);
    }
}

/// Marks the start of a trigger so later lines can report "since trigger".
pub fn mark_trigger() {
    *TRIGGER.lock().unwrap() = Some(Instant::now());
}

pub fn since_trigger_ms() -> Option<f64> {
    TRIGGER.lock().unwrap().map(|t| t.elapsed().as_secs_f64() * 1000.0)
}

pub fn write(msg: impl AsRef<str>) {
    let epoch_ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let up = START.get().map(|s| s.elapsed().as_secs_f64() * 1000.0).unwrap_or(0.0);
    let trig = since_trigger_ms().map(|t| format!(" trig+{t:.1}ms")).unwrap_or_default();
    let line = format!("[{epoch_ms}] up+{up:.1}ms{trig} {}\n", msg.as_ref());
    eprint!("{line}");
    if let Some(f) = FILE.lock().unwrap().as_mut() {
        let _ = f.write_all(line.as_bytes());
    }
}
