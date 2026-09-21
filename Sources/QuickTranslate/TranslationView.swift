import SwiftUI

struct TranslationView: View {
    @ObservedObject var vm: TranslationViewModel
    @ObservedObject var settings: AppSettings
    var onClose: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            sourceArea
            Divider()
            resultArea
            Divider()
            footer
        }
        .frame(minWidth: 380, minHeight: 240)
    }

    private var header: some View {
        HStack(spacing: 8) {
            Image(systemName: "character.bubble")
                .foregroundColor(.accentColor)
            Text(verbatim: "QuickTranslate")
                .font(.system(size: 12, weight: .semibold))
                .foregroundColor(.secondary)
            Spacer()
            if let detected = vm.detectedLanguage {
                Text(LocalizedStringKey(detected))
                    .font(.system(size: 12))
            } else {
                Text("Auto-detect")
                    .font(.system(size: 12))
                    .foregroundColor(.secondary)
            }
            Image(systemName: "arrow.right")
                .font(.system(size: 10, weight: .semibold))
                .foregroundColor(.secondary)
            Picker("", selection: Binding(get: { vm.targetLanguage }, set: { vm.setTarget($0) })) {
                ForEach(AppSettings.languages, id: \.self) { Text(LocalizedStringKey($0)).tag($0) }
            }
            .labelsHidden()
            .controlSize(.small)
            .frame(width: 150)

            Button {
                vm.pinned.toggle()
            } label: {
                Image(systemName: vm.pinned ? "pin.fill" : "pin")
            }
            .buttonStyle(.borderless)
            .help(vm.pinned ? "Unpin" : "Pin window (stays open when clicking outside)")

            Button(action: onClose) {
                Image(systemName: "xmark.circle.fill")
                    .foregroundColor(.secondary)
            }
            .buttonStyle(.borderless)
            .keyboardShortcut("w", modifiers: .command)
            .help("Close (Esc)")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    private var sourceArea: some View {
        TextEditor(text: $vm.sourceText)
            .font(.system(size: 12))
            .foregroundColor(.secondary)
            .scrollContentBackground(.hidden)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .frame(height: 84)
    }

    private var resultArea: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                if let error = vm.errorMessage {
                    Label {
                        Text(verbatim: error).textSelection(.enabled)
                    } icon: {
                        Image(systemName: "exclamationmark.triangle.fill")
                    }
                    .font(.system(size: 12))
                    .foregroundColor(.red)
                } else if vm.translatedText.isEmpty && vm.isLoading {
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text("Translating…").foregroundColor(.secondary)
                    }
                    .font(.system(size: 13))
                } else if vm.translatedText.isEmpty && vm.cancelled {
                    Text("Cancelled").font(.system(size: 13)).foregroundColor(.secondary)
                } else {
                    Text(verbatim: vm.translatedText)
                        .font(.system(size: 14))
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .padding(12)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var footer: some View {
        HStack(spacing: 8) {
            Text(verbatim: engineLabel)
                .font(.system(size: 11))
                .foregroundColor(.secondary)
                .lineLimit(1)
            Spacer()
            if vm.isLoading {
                ProgressView().controlSize(.small)
                Button("Cancel") { vm.cancel() }
                    .keyboardShortcut(".", modifiers: .command)
                    .controlSize(.small)
                    .help("⌘.")
            } else if vm.cancelled {
                Text("Cancelled").font(.system(size: 11)).foregroundColor(.secondary)
            }
            Button("Retranslate") { vm.retranslate() }
                .keyboardShortcut(.return, modifiers: .command)
                .controlSize(.small)
                .help("⌘⏎")
            Button(vm.copied ? "Copied ✓" : "Copy") { vm.copyTranslation() }
                .keyboardShortcut("c", modifiers: [.command, .shift])
                .controlSize(.small)
                .disabled(vm.translatedText.isEmpty)
                .help("⇧⌘C")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    private var engineLabel: String {
        let model = settings.currentModel.trimmingCharacters(in: .whitespaces)
        let name = settings.backend == .claude ? "Claude" : "ChatGPT"
        return model.isEmpty ? name : "\(name) · \(model)"
    }
}
