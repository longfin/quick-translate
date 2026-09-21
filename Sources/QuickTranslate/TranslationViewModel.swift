import AppKit
import Combine

final class TranslationViewModel: ObservableObject {
    @Published var sourceText = ""
    @Published var translatedText = ""
    @Published var isLoading = false
    @Published var errorMessage: String?
    @Published var targetLanguage: String
    @Published var pinned = false
    @Published var copied = false

    let settings: AppSettings
    private let engine = TranslationEngine()
    private var job: ProcessJob?
    private var generation = 0
    private var copiedResetWork: DispatchWorkItem?

    init(settings: AppSettings) {
        self.settings = settings
        self.targetLanguage = settings.targetLanguage
    }

    func translate(text: String) {
        sourceText = text
        retranslate()
    }

    func retranslate() {
        cancel()
        generation += 1
        let gen = generation
        translatedText = ""
        errorMessage = nil
        copied = false

        let text = sourceText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else {
            errorMessage = L("Nothing to translate. Select text and press ⌘C twice.")
            return
        }
        isLoading = true
        Log.write("translate start backend=\(settings.backend.rawValue) target=\(targetLanguage) chars=\(text.count)")

        let fallback = targetLanguage == settings.fallbackLanguage
            ? settings.targetLanguage
            : settings.fallbackLanguage

        job = engine.translate(text: text, target: targetLanguage, fallback: fallback, settings: settings) { [weak self] event in
            guard let self, gen == self.generation else { return }
            switch event {
            case .delta(let t):
                self.translatedText += t
            case .finished(let full):
                if !full.isEmpty { self.translatedText = full }
                Log.write("translate finished chars=\(self.translatedText.count)")
                self.isLoading = false
                self.job = nil
            case .failed(let msg):
                self.errorMessage = msg
                Log.write("translate failed: \(msg)")
                self.isLoading = false
                self.job = nil
            }
        }
    }

    func cancel() {
        job?.cancel()
        job = nil
        isLoading = false
    }

    func copyTranslation() {
        guard !translatedText.isEmpty else { return }
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(translatedText, forType: .string)
        copied = true
        copiedResetWork?.cancel()
        let work = DispatchWorkItem { [weak self] in self?.copied = false }
        copiedResetWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5, execute: work)
    }
}
