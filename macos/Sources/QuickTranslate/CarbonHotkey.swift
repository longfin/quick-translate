import Carbon.HIToolbox
import Foundation

/// A system-wide hot key via RegisterEventHotKey (the API Raycast/Alfred/Hammerspoon use). The key is
/// consumed before reaching other apps, and no Accessibility permission is needed to receive it.
final class CarbonHotkey {
    private var ref: EventHotKeyRef?
    private var handler: EventHandlerRef?
    private let onTrigger: () -> Void
    private static let signature: OSType = 0x51545248   // "QTRH"

    init(onTrigger: @escaping () -> Void) {
        self.onTrigger = onTrigger
    }

    var isRegistered: Bool { ref != nil }

    func register(_ config: HotkeyConfig) {
        unregister()
        var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        let refcon = Unmanaged.passUnretained(self).toOpaque()
        InstallEventHandler(GetApplicationEventTarget(), { _, _, refcon in
            guard let refcon else { return noErr }
            let me = Unmanaged<CarbonHotkey>.fromOpaque(refcon).takeUnretainedValue()
            Log.write("carbon hotkey pressed (secureInput=\(SecureInput.isEnabled))")
            DispatchQueue.main.async { me.onTrigger() }
            return noErr
        }, 1, &spec, refcon, &handler)

        var mods: UInt32 = 0
        if config.flags.contains(.maskCommand) { mods |= UInt32(cmdKey) }
        if config.flags.contains(.maskShift) { mods |= UInt32(shiftKey) }
        if config.flags.contains(.maskAlternate) { mods |= UInt32(optionKey) }
        if config.flags.contains(.maskControl) { mods |= UInt32(controlKey) }
        let id = EventHotKeyID(signature: Self.signature, id: 1)
        let status = RegisterEventHotKey(UInt32(config.keyCode), mods, id, GetApplicationEventTarget(), 0, &ref)
        Log.write("carbon hotkey \(config.display) registered status=\(status)")
        if status != noErr { ref = nil }
    }

    func unregister() {
        if let ref { UnregisterEventHotKey(ref) }
        ref = nil
        if let handler { RemoveEventHandler(handler) }
        handler = nil
    }
}
