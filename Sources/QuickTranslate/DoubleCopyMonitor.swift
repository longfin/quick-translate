import AppKit
import CoreGraphics

/// Detects ⌘C pressed twice in quick succession (⌘C ⌘C, or ⌘ held + C C) using a CGEvent tap.
/// Requires the Accessibility permission (System Settings → Privacy & Security → Accessibility).
final class DoubleCopyMonitor {
    private var tap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?
    private var lastPress: TimeInterval = 0
    private let interval: () -> TimeInterval
    private let onTrigger: () -> Void

    private static let keyCodeC: Int64 = 8 // kVK_ANSI_C (physical key, layout independent)

    init(interval: @escaping () -> TimeInterval, onTrigger: @escaping () -> Void) {
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
            options: .defaultTap,   // active tap: needs Accessibility; we pass every event through untouched
            eventsOfInterest: mask,
            callback: { _, type, event, refcon in
                guard let refcon else { return Unmanaged.passUnretained(event) }
                let monitor = Unmanaged<DoubleCopyMonitor>.fromOpaque(refcon).takeUnretainedValue()
                monitor.handle(type: type, event: event)
                return Unmanaged.passUnretained(event)
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
        Log.write("event tap installed (listenEventAccess=\(CGPreflightListenEventAccess()), secureInput=\(SecureInput.isEnabled))")
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

    private func handle(type: CGEventType, event: CGEvent) {
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            // macOS disables taps that respond too slowly; just turn it back on.
            if let tap { CGEvent.tapEnable(tap: tap, enable: true) }
            Log.write("event tap re-enabled after \(type == .tapDisabledByTimeout ? "timeout" : "user input")")
            return
        }
        guard type == .keyDown else { return }
        let keycode = event.getIntegerValueField(.keyboardEventKeycode)
        let flags = event.flags
        guard keycode == Self.keyCodeC,
              event.getIntegerValueField(.keyboardEventAutorepeat) == 0,
              flags.contains(.maskCommand),
              flags.intersection([.maskShift, .maskControl, .maskAlternate]).isEmpty
        else { return }
        let now = Date().timeIntervalSinceReferenceDate
        let delta = now - lastPress
        if delta <= interval() {
            lastPress = 0
            Log.write("⌘C ⌘C detected (Δ \(String(format: "%.2f", delta))s)")
            DispatchQueue.main.async { self.onTrigger() }
        } else {
            lastPress = now
        }
    }
}
