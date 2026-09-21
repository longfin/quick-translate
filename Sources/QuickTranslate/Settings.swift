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
    @Published var doublePressInterval: Double { didSet { defaults.set(doublePressInterval, forKey: "doublePressInterval") } }
    @Published var closeOnOutsideClick: Bool { didSet { defaults.set(closeOnOutsideClick, forKey: "closeOnOutsideClick") } }

    private init() {
        backend = TranslationBackend(rawValue: defaults.string(forKey: "backend") ?? "") ?? .claude
        claudeModel = defaults.string(forKey: "claudeModel") ?? "haiku"
        codexModel = defaults.string(forKey: "codexModel") ?? ""
        targetLanguage = defaults.string(forKey: "targetLanguage") ?? "Korean"
        fallbackLanguage = defaults.string(forKey: "fallbackLanguage") ?? "English"
        claudePath = defaults.string(forKey: "claudePath") ?? ""
        codexPath = defaults.string(forKey: "codexPath") ?? ""
        doublePressInterval = defaults.object(forKey: "doublePressInterval") as? Double ?? 0.4
        closeOnOutsideClick = defaults.object(forKey: "closeOnOutsideClick") as? Bool ?? true
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
