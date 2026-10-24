//! The app settings: a port of the Swift app's `Settings.swift` (`AppSettings`).
//!
//! Stored as plain JSON (serde) in `<app config dir>/settings.json`. The keys are the Swift
//! `UserDefaults` keys verbatim where the setting exists there; the desktop-only ones are
//! `hotkey` (the single-press shortcut as text, e.g. `Ctrl+Shift+T`: the Swift app stores macOS
//! key codes instead) and `launchAtLogin` (the macOS app reads the system's login items instead).
//!
//! Loading is lenient, key by key: a missing file, a malformed file or a key with a value of the
//! wrong type falls back to the Swift default for that key only. Keys this version doesn't know
//! are kept and written back unchanged.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    #[default]
    Claude,
    Codex,
}

impl Backend {
    pub fn executable_name(self) -> &'static str {
        match self {
            Backend::Claude => "claude",
            Backend::Codex => "codex",
        }
    }
}

/// Interface languages for `uiLanguage` ("system" follows the OS).
pub const UI_LANGUAGES: [&str; 4] = ["system", "en", "ko", "ja"];

/// Range of `doublePressInterval` in seconds (the Swift slider's range).
pub const INTERVAL_RANGE: (f64, f64) = (0.2, 1.0);

pub const DEFAULT_HOTKEY: &str = "Ctrl+Shift+T";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub backend: Backend,
    pub claude_model: String,
    pub codex_model: String,
    /// Manual CLI paths; blank = auto-detect.
    pub claude_path: String,
    pub codex_path: String,
    pub keep_claude_warm: bool,
    /// `None` until picked: the system languages decide (`panel::language_defaults`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_language: Option<String>,
    /// "system", "en", "ko" or "ja".
    pub ui_language: String,
    /// `true`: Ctrl+C Ctrl+C (detected from the clipboard). `false`: the single-press `hotkey`.
    pub hotkey_double_press: bool,
    /// The single-press shortcut, `Ctrl+Shift+T` style (desktop only).
    pub hotkey: String,
    /// Seconds between the two copies.
    pub double_press_interval: f64,
    pub close_on_outside_click: bool,
    /// Run at login (desktop only). Mirrors the OS autostart entry, which wins when they differ.
    pub launch_at_login: bool,
    /// Keys this version doesn't know, kept so saving doesn't drop them.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            backend: Backend::Claude,
            claude_model: "haiku".into(),
            codex_model: String::new(),
            claude_path: String::new(),
            codex_path: String::new(),
            keep_claude_warm: true,
            target_language: None,
            fallback_language: None,
            ui_language: "system".into(),
            hotkey_double_press: true,
            hotkey: DEFAULT_HOTKEY.into(),
            double_press_interval: 0.4,
            close_on_outside_click: true,
            launch_at_login: false,
            extra: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// Loads `path`, falling back to defaults when the file is missing or malformed.
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        // Windows editors (Notepad, PowerShell 5's Set-Content) may prepend a UTF-8 BOM.
        let text = text.trim_start_matches('\u{feff}');
        match serde_json::from_str::<Value>(text) {
            Ok(Value::Object(map)) => Self::default().merged(&map, |key, e| {
                crate::log::write(format!("settings: ignoring {key:?} in {}: {e}", path.display()))
            }),
            Ok(_) | Err(_) => {
                crate::log::write(format!(
                    "settings: ignoring malformed {}",
                    path.display()
                ));
                Self::default()
            }
        }
    }

    /// `self` with the keys of `patch` applied one by one; a key whose value doesn't fit is
    /// skipped and reported to `on_error`. The result is [`Self::normalized`].
    pub fn merged(&self, patch: &Map<String, Value>, mut on_error: impl FnMut(&str, String)) -> Self {
        let mut current = match serde_json::to_value(self) {
            Ok(Value::Object(map)) => map,
            _ => Map::new(),
        };
        for (key, value) in patch {
            let mut candidate = current.clone();
            if value.is_null() {
                candidate.remove(key); // e.g. targetLanguage: null = back to the system default
            } else {
                candidate.insert(key.clone(), value.clone());
            }
            match serde_json::from_value::<Settings>(Value::Object(candidate.clone())) {
                Ok(_) => current = candidate,
                Err(e) => on_error(key, e.to_string()),
            }
        }
        serde_json::from_value::<Settings>(Value::Object(current))
            .unwrap_or_default()
            .normalized()
    }

    /// Clamps and repairs values a hand-edited file could get wrong.
    pub fn normalized(mut self) -> Self {
        let (lo, hi) = INTERVAL_RANGE;
        if !self.double_press_interval.is_finite() {
            self.double_press_interval = 0.4;
        }
        self.double_press_interval = self.double_press_interval.clamp(lo, hi);
        if !UI_LANGUAGES.contains(&self.ui_language.as_str()) {
            self.ui_language = "system".into();
        }
        let known = |l: &Option<String>| {
            l.as_ref()
                .is_some_and(|l| super::language::LANGUAGES.iter().any(|k| k.name == l))
        };
        if !known(&self.target_language) {
            self.target_language = None;
        }
        if !known(&self.fallback_language) {
            self.fallback_language = None;
        }
        if self.hotkey.trim().is_empty() {
            self.hotkey = DEFAULT_HOTKEY.into();
        }
        self
    }

    /// Writes the settings atomically: a temporary file next to `path`, then a rename over it.
    pub fn save(&self, path: &Path) -> std::io::Result<String> {
        let mut text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        text.push('\n');
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &text)?;
        if let Err(e) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        Ok(text)
    }

    pub fn current_model(&self) -> &str {
        match self.backend {
            Backend::Claude => &self.claude_model,
            Backend::Codex => &self.codex_model,
        }
    }

    pub fn current_path_override(&self) -> &str {
        match self.backend {
            Backend::Claude => &self.claude_path,
            Backend::Codex => &self.codex_path,
        }
    }

    pub fn path_override(&self, backend: Backend) -> &str {
        match backend {
            Backend::Claude => &self.claude_path,
            Backend::Codex => &self.codex_path,
        }
    }

    /// Whether the resident Claude process depends on anything that changed.
    pub fn warm_worker_changed(&self, other: &Settings) -> bool {
        self.backend != other.backend
            || self.keep_claude_warm != other.keep_claude_warm
            || self.claude_model != other.claude_model
            || self.claude_path != other.claude_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_bom_is_ignored() {
        let path = std::env::temp_dir().join(format!("qt-settings-bom-{}.json", std::process::id()));
        std::fs::write(&path, "\u{feff}{\"claudeModel\": \"sonnet\"}").unwrap();
        let s = Settings::load(&path);
        std::fs::remove_file(&path).ok();
        assert_eq!(s.claude_model, "sonnet");
    }

    fn parse(json: &str) -> Settings {
        match serde_json::from_str::<Value>(json).unwrap() {
            Value::Object(map) => Settings::default().merged(&map, |_, _| {}),
            _ => unreachable!(),
        }
    }

    #[test]
    fn defaults_match_swift() {
        let s = parse("{}");
        assert_eq!(s, Settings::default());
        assert_eq!(s.backend, Backend::Claude);
        assert_eq!(s.claude_model, "haiku");
        assert_eq!(s.codex_model, "");
        assert!(s.keep_claude_warm);
        assert!(s.hotkey_double_press);
        assert_eq!(s.double_press_interval, 0.4);
        assert!(s.close_on_outside_click);
        assert_eq!(s.ui_language, "system");
        assert_eq!(s.target_language, None);
        assert!(!s.launch_at_login);
    }

    #[test]
    fn swift_keys() {
        let s = parse(
            r#"{"backend":"codex","claudeModel":"sonnet","codexModel":"gpt-5","claudePath":"~/c",
                "codexPath":"/x/codex","keepClaudeWarm":false,"targetLanguage":"Korean",
                "fallbackLanguage":"Japanese","uiLanguage":"ko","hotkeyDoublePress":false,
                "doublePressInterval":0.55,"closeOnOutsideClick":false}"#,
        );
        assert_eq!(s.backend, Backend::Codex);
        assert_eq!(s.current_model(), "gpt-5");
        assert_eq!(s.current_path_override(), "/x/codex");
        assert_eq!(s.claude_model, "sonnet");
        assert_eq!(s.claude_path, "~/c");
        assert!(!s.keep_claude_warm);
        assert_eq!(s.target_language.as_deref(), Some("Korean"));
        assert_eq!(s.fallback_language.as_deref(), Some("Japanese"));
        assert_eq!(s.ui_language, "ko");
        assert!(!s.hotkey_double_press);
        assert_eq!(s.double_press_interval, 0.55);
        assert!(!s.close_on_outside_click);
    }

    #[test]
    fn a_bad_value_only_resets_its_key() {
        let s = parse(r#"{"backend":"gemini","claudeModel":"opus","keepClaudeWarm":"yes"}"#);
        assert_eq!(s.backend, Backend::Claude);
        assert_eq!(s.claude_model, "opus");
        assert!(s.keep_claude_warm);
    }

    #[test]
    fn values_are_normalized() {
        let s = parse(
            r#"{"doublePressInterval":5,"uiLanguage":"fr","targetLanguage":"Klingon","hotkey":" "}"#,
        );
        assert_eq!(s.double_press_interval, 1.0);
        assert_eq!(s.ui_language, "system");
        assert_eq!(s.target_language, None);
        assert_eq!(s.hotkey, DEFAULT_HOTKEY);
    }

    #[test]
    fn null_resets_an_optional_key() {
        let s = parse(r#"{"targetLanguage":"Korean"}"#);
        let mut patch = Map::new();
        patch.insert("targetLanguage".into(), Value::Null);
        assert_eq!(s.merged(&patch, |_, _| {}).target_language, None);
    }

    #[test]
    fn round_trip_keeps_unknown_keys() {
        let dir = std::env::temp_dir().join(format!("qt-settings-{}", std::process::id()));
        let path = dir.join("settings.json");
        let mut s = parse(r#"{"futureKey":[1,2],"claudeModel":"sonnet"}"#);
        s.launch_at_login = true;
        s.hotkey = "Ctrl+Alt+K".into();
        s.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"futureKey\""));
        assert!(text.contains("\"launchAtLogin\": true"));
        assert!(!text.contains("targetLanguage"), "unset languages stay unset");
        assert!(!dir.join("settings.json.tmp").exists());
        assert_eq!(Settings::load(&path), s);
        let _ = std::fs::remove_dir_all(dir);
    }
}
