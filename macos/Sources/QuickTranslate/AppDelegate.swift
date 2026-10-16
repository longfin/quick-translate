import AppKit
import SwiftUI
import ApplicationServices
import Combine

final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    private var statusItem: NSStatusItem!
    private let settings = AppSettings.shared
    private lazy var panelController = TranslationPanelController(settings: settings)
    private var hotkey: HotkeyMonitor?
    private var pasteboardWatcher: PasteboardWatcher?
    private var carbonHotkey: CarbonHotkey?
    private var cancellables = Set<AnyCancellable>()
    private var translateMenuItem: NSMenuItem?
    private var settingsWindow: NSWindow?
    private var accessibilityTimer: Timer?
    private let secureInputWarningItem = NSMenuItem(title: "", action: nil, keyEquivalent: "")
    private var accessibilityMenuItem: NSMenuItem?

    func applicationDidFinishLaunching(_ notification: Notification) {
        Log.write("launched v\(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "?")")
        Log.write("ui localization: \(Bundle.main.preferredLocalizations) e.g. \(L("Settings…"))")
        setupStatusItem()

        hotkey = HotkeyMonitor(config: { [settings] in settings.hotkey },
                               interval: { [settings] in settings.doublePressInterval }) { [weak self] _ in
            self?.translateClipboard(afterDelay: 0.15)   // the app's own copy is still landing on the pasteboard
        }
        pasteboardWatcher = PasteboardWatcher(interval: { [settings] in settings.doublePressInterval }) { [weak self] in
            self?.translateClipboard(afterDelay: 0)
        }
        carbonHotkey = CarbonHotkey { [weak self] in self?.copySelectionAndTranslate() }
        NotificationCenter.default.addObserver(forName: .quickTranslateWrotePasteboard, object: nil, queue: .main) { [weak self] _ in
            self?.pasteboardWatcher?.ignoreOwnChange()
        }
        configureHotkey()
        settings.$hotkey.dropFirst().removeDuplicates().sink { [weak self] _ in
            DispatchQueue.main.async { self?.configureHotkey() }
        }.store(in: &cancellables)
        TranslationEngine.shared.prewarm(settings: settings)

        // External trigger (e.g. Raycast / Hammerspoon / scripts):
        //   osascript -l JavaScript -e 'ObjC.import("Foundation"); $.NSDistributedNotificationCenter.defaultCenter.postNotificationNameObject("dev.swen.QuickTranslate.translate", null)'
        DistributedNotificationCenter.default().addObserver(
            self,
            selector: #selector(translateClipboardAction),
            name: Notification.Name("dev.swen.QuickTranslate.translate"),
            object: nil
        )
    }

    func applicationWillTerminate(_ notification: Notification) {
        ClaudeWorker.shared.shutdown()
    }

    // MARK: - Status bar

    private func setupStatusItem() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        if let button = statusItem.button {
            button.image = Self.menuBarIcon()
                ?? NSImage(systemSymbolName: "character.bubble", accessibilityDescription: "QuickTranslate")
        }

        let menu = NSMenu()
        menu.delegate = self
        secureInputWarningItem.isHidden = true
        menu.addItem(secureInputWarningItem)
        let translateItem = NSMenuItem(title: L("Translate Clipboard (%@)", settings.hotkey.display), action: #selector(translateClipboardAction), keyEquivalent: "")
        translateItem.target = self
        translateMenuItem = translateItem
        menu.addItem(translateItem)
        menu.addItem(.separator())

        let settingsItem = NSMenuItem(title: L("Settings…"), action: #selector(openSettings), keyEquivalent: ",")
        settingsItem.target = self
        menu.addItem(settingsItem)

        let axItem = NSMenuItem(title: L("Accessibility Permission…"), action: #selector(openAccessibility), keyEquivalent: "")
        axItem.target = self
        axItem.isHidden = AXIsProcessTrusted()   // only useful until the permission is granted
        accessibilityMenuItem = axItem
        menu.addItem(axItem)
        menu.addItem(.separator())

        menu.addItem(NSMenuItem(title: L("Quit QuickTranslate"), action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q"))
        statusItem.menu = menu
    }

    /// Template image (black + alpha) so macOS tints it for light/dark menu bars.
    private static func menuBarIcon() -> NSImage? {
        guard let url = Bundle.main.url(forResource: "MenuBarIcon", withExtension: "png"),
              let image = NSImage(contentsOf: url) else { return nil }
        if let url2x = Bundle.main.url(forResource: "MenuBarIcon@2x", withExtension: "png"),
           let rep = NSImageRep(contentsOf: url2x) {
            rep.size = NSSize(width: 18, height: 18)
            image.addRepresentation(rep)
        }
        image.size = NSSize(width: 18, height: 18)
        image.isTemplate = true
        image.accessibilityDescription = "QuickTranslate"
        return image
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        accessibilityMenuItem?.isHidden = AXIsProcessTrusted() || !settings.hotkey.needsAccessibility
        translateMenuItem?.title = L("Translate Clipboard (%@)", settings.hotkey.display)
        if settings.hotkey.dependsOnKeyEvents, let warning = SecureInput.warningText() {
            secureInputWarningItem.title = "⚠️ " + warning
            secureInputWarningItem.isHidden = false
        } else {
            secureInputWarningItem.isHidden = true
        }
    }

    // MARK: - Hotkey engines

    /// Picks the detection mechanism for the configured shortcut:
    /// - ⌘C ⌘C: pasteboard watcher (immune to Secure Keyboard Entry, no permission needed)
    /// - any other shortcut pressed twice: CGEvent tap (needs Accessibility)
    /// - pressed once: Carbon hot key, then a synthetic ⌘C to copy the selection (needs Accessibility)
    private func configureHotkey() {
        let cfg = settings.hotkey
        pasteboardWatcher?.stop()
        carbonHotkey?.unregister()
        hotkey?.stop()
        Log.write("hotkey configured: \(cfg.display) mode=\(cfg.isCopyDoublePress ? "pasteboard" : cfg.doublePress ? "event tap" : "carbon")")
        if cfg.isCopyDoublePress {
            pasteboardWatcher?.start()
        } else if cfg.doublePress {
            ensureAccessibility { [weak self] in self?.hotkey?.start() ?? false }
        } else {
            carbonHotkey?.register(cfg)
            ensureAccessibility { true }   // needed later, for posting ⌘C
        }
    }

    // MARK: - Accessibility

    /// Prompts for Accessibility if needed, then runs `onGranted` (which returns false to retry later).
    private func ensureAccessibility(onGranted: @escaping () -> Bool) {
        accessibilityTimer?.invalidate()
        accessibilityTimer = nil
        let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
        if AXIsProcessTrustedWithOptions(options), onGranted() {
            Log.write("accessibility granted")
            return
        }
        Log.write("accessibility NOT granted yet; polling")
        // Not trusted yet: the system prompt is showing. Poll until granted.
        accessibilityTimer = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in
            guard let self, AXIsProcessTrusted() else { return }
            self.accessibilityTimer?.invalidate()
            self.accessibilityTimer = nil
            Log.write("accessibility granted (late); relaunching so key events are delivered")
            self.relaunch()
        }
    }

    /// macOS only delivers global key events to processes that were trusted at launch, so relaunch.
    func relaunch() {
        let path = Bundle.main.bundleURL.path
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/bin/sh")
        p.arguments = ["-c", "sleep 0.7; /usr/bin/open \"\(path)\""]
        try? p.run()
        NSApp.terminate(nil)
    }

    // MARK: - Actions

    @objc private func translateClipboardAction() {
        Log.write("translate requested (menu/notification)")
        translateClipboard(afterDelay: 0)
    }

    private func translateClipboard(afterDelay delay: TimeInterval) {
        // Give the frontmost app a moment to finish writing the pasteboard after the second ⌘C.
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
            guard let self else { return }
            let text = NSPasteboard.general.string(forType: .string) ?? ""
            self.panelController.show(text: text)
        }
    }

    /// Single-press shortcuts: copy the selection ourselves, then translate whatever landed on the pasteboard.
    private func copySelectionAndTranslate() {
        let before = NSPasteboard.general.changeCount
        Hotkey.postCopyKeystroke()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { [weak self] in
            guard let self else { return }
            if NSPasteboard.general.changeCount == before {
                Log.write("copy keystroke did not change the pasteboard; translating existing clipboard")
            }
            let text = NSPasteboard.general.string(forType: .string) ?? ""
            self.panelController.show(text: text)
        }
    }

    @objc private func openSettings() {
        if settingsWindow == nil {
            let host = NSHostingController(rootView: SettingsView(settings: settings))
            let window = NSWindow(contentViewController: host)
            window.title = L("QuickTranslate Settings")
            window.styleMask = [.titled, .closable]
            window.isReleasedWhenClosed = false
            window.center()
            settingsWindow = window
        }
        NSApp.activate(ignoringOtherApps: true)
        settingsWindow?.makeKeyAndOrderFront(nil)
    }

    @objc private func openAccessibility() {
        openAccessibilitySettings()
    }
}
