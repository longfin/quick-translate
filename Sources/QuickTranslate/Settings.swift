import Foundation
import Combine

enum TranslationBackend: String, CaseIterable, Identifiable {
    case claude
    case codex

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .claude: return "Claude (Claude Code CLI)"
        case .codex: return "ChatGPT (Codex CLI)"
        }
    }

    var executableName: String {
        switch self {
        case .claude: return "claude"
        case .codex: return "codex"
        }
    }
}

/// UserDefaults-backed app settings.
final class AppSettings: ObservableObject {
    static let shared = AppSettings()

    static let languages: [String] = [
        "Korean", "English", "Japanese", "Chinese (Simplified)", "Chinese (Traditional)",
        "Spanish", "French", "German", "Portuguese", "Italian", "Russian",
        "Vietnamese", "Thai", "Indonesian", "Hindi", "Arabic",
    ]

    private let defaults = UserDefaults.standard

    @Published var backend: TranslationBackend { didSet { defaults.set(backend.rawValue, forKey: "backend") } }
    @Published var claudeModel: String { didSet { defaults.set(claudeModel, forKey: "claudeModel") } }
    @Published var codexModel: String { didSet { defaults.set(codexModel, forKey: "codexModel") } }
    @Published var targetLanguage: String { didSet { defaults.set(targetLanguage, forKey: "targetLanguage") } }
    @Published var fallbackLanguage: String { didSet { defaults.set(fallbackLanguage, forKey: "fallbackLanguage") } }
    @Published var claudePath: String { didSet { defaults.set(claudePath, forKey: "claudePath") } }
    @Published var codexPath: String { didSet { defaults.set(codexPath, forKey: "codexPath") } }
    @Published var keepClaudeWarm: Bool { didSet { defaults.set(keepClaudeWarm, forKey: "keepClaudeWarm") } }
    @Published var hotkey: HotkeyConfig {
        didSet {
            defaults.set(Int(hotkey.keyCode), forKey: "hotkeyKeyCode")
            defaults.set(String(hotkey.modifiers), forKey: "hotkeyModifiers")
            defaults.set(hotkey.doublePress, forKey: "hotkeyDoublePress")
        }
    }
    @Published var doublePressInterval: Double { didSet { defaults.set(doublePressInterval, forKey: "doublePressInterval") } }
    @Published var closeOnOutsideClick: Bool { didSet { defaults.set(closeOnOutsideClick, forKey: "closeOnOutsideClick") } }

    /// UI language override: "system" or a localization code ("en", "ko", "ja").
    /// Applied through AppleLanguages, which macOS reads at launch, so changing it relaunches the app.
    @Published var uiLanguage: String {
        didSet {
            defaults.set(uiLanguage, forKey: "uiLanguage")
            if uiLanguage == "system" {
                defaults.removeObject(forKey: "AppleLanguages")
            } else {
                defaults.set([uiLanguage], forKey: "AppleLanguages")
            }
        }
    }
    static let uiLanguages: [(code: String, name: String)] = [
        ("system", "System default"), ("en", "English"), ("ko", "한국어"), ("ja", "日本語"),
    ]

    private init() {
        backend = TranslationBackend(rawValue: defaults.string(forKey: "backend") ?? "") ?? .claude
        claudeModel = defaults.string(forKey: "claudeModel") ?? "haiku"
        codexModel = defaults.string(forKey: "codexModel") ?? ""
        let systemDefaults = Self.systemDefaultLanguages()
        targetLanguage = defaults.string(forKey: "targetLanguage") ?? systemDefaults.target
        fallbackLanguage = defaults.string(forKey: "fallbackLanguage") ?? systemDefaults.fallback
        claudePath = defaults.string(forKey: "claudePath") ?? ""
        codexPath = defaults.string(forKey: "codexPath") ?? ""
        keepClaudeWarm = defaults.object(forKey: "keepClaudeWarm") as? Bool ?? true
        var hk = HotkeyConfig.default
        if let code = defaults.object(forKey: "hotkeyKeyCode") as? Int { hk.keyCode = UInt16(code) }
        if let mods = defaults.string(forKey: "hotkeyModifiers"), let m = UInt64(mods) { hk.modifiers = m }
        if let dbl = defaults.object(forKey: "hotkeyDoublePress") as? Bool { hk.doublePress = dbl }
        hotkey = hk
        doublePressInterval = defaults.object(forKey: "doublePressInterval") as? Double ?? 0.4
        closeOnOutsideClick = defaults.object(forKey: "closeOnOutsideClick") as? Bool ?? true
        uiLanguage = defaults.string(forKey: "uiLanguage") ?? "system"
    }

    /// First-launch defaults: translate into the user's system language; if the text is already in it,
    /// translate into their next preferred language (or English).
    static func systemDefaultLanguages() -> (target: String, fallback: String) {
        let byCode: [String: String] = [
            "ko": "Korean", "en": "English", "ja": "Japanese", "zh-Hans": "Chinese (Simplified)",
            "zh-Hant": "Chinese (Traditional)", "es": "Spanish", "fr": "French", "de": "German",
            "pt": "Portuguese", "it": "Italian", "ru": "Russian", "vi": "Vietnamese", "th": "Thai",
            "id": "Indonesian", "hi": "Hindi", "ar": "Arabic",
        ]
        var found: [String] = []
        for id in Locale.preferredLanguages {
            let lang = Locale(identifier: id).language
            let code = lang.languageCode?.identifier ?? ""
            let key = code == "zh" ? (lang.script?.identifier == "Hant" ? "zh-Hant" : "zh-Hans") : code
            if let name = byCode[key], !found.contains(name) { found.append(name) }
        }
        let target = found.first ?? "English"
        let fallback = found.dropFirst().first ?? (target == "English" ? "Korean" : "English")
        return (target, fallback)
    }

    var currentModel: String {
        switch backend {
        case .claude: return claudeModel
        case .codex: return codexModel
        }
    }

    var currentPathOverride: String {
        switch backend {
        case .claude: return claudePath
        case .codex: return codexPath
        }
    }
}
