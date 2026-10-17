import AppKit
import SwiftUI

final class TranslationPanel: NSPanel {
    var onClose: (() -> Void)?

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    override func cancelOperation(_ sender: Any?) {
        onClose?()
    }

    // The close button (and ⌘W below) go through the controller so a running translation is cancelled.
    override func performClose(_ sender: Any?) {
        onClose?()
    }

    // LSUIElement app: there is no Window menu to supply ⌘W / ⌘M.
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        let mods = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        if mods == .command {
            switch event.charactersIgnoringModifiers {
            case "w": performClose(nil); return true
            case "m": miniaturize(nil); return true
            default: break
            }
        }
        return super.performKeyEquivalent(with: event)
    }
}

/// Owns the floating translation panel.
final class TranslationPanelController {
    private let panel: TranslationPanel
    private let settings: AppSettings
    let viewModel: TranslationViewModel
    private let toolbar: TranslationToolbar
    private var outsideClickMonitor: Any?

    private static let defaultSize = NSSize(width: 480, height: 340)

    init(settings: AppSettings) {
        self.settings = settings
        self.viewModel = TranslationViewModel(settings: settings)
        self.toolbar = TranslationToolbar(vm: viewModel)

        panel = TranslationPanel(
            contentRect: NSRect(origin: .zero, size: Self.defaultSize),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.titleVisibility = .hidden
        panel.titlebarAppearsTransparent = true
        panel.level = .floating
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.isMovableByWindowBackground = true
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.minSize = NSSize(width: 380, height: 240)
        panel.setFrameAutosaveName("QuickTranslatePanel")

        let host = NSHostingView(rootView: TranslationView(vm: viewModel, settings: settings))
        panel.toolbar = toolbar.toolbar
        panel.toolbarStyle = .unified
        panel.contentView = host
        panel.onClose = { [weak self] in self?.hide() }
    }

    var isVisible: Bool { panel.isVisible }

    /// Pays for SwiftUI's first layout/render of the content and the toolbar up front, so the
    /// first real `show` is as fast as later ones (#6). The panel is ordered in fully transparent
    /// and out again in the same run-loop turn: nothing is composited and nothing takes focus.
    func warmUp() {
        guard !panel.isVisible else { return }
        let start = Date()
        panel.alphaValue = 0
        panel.ignoresMouseEvents = true
        panel.orderFrontRegardless()
        panel.contentView?.layoutSubtreeIfNeeded()
        panel.displayIfNeeded()
        panel.orderOut(nil)
        panel.ignoresMouseEvents = false
        panel.alphaValue = 1
        Log.write(String(format: "panel warmed up in %.0f ms", Date().timeIntervalSince(start) * 1000))
    }

    func show(text: String) {
        if panel.isMiniaturized {
            panel.deminiaturize(nil)
        } else if !panel.isVisible {
            positionNearMouse()
        }
        panel.makeKeyAndOrderFront(nil)
        installOutsideClickMonitor()
        viewModel.translate(text: text)
    }

    func hide() {
        removeOutsideClickMonitor()
        viewModel.cancel()
        panel.orderOut(nil)
    }

    private func positionNearMouse() {
        let mouse = NSEvent.mouseLocation
        let screen = NSScreen.screens.first { NSMouseInRect(mouse, $0.frame, false) } ?? NSScreen.main
        let vf = screen?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        let size = panel.frame.size.width > 0 ? panel.frame.size : Self.defaultSize

        var x = mouse.x + 12
        var y = mouse.y - 12 - size.height
        x = min(x, vf.maxX - size.width)
        x = max(x, vf.minX)
        y = max(y, vf.minY)
        y = min(y, vf.maxY - size.height)
        panel.setFrame(NSRect(x: x, y: y, width: size.width, height: size.height), display: true)
    }

    private func installOutsideClickMonitor() {
        guard outsideClickMonitor == nil else { return }
        outsideClickMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
            guard let self, self.settings.closeOnOutsideClick, !self.viewModel.pinned,
                  !self.panel.isMiniaturized,
                  !self.viewModel.isLoading   // keep the panel up until the translation is done
            else { return }
            Log.write("panel hidden (outside click)")
            self.hide()
        }
    }

    private func removeOutsideClickMonitor() {
        if let m = outsideClickMonitor { NSEvent.removeMonitor(m) }
        outsideClickMonitor = nil
    }
}
