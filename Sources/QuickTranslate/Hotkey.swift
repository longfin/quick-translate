import AppKit
import Carbon.HIToolbox

/// A global keyboard shortcut: physical key + modifiers, pressed once or twice in quick succession.
struct HotkeyConfig: Equatable {
    var keyCode: UInt16 = UInt16(kVK_ANSI_C)
    var modifiers: UInt64 = CGEventFlags.maskCommand.rawValue   // only ⌘⇧⌥⌃ bits are stored
    var doublePress: Bool = true

    static let `default` = HotkeyConfig()
    static let modifierMask: CGEventFlags = [.maskCommand, .maskShift, .maskAlternate, .maskControl]

    var flags: CGEventFlags { CGEventFlags(rawValue: modifiers) }

    /// "⌘C ⌘C" or "⌃⌥T"
    var display: String {
        let combo = modifierSymbols + Hotkey.keyLabel(for: keyCode)
        return doublePress ? "\(combo) \(combo)" : combo
    }

    var modifierSymbols: String {
        var s = ""
        if flags.contains(.maskControl) { s += "⌃" }
        if flags.contains(.maskAlternate) { s += "⌥" }
        if flags.contains(.maskShift) { s += "⇧" }
        if flags.contains(.maskCommand) { s += "⌘" }
        return s
    }

    /// Something like ⇧C alone would fire while typing; require ⌘, ⌥ or ⌃ unless it is a function key.
    var isUsable: Bool {
        if Hotkey.functionKeys[keyCode] != nil { return true }
        return !flags.intersection([.maskCommand, .maskAlternate, .maskControl]).isEmpty
    }
}

enum Hotkey {
    /// ANSI key codes are layout independent, so labels use the US layout (what ⌘-shortcuts show in menus).
    private static let ansiLabels: [UInt16: String] = [
        0: "A", 1: "S", 2: "D", 3: "F", 4: "H", 5: "G", 6: "Z", 7: "X", 8: "C", 9: "V", 11: "B", 12: "Q", 13: "W",
        14: "E", 15: "R", 16: "Y", 17: "T", 18: "1", 19: "2", 20: "3", 21: "4", 22: "6", 23: "5", 24: "=", 25: "9",
        26: "7", 27: "-", 28: "8", 29: "0", 30: "]", 31: "O", 32: "U", 33: "[", 34: "I", 35: "P", 37: "L", 38: "J",
        39: "'", 40: "K", 41: ";", 42: "\\", 43: ",", 44: "/", 45: "N", 46: "M", 47: ".", 50: "`",
    ]
    static let functionKeys: [UInt16: String] = [
        122: "F1", 120: "F2", 99: "F3", 118: "F4", 96: "F5", 97: "F6", 98: "F7", 100: "F8", 101: "F9", 109: "F10",
        103: "F11", 111: "F12", 105: "F13", 107: "F14", 113: "F15",
    ]
    private static let specialKeys: [UInt16: String] = [
        36: "↩", 48: "⇥", 49: "Space", 51: "⌫", 53: "⎋", 76: "⌤", 117: "⌦", 115: "↖", 119: "↘", 116: "⇞", 121: "⇟",
        123: "←", 124: "→", 125: "↓", 126: "↑",
    ]

    static func keyLabel(for keyCode: UInt16) -> String {
        ansiLabels[keyCode] ?? functionKeys[keyCode] ?? specialKeys[keyCode] ?? "Key \(keyCode)"
    }

    /// Modifier-only key codes, which cannot be a shortcut on their own.
    static func isModifierKey(_ keyCode: UInt16) -> Bool {
        [54, 55, 56, 57, 58, 59, 60, 61, 62, 63].contains(keyCode)
    }

    static func config(from event: NSEvent) -> HotkeyConfig? {
        guard !isModifierKey(event.keyCode) else { return nil }
        let cg = CGEventFlags(rawValue: UInt64(event.modifierFlags.rawValue)).intersection(HotkeyConfig.modifierMask)
        return HotkeyConfig(keyCode: event.keyCode, modifiers: cg.rawValue, doublePress: true)
    }

    /// Marker on events we post ourselves so the tap ignores them.
    static let syntheticMarker: Int64 = 0x5154   // "QT"

    /// Sends ⌘C to the frontmost app so the current selection lands on the pasteboard.
    static func postCopyKeystroke() {
        guard let source = CGEventSource(stateID: .combinedSessionState) else { return }
        for down in [true, false] {
            guard let e = CGEvent(keyboardEventSource: source, virtualKey: CGKeyCode(kVK_ANSI_C), keyDown: down) else { continue }
            e.flags = .maskCommand
            e.setIntegerValueField(.eventSourceUserData, value: syntheticMarker)
            e.post(tap: .cghidEventTap)
        }
    }
}
