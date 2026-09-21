import Foundation

/// Runs a child process, streams stdout line-by-line, and reports exit.
final class ProcessJob {
    private let process = Process()
    private let stdoutPipe = Pipe()
    private let stderrPipe = Pipe()
    private let stdinPipe = Pipe()
    private var buffer = Data()
    private let input: String?
    private let onLine: (String) -> Void
    private let onExit: (Int32, String) -> Void
    private let stateLock = NSLock()
    private var cancelled = false

    init(executable: String,
         arguments: [String],
         environment: [String: String],
         currentDirectory: URL? = nil,
         input: String? = nil,
         onLine: @escaping (String) -> Void,
         onExit: @escaping (Int32, String) -> Void) {
        process.executableURL = URL(fileURLWithPath: executable)
        process.arguments = arguments
        process.environment = environment
        if let cwd = currentDirectory { process.currentDirectoryURL = cwd }
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe
        process.standardInput = stdinPipe
        self.input = input
        self.onLine = onLine
        self.onExit = onExit
    }

    func start() throws {
        try process.run()

        // Feed stdin on a background thread.
        let writer = stdinPipe.fileHandleForWriting
        let inputData = input.map { Data($0.utf8) }
        DispatchQueue.global(qos: .userInitiated).async {
            if let data = inputData { try? writer.write(contentsOf: data) }
            try? writer.close()
        }

        // Drain stderr concurrently so the child never blocks on a full pipe.
        let group = DispatchGroup()
        var stderrData = Data()
        group.enter()
        DispatchQueue.global(qos: .utility).async {
            stderrData = self.stderrPipe.fileHandleForReading.readDataToEndOfFile()
            group.leave()
        }

        DispatchQueue.global(qos: .userInitiated).async {
            let handle = self.stdoutPipe.fileHandleForReading
            while true {
                let chunk = handle.availableData
                if chunk.isEmpty { break }
                self.consume(chunk)
            }
            if !self.buffer.isEmpty, let last = String(data: self.buffer, encoding: .utf8) {
                self.buffer.removeAll()
                if !self.isCancelled { self.onLine(last) }
            }
            self.process.waitUntilExit()
            group.wait()
            Log.write("process exited status=\(self.process.terminationStatus) cancelled=\(self.isCancelled)")
            if !self.isCancelled {
                let err = String(data: stderrData, encoding: .utf8) ?? ""
                self.onExit(self.process.terminationStatus, err)
            }
        }
    }

    private var isCancelled: Bool {
        stateLock.lock(); defer { stateLock.unlock() }
        return cancelled
    }

    func cancel() {
        stateLock.lock(); cancelled = true; stateLock.unlock()
        if process.isRunning { process.terminate() }
    }

    private func consume(_ chunk: Data) {
        buffer.append(chunk)
        while let idx = buffer.firstIndex(of: 0x0A) {
            let lineData = buffer.subdata(in: buffer.startIndex..<idx)
            buffer.removeSubrange(buffer.startIndex...idx)
            if isCancelled { continue }
            if let line = String(data: lineData, encoding: .utf8), !line.isEmpty {
                onLine(line)
            }
        }
    }
}
