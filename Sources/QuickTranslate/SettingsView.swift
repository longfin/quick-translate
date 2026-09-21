import SwiftUI
import ApplicationServices

struct SettingsView: View {
    @ObservedObject var settings: AppSettings
    @State private var accessibilityGranted = AXIsProcessTrusted()
    @State private var secureInputWarning: String? = SecureInput.warningText()
    @State private var detectedPath: String = ""
    @State private var testResult: String = ""
    @State private var testing = false
    private let engine = TranslationEngine.shared
    private let timer = Timer.publish(every: 2, on: .main, in: .common).autoconnect()

    var body: some View {
        Form {
            Section("Translation Engine") {
                Picker("Engine", selection: $settings.backend) {
                    ForEach(TranslationBackend.allCases) { Text(verbatim: $0.displayName).tag($0) }
                }
                switch settings.backend {
                case .claude:
                    TextField("Model (haiku / sonnet / opus)", text: $settings.claudeModel)
                    TextField("claude path (blank = auto-detect)", text: $settings.claudePath)
                    Toggle("Keep Claude running in the background (faster, ~300 MB RAM)", isOn: $settings.keepClaudeWarm)
                case .codex:
                    TextField("Model (blank = codex default)", text: $settings.codexModel)
                    TextField("codex path (blank = auto-detect)", text: $settings.codexPath)
                }
                HStack {
                    if detectedPath.isEmpty {
                        Text("CLI not found").font(.caption).foregroundColor(.red)
                    } else {
                        Text(verbatim: detectedPath).font(.caption).foregroundColor(.secondary).textSelection(.enabled)
                    }
                    Spacer()
                    Button("Test") { runTest() }.disabled(testing)
                }
                if !testResult.isEmpty {
                    Text(verbatim: testResult).font(.caption).textSelection(.enabled)
                }
            }

            Section("Languages") {
                Picker("Default target language", selection: $settings.targetLanguage) {
                    ForEach(AppSettings.languages, id: \.self) { Text(LocalizedStringKey($0)).tag($0) }
                }
                Picker("If already in that language, translate to", selection: $settings.fallbackLanguage) {
                    ForEach(AppSettings.languages, id: \.self) { Text(LocalizedStringKey($0)).tag($0) }
                }
                Picker("Interface language", selection: Binding(
                    get: { settings.uiLanguage },
                    set: { code in
                        guard code != settings.uiLanguage else { return }
                        settings.uiLanguage = code
                        (NSApp.delegate as? AppDelegate)?.relaunch()   // AppleLanguages is read at launch
                    })) {
                    ForEach(AppSettings.uiLanguages, id: \.code) { item in
                        if item.code == "system" {
                            Text("System default").tag(item.code)
                        } else {
                            Text(verbatim: item.name).tag(item.code)
                        }
                    }
                }
                Text("Changing the interface language restarts the app.")
                    .font(.caption).foregroundColor(.secondary)
            }

            Section("Hotkey & Window") {
                HotkeyRecorderRow(settings: settings)
                Picker("Trigger", selection: Binding(
                    get: { settings.hotkey.doublePress },
                    set: { settings.hotkey.doublePress = $0 })) {
                    Text("Press twice").tag(true)
                    Text("Press once").tag(false)
                }
                .pickerStyle(.segmented)
                if settings.hotkey.isCopyDoublePress {
                    Text("⌘C ⌘C is detected from the clipboard, so it works even while Secure Keyboard Entry is on and needs no Accessibility permission.")
                        .font(.caption).foregroundColor(.secondary)
                }
                if settings.hotkey.doublePress {
                    HStack {
                        Text("Double-press interval")
                        Slider(value: $settings.doublePressInterval, in: 0.2...1.0, step: 0.05)
                        Text(verbatim: L("%.2fs", settings.doublePressInterval))
                            .monospacedDigit()
                            .frame(width: 50, alignment: .trailing)
                    }
                } else {
                    Text("When pressed once, the selected text is copied automatically first.")
                        .font(.caption).foregroundColor(.secondary)
                }
                Toggle("Close when clicking outside", isOn: $settings.closeOnOutsideClick)
            }

            Section("Permissions") {
                HStack {
                    Image(systemName: accessibilityGranted ? "checkmark.circle.fill" : "xmark.circle.fill")
                        .foregroundColor(accessibilityGranted ? .green : .red)
                    if accessibilityGranted {
                        Text("Accessibility permission granted.")
                    } else if settings.hotkey.needsAccessibility {
                        Text("Accessibility permission is required for the global hotkey.")
                    } else {
                        Text("Not needed for ⌘C ⌘C. Required only for other shortcuts.")
                    }
                    Spacer()
                    if !accessibilityGranted && settings.hotkey.needsAccessibility {
                        Button("Open System Settings") { openAccessibilitySettings() }
                    }
                }
                if settings.hotkey.dependsOnKeyEvents, let warning = secureInputWarning {
                    HStack(alignment: .top) {
                        Image(systemName: "exclamationmark.triangle.fill").foregroundColor(.orange)
                        Text(verbatim: warning + " " + L("Turn off Secure Keyboard Entry in that app, or use the hotkey from another app."))
                    }
                }
            }
        }
        .formStyle(.grouped)
        .frame(width: 540, height: 500)
        .onAppear { refreshDetectedPath() }
        .onChange(of: settings.backend) { _ in refreshDetectedPath(); engine.prewarm(settings: settings) }
        .onChange(of: settings.keepClaudeWarm) { _ in engine.prewarm(settings: settings) }
        .onChange(of: settings.claudeModel) { _ in engine.prewarm(settings: settings) }
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
        testResult = L("Testing…")
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

/// Shows the current shortcut; "Change…" captures the next key combination pressed.
struct HotkeyRecorderRow: View {
    @ObservedObject var settings: AppSettings
    @State private var recording = false
    @State private var monitor: Any?

    var body: some View {
        HStack {
            Text("Shortcut")
            Spacer()
            Text(verbatim: recording ? L("Press the new shortcut…") : settings.hotkey.display)
                .font(.system(.body, design: .rounded).monospacedDigit())
                .padding(.horizontal, 10).padding(.vertical, 4)
                .background(RoundedRectangle(cornerRadius: 6).fill(Color(nsColor: .controlBackgroundColor)))
                .overlay(RoundedRectangle(cornerRadius: 6).stroke(recording ? Color.accentColor : Color.secondary.opacity(0.4)))
            Button(recording ? "Cancel" : "Change…") { recording ? stopRecording() : startRecording() }
            Button("Reset to ⌘C ⌘C") {
                stopRecording()
                settings.hotkey = .default
            }
            .disabled(settings.hotkey == .default)
        }
        .onDisappear { stopRecording() }
    }

    private func startRecording() {
        recording = true
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if event.keyCode == 53 { stopRecording(); return nil }           // Esc cancels
            guard var cfg = Hotkey.config(from: event), cfg.isUsable else { return nil }
            cfg.doublePress = settings.hotkey.doublePress
            settings.hotkey = cfg
            stopRecording()
            return nil
        }
    }

    private func stopRecording() {
        if let m = monitor { NSEvent.removeMonitor(m) }
        monitor = nil
        recording = false
    }
}
