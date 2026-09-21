import SwiftUI
import ApplicationServices

struct SettingsView: View {
    @ObservedObject var settings: AppSettings
    @State private var accessibilityGranted = AXIsProcessTrusted()
    @State private var secureInputWarning: String? = SecureInput.warningText()
    @State private var detectedPath: String = ""
    @State private var testResult: String = ""
    @State private var testing = false
    private let engine = TranslationEngine()
    private let timer = Timer.publish(every: 2, on: .main, in: .common).autoconnect()

    var body: some View {
        Form {
            Section("번역 엔진") {
                Picker("엔진", selection: $settings.backend) {
                    ForEach(TranslationBackend.allCases) { Text($0.displayName).tag($0) }
                }
                switch settings.backend {
                case .claude:
                    TextField("모델 (haiku / sonnet / opus)", text: $settings.claudeModel)
                    TextField("claude 경로 (비우면 자동 탐색)", text: $settings.claudePath)
                case .codex:
                    TextField("모델 (비우면 codex 기본값)", text: $settings.codexModel)
                    TextField("codex 경로 (비우면 자동 탐색)", text: $settings.codexPath)
                }
                HStack {
                    Text(detectedPath.isEmpty ? "CLI를 찾을 수 없습니다" : detectedPath)
                        .font(.caption)
                        .foregroundColor(detectedPath.isEmpty ? .red : .secondary)
                        .textSelection(.enabled)
                    Spacer()
                    Button("테스트") { runTest() }.disabled(testing)
                }
                if !testResult.isEmpty {
                    Text(testResult).font(.caption).textSelection(.enabled)
                }
            }

            Section("언어") {
                Picker("기본 번역 언어", selection: $settings.targetLanguage) {
                    ForEach(AppSettings.languages, id: \.self) { Text($0).tag($0) }
                }
                Picker("이미 그 언어이면 →", selection: $settings.fallbackLanguage) {
                    ForEach(AppSettings.languages, id: \.self) { Text($0).tag($0) }
                }
            }

            Section("단축키 / 창") {
                HStack {
                    Text("⌘C 두 번 인식 간격")
                    Slider(value: $settings.doublePressInterval, in: 0.2...1.0, step: 0.05)
                    Text(String(format: "%.2f초", settings.doublePressInterval))
                        .monospacedDigit()
                        .frame(width: 50, alignment: .trailing)
                }
                Toggle("창 바깥을 클릭하면 닫기", isOn: $settings.closeOnOutsideClick)
            }

            Section("권한") {
                HStack {
                    Image(systemName: accessibilityGranted ? "checkmark.circle.fill" : "xmark.circle.fill")
                        .foregroundColor(accessibilityGranted ? .green : .red)
                    Text(accessibilityGranted
                         ? "손쉬운 사용(Accessibility) 권한이 허용되었습니다."
                         : "전역 단축키를 쓰려면 손쉬운 사용 권한이 필요합니다.")
                    Spacer()
                    if !accessibilityGranted {
                        Button("설정 열기") { openAccessibilitySettings() }
                    }
                }
                if let warning = secureInputWarning {
                    HStack(alignment: .top) {
                        Image(systemName: "exclamationmark.triangle.fill").foregroundColor(.orange)
                        Text(warning + ". 해당 앱에서 Secure Keyboard Entry를 끄거나 다른 앱에서 사용하세요.")
                    }
                }
            }
        }
        .formStyle(.grouped)
        .frame(width: 520, height: 480)
        .onAppear { refreshDetectedPath() }
        .onChange(of: settings.backend) { _ in refreshDetectedPath() }
        .onChange(of: settings.claudePath) { _ in refreshDetectedPath() }
        .onChange(of: settings.codexPath) { _ in refreshDetectedPath() }
        .onReceive(timer) { _ in
            accessibilityGranted = AXIsProcessTrusted()
            secureInputWarning = SecureInput.warningText()
        }
    }

    private func refreshDetectedPath() {
        CLILocator.resetCache()
        detectedPath = CLILocator.find(settings.backend.executableName, override: settings.currentPathOverride) ?? ""
    }

    private func runTest() {
        testing = true
        testResult = "테스트 중…"
        var acc = ""
        engine.translate(text: "The quick brown fox jumps over the lazy dog.",
                         target: settings.targetLanguage,
                         fallback: settings.fallbackLanguage,
                         settings: settings) { event in
            switch event {
            case .delta(let t): acc += t; testResult = acc
            case .finished(let s): testResult = "✓ " + (s.isEmpty ? acc : s); testing = false
            case .failed(let e): testResult = "✗ " + e; testing = false
            }
        }
    }
}

func openAccessibilitySettings() {
    if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") {
        NSWorkspace.shared.open(url)
    }
}
