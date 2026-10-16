import AppKit
import Combine

/// The panel's toolbar: an "English → Korean" pop-up in the center and a pin toggle on the right.
/// Built in AppKit because SwiftUI's toolbar bridging drops the pop-up's title inside the toolbar.
final class TranslationToolbar: NSObject, NSToolbarDelegate {
    private static let languageID = NSToolbarItem.Identifier("language")
    private static let pinID = NSToolbarItem.Identifier("pin")

    private let vm: TranslationViewModel
    private let sourceLabel = NSTextField(labelWithString: "")
    private let popUp = NSPopUpButton(frame: .zero, pullsDown: false)
    private var pinItem: NSToolbarItem?
    private var observers: Set<AnyCancellable> = []

    let toolbar = NSToolbar(identifier: "TranslationToolbar")

    init(vm: TranslationViewModel) {
        self.vm = vm
        super.init()
        popUp.target = self
        popUp.action = #selector(languageChosen)
        popUp.toolTip = L("Target language")
        popUp.autoenablesItems = false
        for language in AppSettings.languages {
            popUp.addItem(withTitle: L(language))
            popUp.lastItem?.representedObject = language
        }
        sourceLabel.textColor = .secondaryLabelColor
        toolbar.delegate = self
        toolbar.displayMode = .iconOnly
        toolbar.allowsUserCustomization = false
        toolbar.centeredItemIdentifiers = [Self.languageID]

        vm.$detectedLanguage.combineLatest(vm.$targetLanguage)
            .sink { [weak self] source, target in self?.updateLanguages(source: source, target: target) }
            .store(in: &observers)
        vm.$pinned
            .sink { [weak self] pinned in self?.updatePin(pinned) }
            .store(in: &observers)
    }

    /// Every item reads "source → target", so the closed pop-up shows the whole direction.
    /// The source language is always auto-detected, so it is a plain label; only the target is a menu.
    /// Translating into the source language is meaningless, so that entry is disabled.
    private func updateLanguages(source: String?, target: String) {
        sourceLabel.stringValue = source.map(L) ?? L("Auto-detect")
        for item in popUp.itemArray {
            item.isEnabled = item.representedObject as? String != source
        }
        popUp.selectItem(at: AppSettings.languages.firstIndex(of: target) ?? 0)
    }

    private func updatePin(_ pinned: Bool) {
        let label = pinned ? L("Unpin") : L("Pin")
        pinItem?.image = NSImage(systemSymbolName: pinned ? "pin.fill" : "pin", accessibilityDescription: label)
        pinItem?.label = label
        pinItem?.toolTip = pinned ? L("Unpin") : L("Pin window (stays open when clicking outside)")
    }

    @objc private func languageChosen() {
        if let language = popUp.selectedItem?.representedObject as? String {
            vm.setTarget(language)
        }
    }

    @objc private func togglePin() {
        vm.pinned.toggle()
    }

    // MARK: NSToolbarDelegate

    func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        [Self.languageID, .flexibleSpace, Self.pinID]
    }

    func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        toolbarDefaultItemIdentifiers(toolbar)
    }

    func toolbar(_ toolbar: NSToolbar, itemForItemIdentifier id: NSToolbarItem.Identifier,
                 willBeInsertedIntoToolbar flag: Bool) -> NSToolbarItem? {
        let item = NSToolbarItem(itemIdentifier: id)
        switch id {
        case Self.languageID:
            item.label = L("Target language")
            let arrow = NSImageView(image: NSImage(systemSymbolName: "arrow.right", accessibilityDescription: nil)!)
            arrow.contentTintColor = .secondaryLabelColor
            let stack = NSStackView(views: [sourceLabel, arrow, popUp])
            stack.spacing = 6
            // The toolbar draws one capsule around the whole item; inset the label so it doesn't touch the edge.
            stack.edgeInsets = NSEdgeInsets(top: 0, left: 12, bottom: 0, right: 0)
            item.view = stack
        case Self.pinID:
            item.target = self
            item.action = #selector(togglePin)
            item.isBordered = true
            pinItem = item
            updatePin(vm.pinned)
        default:
            return nil
        }
        return item
    }
}
