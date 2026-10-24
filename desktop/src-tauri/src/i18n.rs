//! UI strings on the Rust side (tray menu, window titles), from the same catalogs as the web UI
//! (`src/locales/*.json`).

use std::collections::HashMap;
use std::sync::OnceLock;

type Catalog = HashMap<String, String>;

fn catalogs() -> &'static [(&'static str, Catalog); 3] {
    static CATALOGS: OnceLock<[(&'static str, Catalog); 3]> = OnceLock::new();
    CATALOGS.get_or_init(|| {
        let parse = |json: &str| serde_json::from_str::<Catalog>(json).unwrap_or_default();
        [
            ("en", parse(include_str!("../../src/locales/en.json"))),
            ("ko", parse(include_str!("../../src/locales/ko.json"))),
            ("ja", parse(include_str!("../../src/locales/ja.json"))),
        ]
    })
}

/// The UI locale for a `uiLanguage` setting: "en", "ko" or "ja"; "system" follows the first
/// system language (ko*, ja*, else en), like `systemLocale()` in `src/i18n.ts`.
pub fn locale(preference: &str) -> &'static str {
    match preference {
        "en" => "en",
        "ko" => "ko",
        "ja" => "ja",
        _ => {
            let tag = sys_locale::get_locale().unwrap_or_default().to_ascii_lowercase();
            if tag.starts_with("ko") {
                "ko"
            } else if tag.starts_with("ja") {
                "ja"
            } else {
                "en"
            }
        }
    }
}

/// `key` (the English text) in `locale`, else in English, else the key itself.
pub fn t(locale: &str, key: &str) -> String {
    let all = catalogs();
    let find = |code: &str| {
        all.iter()
            .find(|(c, _)| *c == code)
            .and_then(|(_, cat)| cat.get(key))
    };
    find(locale)
        .or_else(|| find("en"))
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_with_fallback() {
        assert_eq!(t("ko", "Settings…"), "설정…");
        assert_eq!(t("ja", "Quit"), "終了");
        assert_eq!(t("en", "Quit"), "Quit");
        assert_eq!(t("ko", "no such key"), "no such key");
        assert_eq!(locale("ja"), "ja");
    }
}
