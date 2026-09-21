import AppKit

/// Detects ⌘C ⌘C without looking at the keyboard: every copy bumps NSPasteboard.changeCount, so two
/// bumps within the interval mean the user copied twice. Works while Secure Keyboard Entry is on and
/// needs no Accessibility permission.
final class PasteboardWatcher {
    private var timer: DispatchSourceTimer?
    private var lastCount = NSPasteboard.general.changeCount
    private var lastChange: TimeInterval = 0
    private var ignoreUntil: TimeInterval = 0
    private let interval: () -> TimeInterval
    private let onTrigger: () -> Void

    init(interval: @escaping () -> TimeInterval, onTrigger: @escaping () -> Void) {
        self.interval = interval
        self.onTrigger = onTrigger
    }

    var isRunning: Bool { timer != nil }

    func start() {
        guard timer == nil else { return }
        lastCount = NSPasteboard.general.changeCount
        lastChange = 0
        let t = DispatchSource.makeTimerSource(queue: .main)
        t.schedule(deadline: .now(), repeating: .milliseconds(60), leeway: .milliseconds(20))
        t.setEventHandler { [weak self] in self?.poll() }
        t.resume()
        timer = t
        Log.write("pasteboard watcher started")
    }

    func stop() {
        timer?.cancel()
        timer = nil
    }

    /// Call after the app writes to the pasteboard itself so that write is not mistaken for a copy.
    func ignoreOwnChange() {
        ignoreUntil = Date().timeIntervalSinceReferenceDate + 1.0
        lastCount = NSPasteboard.general.changeCount
    }

    private func poll() {
        let count = NSPasteboard.general.changeCount
        guard count != lastCount else { return }
        lastCount = count
        let now = Date().timeIntervalSinceReferenceDate
        guard now >= ignoreUntil else { return }
        let delta = now - lastChange
        if delta <= interval() {
            lastChange = 0
            Log.write("pasteboard changed twice (Δ \(String(format: "%.2f", delta))s) → translate")
            onTrigger()
        } else {
            lastChange = now
        }
    }
}
