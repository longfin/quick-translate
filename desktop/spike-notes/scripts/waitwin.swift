import CoreGraphics
import Foundation

// waitwin <owner> <status|panel> <timeoutSec>
// Polls the window server (~1 ms) and prints the epoch-ms when <owner> first has an on-screen
// status-bar item (layer 25) or a floating panel (layer 3–20, height ≥ 100). Needs no permissions
// beyond reading window metadata, so it measures both apps the same way.
let args = CommandLine.arguments
let owner = args[1], mode = args[2]
let deadline = Date().addingTimeInterval(Double(args.count > 3 ? args[3] : "10") ?? 10)
while Date() < deadline {
    let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
    for w in list where (w[kCGWindowOwnerName as String] as? String) == owner {
        let layer = w[kCGWindowLayer as String] as? Int ?? 0
        let h = (w[kCGWindowBounds as String] as? [String: Any])?["Height"] as? Double ?? 0
        let hit = mode == "status" ? layer == 25 : (layer >= 3 && layer <= 20 && h >= 100)
        if hit {
            print(Int64(Date().timeIntervalSince1970 * 1000))
            exit(0)
        }
    }
    usleep(1000)
}
print("timeout")
exit(1)
