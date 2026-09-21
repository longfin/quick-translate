import AppKit
import SwiftUI

final class TranslationPanel: NSPanel {
    var onEscape: (() -> Void)?

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    override func cancelOperation(_ sender: Any?) {
        onEscape?()
    }
}

/// Owns the floating translation panel.
final class TranslationPanelController {
    private let panel: TranslationPanel
    private let settings: AppSettings
    let viewModel: TranslationViewModel
    private var outsideClickMonitor: Any?

    private static let defaultSize = NSSize(width: 480, height: 340)

    init(settings: AppSettings) {
        self.settings = settings
        self.viewModel = TranslationViewModel(settings: settings)

        panel = TranslationPanel(
            contentRect: NSRect(origin: .zero, size: Self.defaultSize),
            styleMask: [.titled, .fullSizeContentView, .nonactivatingPanel, .resizable, .utilityWindow],
            backing: .buffered,
            defer: false
        )
        panel.titleVisibility = .hidden
        panel.titlebarAppearsTransparent = true
        panel.standardWindowButton(.closeButton)?.isHidden = true
        panel.standardWindowButton(.miniaturizeButton)?.isHidden = true
        panel.standardWindowButton(.zoomButton)?.isHidden = true
        panel.level = .floating
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.isMovableByWindowBackground = true
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.minSize = NSSize(width: 380, height: 240)
        panel.setFrameAutosaveName("QuickTranslatePanel")

        let root = TranslationView(vm: viewModel, settings: settings) { [weak self] in self?.hide() }
        panel.contentView = NSHostingView(rootView: root)
        panel.onEscape = { [weak self] in self?.hide() }
    }

    var isVisible: Bool { panel.isVisible }

    func show(text: String) {
        if !panel.isVisible {
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
