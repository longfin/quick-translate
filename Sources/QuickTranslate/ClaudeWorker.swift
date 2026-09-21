import Foundation

/// A resident `claude -p --input-format stream-json` process. Keeping it alive skips the ~0.6 s CLI
/// startup and keeps the HTTP connection and prompt cache warm, so a translation takes ~0.7 s instead
/// of ~2.2 s. After every turn we send `/clear`, which resets the conversation locally (no API call),
/// so each translation still starts from a fresh context.
final class ClaudeWorker {
    static let shared = ClaudeWorker()

    private struct Config: Equatable { let exe: String; let model: String }
    private struct Turn { let id: Int; let onEvent: (TranslationEvent) -> Void }

    private let queue = DispatchQueue(label: "quicktranslate.claude-worker")
    private var process: Process?
    private var stdinHandle: FileHandle?
    private var config: Config?
    private var current: Turn?
    private var nextTurnID = 0
    private var stderrTail = ""
    private var processID: Int32 = 0

    static let systemPrompt = """
        You are a professional translation engine.
        Each user message starts with an instruction line ending in a colon, followed by the text to translate.
        Preserve the original meaning, tone, formatting, line breaks, markdown and code blocks. Do not translate code identifiers, URLs or proper nouns that are normally left as-is.
        Output ONLY the translated text. No explanations, no notes, no quotes, no preamble.
        """

    /// Handle returned to the caller; cancelling drops the process (a turn cannot be interrupted).
    final class TurnHandle: TranslationJob {
        fileprivate let id: Int
        fileprivate unowned let worker: ClaudeWorker
        fileprivate init(id: Int, worker: ClaudeWorker) { self.id = id; self.worker = worker }
        func cancel() { worker.cancel(turnID: id) }
    }

    // MARK: - Public

    /// Spawn ahead of time so the first translation is fast too.
    func prewarm(exe: String, model: String) {
        queue.async { self.ensureProcess(Config(exe: exe, model: model)) }
    }

    func shutdown() {
        queue.async { self.killProcess() }
    }

    func translate(exe: String, model: String, instruction: String, text: String,
                   onEvent: @escaping (TranslationEvent) -> Void) -> TurnHandle {
        let id = queue.sync { () -> Int in nextTurnID += 1; return nextTurnID }
        queue.async {
            if self.current != nil {
                // A turn is still running: a stream-json session can't be interrupted, so start over.
                self.current = nil
                self.killProcess()
            }
            self.ensureProcess(Config(exe: exe, model: model))
            guard let stdin = self.stdinHandle else {
                onEvent(.failed(L("Failed to launch claude: %@", self.stderrTail.isEmpty ? "spawn failed" : self.stderrTail)))
                return
            }
            self.current = Turn(id: id, onEvent: onEvent)
            let content = instruction + "\n\n" + text
            self.send(["type": "user", "message": ["role": "user", "content": content]], to: stdin)
        }
        return TurnHandle(id: id, worker: self)
    }

    // MARK: - Internals (all on `queue`)

    private func cancel(turnID: Int) {
        queue.async {
            guard let cur = self.current, cur.id == turnID else { return }
            self.current = nil
            self.killProcess()
            if let cfg = self.config { self.ensureProcess(cfg) }   // respawn so the next request is warm
        }
    }

    private func ensureProcess(_ cfg: Config) {
        if let p = process, p.isRunning, config == cfg { return }
        killProcess()
        config = cfg

        let p = Process()
        p.executableURL = URL(fileURLWithPath: cfg.exe)
        var args = [
            "-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--include-partial-messages",
            "--tools", "", "--strict-mcp-config", "--setting-sources", "", "--no-session-persistence",
            "--system-prompt", Self.systemPrompt,
        ]
        if !cfg.model.isEmpty { args += ["--model", cfg.model] }
        p.arguments = args
        var env = CLILocator.environment()
        env["MAX_THINKING_TOKENS"] = "0"
        p.environment = env
        p.currentDirectoryURL = FileManager.default.temporaryDirectory

        let stdin = Pipe(), stdout = Pipe(), stderr = Pipe()
        p.standardInput = stdin; p.standardOutput = stdout; p.standardError = stderr
        do { try p.run() } catch {
            Log.write("claude worker spawn failed: \(error.localizedDescription)")
            stderrTail = error.localizedDescription
            return
        }
        process = p
        stdinHandle = stdin.fileHandleForWriting
        processID = p.processIdentifier
        stderrTail = ""
        Log.write("claude worker spawned pid=\(p.processIdentifier) model=\(cfg.model)")

        let pid = p.processIdentifier
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let handle = stdout.fileHandleForReading
            var buffer = Data()
            while true {
                let chunk = handle.availableData
                if chunk.isEmpty { break }
                buffer.append(chunk)
                while let idx = buffer.firstIndex(of: 0x0A) {
                    let lineData = buffer.subdata(in: buffer.startIndex..<idx)
                    buffer.removeSubrange(buffer.startIndex...idx)
                    if let line = String(data: lineData, encoding: .utf8), !line.isEmpty {
                        self?.queue.async { self?.handle(line: line, from: pid) }
                    }
                }
            }
            p.waitUntilExit()
            self?.queue.async { self?.processExited(pid: pid, status: p.terminationStatus) }
        }
        DispatchQueue.global(qos: .utility).async { [weak self] in
            let handle = stderr.fileHandleForReading
            while true {
                let chunk = handle.availableData
                if chunk.isEmpty { break }
                if let s = String(data: chunk, encoding: .utf8) {
                    self?.queue.async { self?.stderrTail = String((self?.stderrTail ?? "" + s).suffix(2000)) }
                }
            }
        }
    }

    private func killProcess() {
        guard let p = process else { return }
        try? stdinHandle?.close()
        if p.isRunning { p.terminate() }
        process = nil
        stdinHandle = nil
        processID = 0
    }

    private func send(_ object: [String: Any], to stdin: FileHandle) {
        guard var data = try? JSONSerialization.data(withJSONObject: object) else { return }
        data.append(0x0A)
        try? stdin.write(contentsOf: data)
    }

    private func handle(line: String, from pid: Int32) {
        guard pid == processID,
              let data = line.data(using: .utf8),
              let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let type = obj["type"] as? String else { return }
        switch type {
        case "stream_event":
            guard let cur = current,
                  let ev = obj["event"] as? [String: Any], ev["type"] as? String == "content_block_delta",
                  let delta = ev["delta"] as? [String: Any], delta["type"] as? String == "text_delta",
                  let t = delta["text"] as? String else { return }
            cur.onEvent(.delta(t))
        case "result":
            guard let cur = current else { return }
            current = nil
            let isError = obj["is_error"] as? Bool ?? false
            let result = (obj["result"] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            if isError {
                let errors = (obj["errors"] as? [String])?.joined(separator: "\n") ?? ""
                cur.onEvent(.failed(result.isEmpty ? (errors.isEmpty ? L("Claude returned an error.") : errors) : result))
            } else {
                cur.onEvent(.finished(result))
            }
            // Fresh context for the next translation; handled locally by the CLI, no API call.
            if let stdin = stdinHandle { send(["type": "user", "message": ["role": "user", "content": "/clear"]], to: stdin) }
        default:
            break
        }
    }

    private func processExited(pid: Int32, status: Int32) {
        guard pid == processID else { return }   // an old process we already replaced
        Log.write("claude worker exited status=\(status)")
        process = nil
        stdinHandle = nil
        processID = 0
        if let cur = current {
            current = nil
            let tail = stderrTail.split(separator: "\n").suffix(4).joined(separator: "\n")
            cur.onEvent(.failed(tail.isEmpty ? L("claude exited with code %d", status) : tail))
        }
    }
}
