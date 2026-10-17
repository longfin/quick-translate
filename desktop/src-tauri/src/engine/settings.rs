//! The engine-related subset of `macos/Sources/QuickTranslate/Settings.swift` (`AppSettings`).
//!
//! Read from a plain JSON file with serde for now; Phase 4 moves to `tauri-plugin-store`. The keys
//! are the Swift `UserDefaults` keys verbatim so the two stay interchangeable. Missing keys (and a
//! missing or unreadable file) fall back to the Swift defaults.

use std::path::Path;

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub backend: Backend,
    pub claude_model: String,
    pub codex_model: String,
    /// Manual CLI paths; blank = auto-detect.
    pub claude_path: String,
    pub codex_path: String,
    pub keep_claude_warm: bool,
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
        }
    }
}

impl Settings {
    /// Loads `path`, falling back to defaults when the file is missing or malformed. Unknown keys
    /// (the rest of `AppSettings`: hotkey, languages, ...) are ignored.
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                crate::log::write(format!(
                    "settings: ignoring malformed {}: {e}",
                    path.display()
                ));
                Self::default()
            }),
            Err(_) => Self::default(),
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_swift() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, Settings::default());
        assert_eq!(s.backend, Backend::Claude);
        assert_eq!(s.claude_model, "haiku");
        assert!(s.keep_claude_warm);
    }

    #[test]
    fn swift_keys() {
        let s: Settings = serde_json::from_str(
            r#"{"backend":"codex","claudeModel":"sonnet","codexModel":"gpt-5","claudePath":"~/c",
                "codexPath":"/x/codex","keepClaudeWarm":false,"targetLanguage":"Korean"}"#,
        )
        .unwrap();
        assert_eq!(s.backend, Backend::Codex);
        assert_eq!(s.current_model(), "gpt-5");
        assert_eq!(s.current_path_override(), "/x/codex");
        assert_eq!(s.claude_model, "sonnet");
        assert_eq!(s.claude_path, "~/c");
        assert!(!s.keep_claude_warm);
    }
}
