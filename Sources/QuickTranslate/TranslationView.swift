import SwiftUI

struct TranslationView: View {
    @ObservedObject var vm: TranslationViewModel
    @ObservedObject var settings: AppSettings

    var body: some View {
        VStack(spacing: 0) {
            sourceArea
            Divider()
            resultArea
            Divider()
            footer
        }
        .frame(minWidth: 380, minHeight: 240)
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
                    .help("⌘.")
            } else if vm.cancelled {
                Text("Cancelled").font(.system(size: 11)).foregroundColor(.secondary)
            }
            Button { vm.retranslate() } label: {
                Label("Retranslate", systemImage: "arrow.clockwise")
            }
            .keyboardShortcut(.return, modifiers: .command)
            .help("⌘⏎")
            // The primary action. The title stays "Copy" so the button does not change width;
            // only the icon flips to a checkmark for feedback.
            Button { vm.copyTranslation() } label: {
                Label("Copy", systemImage: vm.copied ? "checkmark" : "doc.on.doc")
                    .modifier(SymbolReplaceTransition())
            }
            .keyboardShortcut("c", modifiers: [.command, .shift])
            .modifier(ProminentButtonStyle())
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

/// Liquid Glass prominent style on macOS 26+, the filled accent style before that.
private struct ProminentButtonStyle: ViewModifier {
    func body(content: Content) -> some View {
        if #available(macOS 26, *) {
            content.buttonStyle(.glassProminent)
        } else {
            content.buttonStyle(.borderedProminent)
        }
    }
}

private struct SymbolReplaceTransition: ViewModifier {
    func body(content: Content) -> some View {
        if #available(macOS 14, *) {
            content.contentTransition(.symbolEffect(.replace))
        } else {
            content
        }
    }
}
