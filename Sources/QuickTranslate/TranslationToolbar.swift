import AppKit
import Combine

/// The panel's toolbar: an "English → Korean" pop-up in the center and a pin toggle on the right.
/// Built in AppKit because SwiftUI's toolbar bridging drops the pop-up's title inside the toolbar.
final class TranslationToolbar: NSObject, NSToolbarDelegate {
    private static let languageID = NSToolbarItem.Identifier("language")
    private static let pinID = NSToolbarItem.Identifier("pin")

    private let vm: TranslationViewModel
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
    private func updateLanguages(source: String?, target: String) {
        let from = source.map(L) ?? L("Auto-detect")
        popUp.removeAllItems()
        for language in AppSettings.languages {
            popUp.addItem(withTitle: "\(from) → \(L(language))")
            popUp.lastItem?.representedObject = language
        }
        popUp.selectItem(at: AppSettings.languages.firstIndex(of: target) ?? 0)
        popUp.sizeToFit()
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
            item.view = popUp
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
