import AppKit
import CoreGraphics

/// Watches for the configured shortcut using a CGEvent tap.
/// Requires the Accessibility permission (System Settings → Privacy & Security → Accessibility).
final class HotkeyMonitor {
    private var tap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?
    private var lastPress: TimeInterval = 0
    private let config: () -> HotkeyConfig
    private let interval: () -> TimeInterval
    private let onTrigger: (HotkeyConfig) -> Void

    init(config: @escaping () -> HotkeyConfig, interval: @escaping () -> TimeInterval,
         onTrigger: @escaping (HotkeyConfig) -> Void) {
        self.config = config
        self.interval = interval
        self.onTrigger = onTrigger
    }

    var isRunning: Bool {
        guard let tap else { return false }
        return CGEvent.tapIsEnabled(tap: tap)
    }

    /// Returns false when the tap could not be created (usually: Accessibility not granted).
    @discardableResult
    func start() -> Bool {
        stop()
        let mask = CGEventMask(1 << CGEventType.keyDown.rawValue)
        let refcon = Unmanaged.passUnretained(self).toOpaque()
        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .defaultTap,   // active tap: needs Accessibility; lets us swallow single-press shortcuts
            eventsOfInterest: mask,
            callback: { _, type, event, refcon in
                guard let refcon else { return Unmanaged.passUnretained(event) }
                let monitor = Unmanaged<HotkeyMonitor>.fromOpaque(refcon).takeUnretainedValue()
                return monitor.handle(type: type, event: event) ? nil : Unmanaged.passUnretained(event)
            },
            userInfo: refcon
        ) else {
            Log.write("event tap creation failed (accessibility not granted?)")
            return false
        }
        self.tap = tap
        let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
        runLoopSource = source
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        Log.write("event tap installed (listenEventAccess=\(CGPreflightListenEventAccess()), secureInput=\(SecureInput.isEnabled), hotkey=\(config().display))")
        return true
    }

    func stop() {
        if let source = runLoopSource {
            CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .commonModes)
        }
        if let tap { CGEvent.tapEnable(tap: tap, enable: false) }
        runLoopSource = nil
        tap = nil
    }

    /// Returns true when the event should be swallowed.
    private func handle(type: CGEventType, event: CGEvent) -> Bool {
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            // macOS disables taps that respond too slowly; just turn it back on.
            if let tap { CGEvent.tapEnable(tap: tap, enable: true) }
            Log.write("event tap re-enabled after \(type == .tapDisabledByTimeout ? "timeout" : "user input")")
            return false
        }
        guard type == .keyDown,
              event.getIntegerValueField(.eventSourceUserData) != Hotkey.syntheticMarker,
              event.getIntegerValueField(.keyboardEventAutorepeat) == 0
        else { return false }

        let cfg = config()
        guard event.getIntegerValueField(.keyboardEventKeycode) == Int64(cfg.keyCode),
              event.flags.intersection(HotkeyConfig.modifierMask) == cfg.flags
        else { return false }

        if !cfg.doublePress {
            Log.write("hotkey \(cfg.display) pressed")
            DispatchQueue.main.async { self.onTrigger(cfg) }
            return true   // a dedicated shortcut: don't let the app underneath see it
        }

        let now = Date().timeIntervalSinceReferenceDate
        let delta = now - lastPress
        if delta <= interval() {
            lastPress = 0
            Log.write("hotkey \(cfg.display) detected (Δ \(String(format: "%.2f", delta))s)")
            DispatchQueue.main.async { self.onTrigger(cfg) }
        } else {
            lastPress = now
        }
        return false      // ⌘C must still reach the app so the copy happens
    }
}
