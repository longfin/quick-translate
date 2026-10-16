import Foundation

/// Finds `claude` / `codex` binaries. GUI apps launched from Finder get a minimal PATH,
/// so we probe the usual install locations and fall back to a login shell lookup.
enum CLILocator {
    private static var cache: [String: String] = [:]
    private static let lock = NSLock()

    static var home: String { NSHomeDirectory() }

    static var candidateDirs: [String] {
        var dirs = [
            "\(home)/.local/bin",
            "\(home)/.npm/bin",
            "\(home)/.npm-global/bin",
            "\(home)/.claude/local",
            "\(home)/.claude/local/node_modules/.bin",
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "\(home)/.volta/bin",
            "\(home)/.bun/bin",
            "\(home)/.cargo/bin",
            "\(home)/.yarn/bin",
        ]
        let nvm = "\(home)/.nvm/versions/node"
        if let versions = try? FileManager.default.contentsOfDirectory(atPath: nvm) {
            for v in versions.sorted().reversed() { dirs.append("\(nvm)/\(v)/bin") }
        }
        dirs += ["/usr/bin", "/bin"]
        return dirs
    }

    static func expand(_ path: String) -> String {
        (path as NSString).expandingTildeInPath
    }

    static func find(_ name: String, override: String = "") -> String? {
        let manual = override.trimmingCharacters(in: .whitespacesAndNewlines)
        if !manual.isEmpty {
            let p = expand(manual)
            return FileManager.default.isExecutableFile(atPath: p) ? p : nil
        }
        lock.lock(); defer { lock.unlock() }
        if let cached = cache[name] { return cached }
        for dir in candidateDirs {
            let p = dir + "/" + name
            if FileManager.default.isExecutableFile(atPath: p) {
                cache[name] = p
                return p
            }
        }
        if let p = shellWhich(name) {
            cache[name] = p
            return p
        }
        return nil
    }

    static func resetCache() {
        lock.lock(); cache.removeAll(); lock.unlock()
    }

    private static func shellWhich(_ name: String) -> String? {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/bin/zsh")
        p.arguments = ["-lc", "command -v \(name)"]
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = FileHandle.nullDevice
        do { try p.run() } catch { return nil }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        let s = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return s.hasPrefix("/") ? s : nil
    }

    /// Environment for child processes with an augmented PATH.
    static func environment() -> [String: String] {
        var env = ProcessInfo.processInfo.environment
        let existing = env["PATH"] ?? "/usr/bin:/bin"
        env["PATH"] = (candidateDirs + [existing]).joined(separator: ":")
        env["NO_COLOR"] = "1"
        env["TERM"] = "dumb"
        return env
    }
}
