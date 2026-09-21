import AppKit
import Carbon.HIToolbox

/// "Secure Keyboard Entry" (terminals, password fields) blocks every global key monitor system-wide.
enum SecureInput {
    static var isEnabled: Bool { IsSecureEventInputEnabled() }

    /// Name of the app currently holding secure input, if it can be determined.
    static func holderName() -> String? {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/sbin/ioreg")
        p.arguments = ["-l", "-w", "0"]
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = FileHandle.nullDevice
        do { try p.run() } catch { return nil }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        guard let out = String(data: data, encoding: .utf8),
              let range = out.range(of: "\"kCGSSessionSecureInputPID\"=") else { return nil }
        let digits = out[range.upperBound...].prefix { $0.isNumber }
        guard let pid = Int32(digits), pid > 0 else { return nil }
        return NSRunningApplication(processIdentifier: pid)?.localizedName ?? "pid \(pid)"
    }

    static func warningText() -> String? {
        guard isEnabled else { return nil }
        let holder = holderName() ?? "다른 앱"
        return "\(holder)이(가) 보안 키보드 입력을 켜서 단축키가 동작하지 않습니다"
    }
}
