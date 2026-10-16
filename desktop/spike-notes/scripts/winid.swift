import CoreGraphics
import Foundation
// Prints "<windowNumber> <layer> <bounds>" for on-screen windows owned by the given process name.
let owner = CommandLine.arguments.dropFirst().first ?? "QuickTranslateSpike"
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
for w in list where (w[kCGWindowOwnerName as String] as? String) == owner {
    print(w[kCGWindowNumber as String]!, w[kCGWindowLayer as String]!, w[kCGWindowBounds as String]!)
}
