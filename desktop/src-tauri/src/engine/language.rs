//! Source language detection, ported from `macos/Sources/QuickTranslate/LanguageDetector.swift`.
//!
//! The Swift app uses `NLLanguageRecognizer`; here a Unicode-script count decides every non-Latin
//! script (as in Swift, which trusts the script first), and the `whatlang` trigram model, limited
//! to the app's languages, handles Latin-script text. Results are the app's language identifiers
//! (`Language::name`, e.g. `"Korean"`), the same strings Swift stores in `targetLanguage`.

// Wired into the panel and the trigger after Phase 3 lands.
#![allow(dead_code)]

use whatlang::{Detector, Lang};

/// One of the app's translation languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language {
    /// Locale code, as in Swift's `AppSettings.systemDefaultLanguages` (`"ko"`, `"zh-Hans"`, ...).
    pub code: &'static str,
    /// The app's identifier for the language: settings value, prompt text and localization key.
    pub name: &'static str,
}

/// Swift's `AppSettings.languages`, in the same order.
#[rustfmt::skip]
pub const LANGUAGES: &[Language] = &[
    Language { code: "ko", name: "Korean" },
    Language { code: "en", name: "English" },
    Language { code: "ja", name: "Japanese" },
    Language { code: "zh-Hans", name: "Chinese (Simplified)" },
    Language { code: "zh-Hant", name: "Chinese (Traditional)" },
    Language { code: "es", name: "Spanish" },
    Language { code: "fr", name: "French" },
    Language { code: "de", name: "German" },
    Language { code: "pt", name: "Portuguese" },
    Language { code: "it", name: "Italian" },
    Language { code: "ru", name: "Russian" },
    Language { code: "vi", name: "Vietnamese" },
    Language { code: "th", name: "Thai" },
    Language { code: "id", name: "Indonesian" },
    Language { code: "hi", name: "Hindi" },
    Language { code: "ar", name: "Arabic" },
];

/// Below this many letters, Latin-script text is too short to call (a word or a name).
const MIN_LATIN_LETTERS: usize = 6;
/// Same cut-off as the Swift `NLLanguageRecognizer` hypotheses.
const MIN_CONFIDENCE: f64 = 0.5;
/// Swift looks at the first 2000 characters only.
const SAMPLE_CHARS: usize = 2000;

/// whatlang languages that map onto [`LANGUAGES`]. Mandarin is resolved to Simplified/Traditional
/// separately, since whatlang does not tell them apart.
const WHATLANG: &[(Lang, &str)] = &[
    (Lang::Kor, "Korean"),
    (Lang::Eng, "English"),
    (Lang::Jpn, "Japanese"),
    (Lang::Cmn, "Chinese (Simplified)"),
    (Lang::Spa, "Spanish"),
    (Lang::Fra, "French"),
    (Lang::Deu, "German"),
    (Lang::Por, "Portuguese"),
    (Lang::Ita, "Italian"),
    (Lang::Rus, "Russian"),
    (Lang::Vie, "Vietnamese"),
    (Lang::Tha, "Thai"),
    (Lang::Ind, "Indonesian"),
    (Lang::Hin, "Hindi"),
    (Lang::Ara, "Arabic"),
];

/// Returns one of [`LANGUAGES`]' names, or `None` when undetermined.
pub fn detect(text: &str) -> Option<String> {
    let sample = match text.char_indices().nth(SAMPLE_CHARS) {
        Some((i, _)) => &text[..i],
        None => text,
    };
    let letters = sample.chars().filter(|c| c.is_alphabetic()).count();
    if letters == 0 {
        return None;
    }
    if let Some(name) = detect_by_script(sample) {
        return Some(name.to_owned());
    }
    if letters < MIN_LATIN_LETTERS {
        return None;
    }
    recognize(sample).map(str::to_owned)
}

/// Unicode-script vote, a straight port of Swift's `detectByScript`.
///
/// `NLLanguageRecognizer` (and whatlang) are swayed by letter count, so Korean prose mixed with code
/// or English identifiers can come out as "Italian, 22%". One non-Latin letter carries far more
/// signal than a Latin one, so when a non-Latin script makes up a good share (30%) of the letters,
/// trust the script. Returns `None` for mostly-Latin text.
pub fn detect_by_script(text: &str) -> Option<&'static str> {
    let (mut latin, mut hangul, mut kana, mut han) = (0usize, 0usize, 0usize, 0usize);
    let (mut cyrillic, mut arabic, mut thai, mut devanagari) = (0usize, 0usize, 0usize, 0usize);
    for c in text.chars().filter(|c| c.is_alphabetic()) {
        match c as u32 {
            0xAC00..=0xD7A3 | 0x1100..=0x11FF | 0x3130..=0x318F => hangul += 1,
            0x3040..=0x30FF => kana += 1,
            0x4E00..=0x9FFF | 0x3400..=0x4DBF => han += 1,
            0x0400..=0x04FF => cyrillic += 1,
            0x0600..=0x06FF => arabic += 1,
            0x0E00..=0x0E7F => thai += 1,
            0x0900..=0x097F => devanagari += 1,
            ..0x0250 => latin += 1,
            _ => {}
        }
    }
    let total = latin + hangul + kana + han + cyrillic + arabic + thai + devanagari;
    if total == 0 {
        return None;
    }
    let threshold = total as f64 * 0.3;
    let share = |n: usize| n as f64 >= threshold;
    if share(hangul) {
        Some("Korean")
    } else if kana > 0 && share(kana + han) {
        Some("Japanese")
    } else if share(han) {
        Some(chinese_variant(text))
    } else if share(cyrillic) {
        Some("Russian")
    } else if share(arabic) {
        Some("Arabic")
    } else if share(thai) {
        Some("Thai")
    } else if share(devanagari) {
        Some("Hindi")
    } else {
        None
    }
}

/// whatlang limited to the app's languages; `None` below [`MIN_CONFIDENCE`].
fn recognize(text: &str) -> Option<&'static str> {
    let detector = Detector::with_allowlist(WHATLANG.iter().map(|&(lang, _)| lang).collect());
    let info = detector.detect(text)?;
    if info.confidence() < MIN_CONFIDENCE {
        return None;
    }
    if info.lang() == Lang::Cmn {
        return Some(chinese_variant(text));
    }
    WHATLANG
        .iter()
        .find(|&&(lang, _)| lang == info.lang())
        .map(|&(_, name)| name)
}

/// Pairs of common characters that differ between Simplified and Traditional Chinese.
const SIMPLIFIED: &str = "们这说国会时来对个为学发后现经过还开关问么东长书车门见话语体电气应实点让认网读写听买卖钱边爱样头师从无动机进运觉谢请华儿业产处义";
const TRADITIONAL: &str = "們這說國會時來對個為學發後現經過還開關問麼東長書車門見話語體電氣應實點讓認網讀寫聽買賣錢邊愛樣頭師從無動機進運覺謝請華兒業產處義";

/// Simplified unless Traditional-only characters outnumber Simplified-only ones. Swift asks
/// `NLLanguageRecognizer` and falls back to Simplified too.
fn chinese_variant(text: &str) -> &'static str {
    let (mut simplified, mut traditional) = (0, 0);
    for c in text.chars() {
        if SIMPLIFIED.contains(c) {
            simplified += 1;
        } else if TRADITIONAL.contains(c) {
            traditional += 1;
        }
    }
    if traditional > simplified {
        "Chinese (Traditional)"
    } else {
        "Chinese (Simplified)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLES: &[(&str, &str)] = &[
        ("Korean", "오늘은 날씨가 좋아서 공원에 산책하러 갔습니다."),
        (
            "English",
            "I think we should meet tomorrow to discuss the project.",
        ),
        (
            "Japanese",
            "今日は天気が良かったので、公園へ散歩に行きました。",
        ),
        (
            "Chinese (Simplified)",
            "今天天气很好，我们去公园散步了，这是一个愉快的下午。",
        ),
        (
            "Chinese (Traditional)",
            "今天天氣很好，我們去公園散步了，這是一個愉快的下午。",
        ),
        (
            "Spanish",
            "El rápido zorro marrón salta sobre el perro perezoso cerca del río.",
        ),
        (
            "French",
            "Le renard brun rapide saute par-dessus le chien paresseux près de la rivière.",
        ),
        (
            "German",
            "Der schnelle braune Fuchs springt über den faulen Hund am Flussufer.",
        ),
        (
            "Portuguese",
            "A rápida raposa marrom pula sobre o cão preguiçoso perto do rio, não é?",
        ),
        ("Italian", "Dove si trova la stazione dei treni più vicina?"),
        (
            "Russian",
            "Быстрая коричневая лиса прыгает через ленивую собаку у реки.",
        ),
        (
            "Vietnamese",
            "Hôm nay trời đẹp nên chúng tôi đã đi dạo trong công viên.",
        ),
        ("Thai", "วันนี้อากาศดีมาก เราจึงไปเดินเล่นที่สวนสาธารณะ"),
        (
            "Indonesian",
            "Hari ini cuacanya sangat bagus, jadi kami berjalan-jalan di taman kota.",
        ),
        ("Hindi", "आज मौसम बहुत अच्छा था, इसलिए हम पार्क में टहलने गए।"),
        (
            "Arabic",
            "كان الطقس جميلا اليوم، لذلك ذهبنا للتنزه في الحديقة.",
        ),
    ];

    #[test]
    fn one_sample_per_language() {
        assert_eq!(SAMPLES.len(), LANGUAGES.len());
        for &(expected, text) in SAMPLES {
            assert_eq!(detect(text).as_deref(), Some(expected), "{text}");
        }
    }

    #[test]
    fn languages_match_swift_order() {
        let names: Vec<_> = LANGUAGES.iter().map(|l| l.name).collect();
        assert_eq!(
            names,
            [
                "Korean",
                "English",
                "Japanese",
                "Chinese (Simplified)",
                "Chinese (Traditional)",
                "Spanish",
                "French",
                "German",
                "Portuguese",
                "Italian",
                "Russian",
                "Vietnamese",
                "Thai",
                "Indonesian",
                "Hindi",
                "Arabic",
            ]
        );
    }

    #[test]
    fn script_decides_non_latin() {
        // Kanji with any kana is Japanese; Han alone is Chinese.
        assert_eq!(detect_by_script("東京大学の研究"), Some("Japanese"));
        assert_eq!(
            detect_by_script("东京大学研究"),
            Some("Chinese (Simplified)")
        );
        // Hangul wins even when mixed with Han and Latin identifiers.
        assert_eq!(detect_by_script("大韓民國 헌법 제1조"), Some("Korean"));
        assert_eq!(
            detect("이 함수는 config.json 파일을 읽어서 parse_settings 를 호출합니다").as_deref(),
            Some("Korean")
        );
        // Short non-Latin text is still decided by script, as in Swift.
        assert_eq!(detect("안녕").as_deref(), Some("Korean"));
        // Mostly Latin: the script vote abstains.
        assert_eq!(detect_by_script("Hello world, 안"), None);
    }

    #[test]
    fn short_or_letterless_is_none() {
        for text in ["", "   ", "12345 67.89", "-- !! ??", "Hi", "ok yes", "café"] {
            assert_eq!(detect(text), None, "{text:?}");
        }
    }

    #[test]
    fn never_outside_the_list() {
        let names: Vec<_> = LANGUAGES.iter().map(|l| l.name).collect();
        for &(_, name) in WHATLANG {
            assert!(names.contains(&name), "{name}");
        }
        // Languages whatlang knows but the app does not (Dutch, Polish, Turkish, Swedish, Ukrainian
        // in Cyrillic) must map to a listed language or nothing.
        for text in [
            "De snelle bruine vos springt over de luie hond bij de rivier.",
            "Szybki brązowy lis przeskakuje nad leniwym psem przy rzece.",
            "Hızlı kahverengi tilki nehrin yanındaki tembel köpeğin üzerinden atlar.",
            "Den snabba bruna räven hoppar över den lata hunden vid floden.",
            "Швидка руда лисиця перестрибує через ледачого пса біля річки.",
        ] {
            if let Some(found) = detect(text) {
                assert!(names.contains(&found.as_str()), "{text} -> {found}");
            }
        }
    }

    #[test]
    fn long_text_is_sampled() {
        let text = format!(
            "{}{}",
            "Bonjour tout le monde. ".repeat(100),
            "a".repeat(10_000)
        );
        assert_eq!(detect(&text).as_deref(), Some("French"));
    }
}
