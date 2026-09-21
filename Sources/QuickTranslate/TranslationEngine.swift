import Foundation

enum TranslationEvent {
    case delta(String)
    case finished(String)
    case failed(String)
}

/// Talks to the locally installed `claude` (Claude subscription) or `codex` (ChatGPT subscription) CLI.
final class TranslationEngine {

    static func systemPrompt(source: String?, target: String, fallback: String) -> String {
        let task: String
        if let source, source != target {
            task = "Translate the text provided by the user from \(source) into \(target)."
        } else {
            task = """
            Translate the text provided by the user into \(target).
            If the text is already written mostly in \(target), translate it into \(fallback) instead.
            """
        }
        return """
        You are a professional translation engine.
        \(task)
        Preserve the original meaning, tone, formatting, line breaks, markdown and code blocks. Do not translate code identifiers, URLs or proper nouns that are normally left as-is.
        Output ONLY the translated text. No explanations, no notes, no quotes, no preamble.
        """
    }

    /// Starts a translation. Events are delivered on the main queue.
    @discardableResult
    func translate(text: String,
                   source: String? = nil,
                   target: String,
                   fallback: String,
                   settings: AppSettings,
                   onEvent: @escaping (TranslationEvent) -> Void) -> ProcessJob? {
        let emit: (TranslationEvent) -> Void = { e in DispatchQueue.main.async { onEvent(e) } }
        let prompt = Self.systemPrompt(source: source, target: target, fallback: fallback)
        let backend = settings.backend

        guard let exe = CLILocator.find(backend.executableName, override: settings.currentPathOverride) else {
            emit(.failed(L("`%@` CLI not found. Install it or set its path in Settings.", backend.executableName)))
            return nil
        }

        Log.write("using \(backend.rawValue) at \(exe)")
        switch backend {
        case .claude:
            return runClaude(exe: exe, prompt: prompt, text: text, model: settings.claudeModel, emit: emit)
        case .codex:
            return runCodex(exe: exe, prompt: prompt, text: text, model: settings.codexModel, emit: emit)
        }
    }

    // MARK: - Claude Code CLI

    private func runClaude(exe: String, prompt: String, text: String, model: String,
                           emit: @escaping (TranslationEvent) -> Void) -> ProcessJob? {
        var args = [
            "-p",
            "--output-format", "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--tools", "",                 // no tools: pure text generation
            "--strict-mcp-config",         // skip user MCP servers (fast startup)
            "--setting-sources", "",       // skip user settings / hooks
            "--no-session-persistence",
            "--system-prompt", prompt,
        ]
        let m = model.trimmingCharacters(in: .whitespaces)
        if !m.isEmpty { args += ["--model", m] }

        let state = ResultState()
        let job = ProcessJob(
            executable: exe,
            arguments: args,
            environment: CLILocator.environment(),
            currentDirectory: Self.scratchDirectory(),
            input: text.hasSuffix("\n") ? text : text + "\n",
            onLine: { line in
                guard let data = line.data(using: .utf8),
                      let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
                      let type = obj["type"] as? String else { return }
                switch type {
                case "stream_event":
                    if let ev = obj["event"] as? [String: Any],
                       ev["type"] as? String == "content_block_delta",
                       let delta = ev["delta"] as? [String: Any],
                       delta["type"] as? String == "text_delta",
                       let t = delta["text"] as? String {
                        emit(.delta(t))
                    }
                case "result":
                    state.gotResult = true
                    let isError = obj["is_error"] as? Bool ?? false
                    let result = (obj["result"] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                    if isError {
                        let errors = (obj["errors"] as? [String])?.joined(separator: "\n") ?? ""
                        emit(.failed(result.isEmpty ? (errors.isEmpty ? L("Claude returned an error.") : errors) : result))
                    } else {
                        emit(.finished(result))
                    }
                default:
                    break
                }
            },
            onExit: { status, stderr in
                if !state.gotResult {
                    let tail = Self.tail(stderr)
                    emit(.failed(tail.isEmpty ? L("claude exited with code %d", status) : tail))
                }
            })
        do { try job.start() } catch {
            emit(.failed(L("Failed to launch claude: %@", error.localizedDescription)))
            return nil
        }
        return job
    }

    // MARK: - Codex CLI

    private func runCodex(exe: String, prompt: String, text: String, model: String,
                          emit: @escaping (TranslationEvent) -> Void) -> ProcessJob? {
        let dir = Self.scratchDirectory().appendingPathComponent("codex-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let outFile = dir.appendingPathComponent("last-message.txt")

        var args = [
            "exec",
            "--skip-git-repo-check",
            "-s", "read-only",
            "-C", dir.path,
            "-o", outFile.path,
        ]
        let m = model.trimmingCharacters(in: .whitespaces)
        if !m.isEmpty { args += ["-m", m] }
        args.append(prompt + "\n\nText to translate:\n\n" + text)

        let job = ProcessJob(
            executable: exe,
            arguments: args,
            environment: CLILocator.environment(),
            currentDirectory: dir,
            input: nil,
            onLine: { _ in },
            onExit: { status, stderr in
                defer { try? FileManager.default.removeItem(at: dir) }
                let result = (try? String(contentsOf: outFile, encoding: .utf8))?
                    .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
                if !result.isEmpty {
                    emit(.finished(result))
                } else {
                    let tail = Self.tail(stderr)
                    emit(.failed(tail.isEmpty ? L("codex exited with code %d", status) : tail))
                }
            })
        do { try job.start() } catch {
            emit(.failed(L("Failed to launch codex: %@", error.localizedDescription)))
            return nil
        }
        return job
    }

    // MARK: - Helpers

    private final class ResultState {
        var gotResult = false
    }

    private static func scratchDirectory() -> URL {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("QuickTranslate", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    private static func tail(_ s: String, lines: Int = 6) -> String {
        let all = s.split(separator: "\n").map(String.init).filter { !$0.trimmingCharacters(in: .whitespaces).isEmpty }
        return all.suffix(lines).joined(separator: "\n")
    }
}
