import Foundation

/// Localized string lookup from the app bundle (Resources/<lang>.lproj/Localizable.strings).
/// Keys are the English strings, so missing translations fall back to English.
func L(_ key: String) -> String {
    NSLocalizedString(key, comment: "")
}

func L(_ key: String, _ args: CVarArg...) -> String {
    String(format: NSLocalizedString(key, comment: ""), arguments: args)
}
