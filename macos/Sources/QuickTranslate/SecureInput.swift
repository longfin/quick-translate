import AppKit
import Carbon.HIToolbox
import IOKit

/// "Secure Keyboard Entry" (terminals, password fields) blocks every global key monitor system-wide.
enum SecureInput {
    static var isEnabled: Bool { IsSecureEventInputEnabled() }

    /// PID of the process holding secure input, from the IORegistry root's IOConsoleUsers property.
    static func holderPID() -> pid_t? {
        let root = IORegistryGetRootEntry(kIOMainPortDefault)
        guard root != 0 else { return nil }
        defer { IOObjectRelease(root) }
        guard let users = IORegistryEntryCreateCFProperty(root, "IOConsoleUsers" as CFString, kCFAllocatorDefault, 0)?
                .takeRetainedValue() as? [[String: Any]] else { return nil }
        for user in users {
            if let pid = (user["kCGSSessionSecureInputPID"] as? NSNumber)?.int32Value, pid > 0 {
                return pid
            }
        }
        return nil
    }

    /// Name of the app currently holding secure input, if it can be determined.
    static func holderName() -> String? {
        guard let pid = holderPID() else { return nil }
        if let app = NSRunningApplication(processIdentifier: pid) {
            return app.localizedName ?? app.bundleIdentifier
        }
        // Not a GUI app (or already gone): fall back to the process name.
        var info = proc_bsdinfo()
        let size = Int32(MemoryLayout<proc_bsdinfo>.size)
        if proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, size) == size {
            let name = withUnsafePointer(to: &info.pbi_name) {
                $0.withMemoryRebound(to: CChar.self, capacity: Int(MAXCOMLEN) * 2) { String(cString: $0) }
            }
            if !name.isEmpty { return name }
        }
        return "pid \(pid)"
    }

    static func warningText() -> String? {
        guard isEnabled else { return nil }
        let holder = holderName() ?? L("Another app")
        return L("%@ has Secure Keyboard Entry enabled, so the hotkey cannot work.", holder)
    }
}
