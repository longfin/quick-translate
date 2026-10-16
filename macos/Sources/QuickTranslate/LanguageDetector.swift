import Foundation
import NaturalLanguage

/// On-device source language detection (NaturalLanguage framework), mapped to the app's language names.
enum LanguageDetector {
    private static let byNLLanguage: [NLLanguage: String] = [
        .korean: "Korean", .english: "English", .japanese: "Japanese",
        .simplifiedChinese: "Chinese (Simplified)", .traditionalChinese: "Chinese (Traditional)",
        .spanish: "Spanish", .french: "French", .german: "German", .portuguese: "Portuguese",
        .italian: "Italian", .russian: "Russian", .vietnamese: "Vietnamese", .thai: "Thai",
        .indonesian: "Indonesian", .hindi: "Hindi", .arabic: "Arabic",
    ]

    /// Returns one of `AppSettings.languages`, or nil when undetermined.
    static func detect(_ text: String) -> String? {
        let sample = String(text.prefix(2000))
        guard sample.contains(where: { $0.isLetter }) else { return nil }
        if let language = detectByScript(sample) { return language }
        return recognize(sample, among: Array(byNLLanguage.keys))
    }

    /// NLLanguageRecognizer is swayed by letter count, so Korean prose mixed with code or English
    /// identifiers can come out as "Italian, 22%". One non-Latin letter carries far more signal than
    /// a Latin one, so when a non-Latin script makes up a good share of the letters, trust the script.
    private static func detectByScript(_ text: String) -> String? {
        var latin = 0, hangul = 0, kana = 0, han = 0, cyrillic = 0, arabic = 0, thai = 0, devanagari = 0
        for scalar in text.unicodeScalars where scalar.properties.isAlphabetic {
            switch scalar.value {
            case 0xAC00...0xD7A3, 0x1100...0x11FF, 0x3130...0x318F: hangul += 1
            case 0x3040...0x30FF: kana += 1
            case 0x4E00...0x9FFF, 0x3400...0x4DBF: han += 1
            case 0x0400...0x04FF: cyrillic += 1
            case 0x0600...0x06FF: arabic += 1
            case 0x0E00...0x0E7F: thai += 1
            case 0x0900...0x097F: devanagari += 1
            case ..<0x0250: latin += 1
            default: break
            }
        }
        let total = latin + hangul + kana + han + cyrillic + arabic + thai + devanagari
        guard total > 0 else { return nil }
        let threshold = Double(total) * 0.3
        if Double(hangul) >= threshold { return "Korean" }
        if kana > 0 && Double(kana + han) >= threshold { return "Japanese" }
        if Double(han) >= threshold {
            return recognize(text, among: [.simplifiedChinese, .traditionalChinese]) ?? "Chinese (Simplified)"
        }
        if Double(cyrillic) >= threshold { return "Russian" }
        if Double(arabic) >= threshold { return "Arabic" }
        if Double(thai) >= threshold { return "Thai" }
        if Double(devanagari) >= threshold { return "Hindi" }
        return nil
    }

    private static func recognize(_ text: String, among languages: [NLLanguage]) -> String? {
        let recognizer = NLLanguageRecognizer()
        recognizer.languageConstraints = languages
        recognizer.processString(text)
        guard let (lang, confidence) = recognizer.languageHypotheses(withMaximum: 1).first,
              confidence >= 0.5 else { return nil }
        return byNLLanguage[lang]
    }
}
