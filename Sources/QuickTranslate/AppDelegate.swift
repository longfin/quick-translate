import AppKit
import SwiftUI
import ApplicationServices

final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    private var statusItem: NSStatusItem!
    private let settings = AppSettings.shared
    private lazy var panelController = TranslationPanelController(settings: settings)
    private var hotkey: DoubleCopyMonitor?
    private var settingsWindow: NSWindow?
    private var accessibilityTimer: Timer?
    private let secureInputWarningItem = NSMenuItem(title: "", action: nil, keyEquivalent: "")
    private var accessibilityMenuItem: NSMenuItem?

    func applicationDidFinishLaunching(_ notification: Notification) {
        Log.write("launched v\(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "?")")
        Log.write("ui localization: \(Bundle.main.preferredLocalizations) e.g. \(L("Settings…"))")
        setupStatusItem()

        hotkey = DoubleCopyMonitor(interval: { [settings] in settings.doublePressInterval }) { [weak self] in
            self?.translateClipboard(afterDelay: 0.15)
        }
        ensureAccessibility()

        // External trigger (e.g. Raycast / Hammerspoon / scripts):
        //   osascript -l JavaScript -e 'ObjC.import("Foundation"); $.NSDistributedNotificationCenter.defaultCenter.postNotificationNameObject("dev.swen.QuickTranslate.translate", null)'
        DistributedNotificationCenter.default().addObserver(
            self,
            selector: #selector(translateClipboardAction),
            name: Notification.Name("dev.swen.QuickTranslate.translate"),
            object: nil
        )
    }

    // MARK: - Status bar

    private func setupStatusItem() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        if let button = statusItem.button {
            button.image = NSImage(systemSymbolName: "character.bubble", accessibilityDescription: "QuickTranslate")
                ?? NSImage(systemSymbolName: "globe", accessibilityDescription: "QuickTranslate")
        }

        let menu = NSMenu()
        menu.delegate = self
        secureInputWarningItem.isHidden = true
        menu.addItem(secureInputWarningItem)
        let translateItem = NSMenuItem(title: L("Translate Clipboard (⌘C ⌘C)"), action: #selector(translateClipboardAction), keyEquivalent: "")
        translateItem.target = self
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

    func menuNeedsUpdate(_ menu: NSMenu) {
        accessibilityMenuItem?.isHidden = AXIsProcessTrusted()
        if let warning = SecureInput.warningText() {
            secureInputWarningItem.title = "⚠️ " + warning
            secureInputWarningItem.isHidden = false
        } else {
            secureInputWarningItem.isHidden = true
        }
    }

    // MARK: - Accessibility

    private func ensureAccessibility() {
        let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
        if AXIsProcessTrustedWithOptions(options), hotkey?.start() == true {
            Log.write("accessibility granted, hotkey monitor started")
            return
        }
        Log.write("accessibility NOT granted yet; polling")
        // Not trusted yet: the system prompt is showing. Poll until granted, then install the tap.
        accessibilityTimer = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in
            guard let self, AXIsProcessTrusted(), self.hotkey?.start() == true else { return }
            self.accessibilityTimer?.invalidate()
            self.accessibilityTimer = nil
            Log.write("accessibility granted (late); relaunching so the event tap receives events")
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
