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
            Text("QuickTranslate")
                .font(.system(size: 12, weight: .semibold))
                .foregroundColor(.secondary)
            Spacer()
            Picker("", selection: $vm.targetLanguage) {
                ForEach(AppSettings.languages, id: \.self) { Text($0).tag($0) }
            }
            .labelsHidden()
            .controlSize(.small)
            .frame(width: 160)
            .onChange(of: vm.targetLanguage) { _ in vm.retranslate() }

            Button {
                vm.pinned.toggle()
            } label: {
                Image(systemName: vm.pinned ? "pin.fill" : "pin")
            }
            .buttonStyle(.borderless)
            .help(vm.pinned ? "고정 해제" : "창 고정 (바깥 클릭 시 닫히지 않음)")

            Button(action: onClose) {
                Image(systemName: "xmark.circle.fill")
                    .foregroundColor(.secondary)
            }
            .buttonStyle(.borderless)
            .keyboardShortcut("w", modifiers: .command)
            .help("닫기 (Esc)")
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
                        Text(error).textSelection(.enabled)
                    } icon: {
                        Image(systemName: "exclamationmark.triangle.fill")
                    }
                    .font(.system(size: 12))
                    .foregroundColor(.red)
                } else if vm.translatedText.isEmpty && vm.isLoading {
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text("번역 중…").foregroundColor(.secondary)
                    }
                    .font(.system(size: 13))
                } else {
                    Text(vm.translatedText)
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
            Text(engineLabel)
                .font(.system(size: 11))
                .foregroundColor(.secondary)
                .lineLimit(1)
            Spacer()
            if vm.isLoading {
                ProgressView().controlSize(.small)
            }
            Button("다시 번역") { vm.retranslate() }
                .keyboardShortcut(.return, modifiers: .command)
                .controlSize(.small)
                .help("⌘⏎")
            Button(vm.copied ? "복사됨 ✓" : "복사") { vm.copyTranslation() }
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
