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
        let recognizer = NLLanguageRecognizer()
        recognizer.languageConstraints = Array(byNLLanguage.keys)
        recognizer.processString(sample)
        guard let (lang, confidence) = recognizer.languageHypotheses(withMaximum: 1).first,
              confidence >= 0.5 else { return nil }
        return byNLLanguage[lang]
    }
}
