// Generates the app icon, menu bar template icon and README logo.
// Run: swift Resources/Icon/generate.swift   (see build-icons.sh)
import AppKit

let outDir = URL(fileURLWithPath: CommandLine.arguments.dropFirst().first ?? "Resources/Icon")
try? FileManager.default.createDirectory(at: outDir, withIntermediateDirectories: true)

func savePNG(_ image: NSImage, pixels: Int, to name: String) {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8,
                               samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                               colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    rep.size = NSSize(width: pixels, height: pixels)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    NSGraphicsContext.current?.imageInterpolation = .high
    image.draw(in: NSRect(x: 0, y: 0, width: pixels, height: pixels), from: .zero, operation: .copy, fraction: 1)
    NSGraphicsContext.restoreGraphicsState()
    let png = rep.representation(using: .png, properties: [:])!
    try! png.write(to: outDir.appendingPathComponent(name))
}

/// Speech bubble: rounded rect with a small tail at the bottom-left or bottom-right.
func bubble(in r: NSRect, radius: CGFloat, tailRight: Bool) -> NSBezierPath {
    let p = NSBezierPath(roundedRect: r, xRadius: radius, yRadius: radius)
    let tail = NSBezierPath()
    let w = r.width * 0.16, h = r.height * 0.18
    if tailRight {
        tail.move(to: NSPoint(x: r.maxX - radius * 0.9, y: r.minY + 1))
        tail.line(to: NSPoint(x: r.maxX - radius * 0.9 - w, y: r.minY + 1))
        tail.line(to: NSPoint(x: r.maxX - radius * 0.6, y: r.minY - h))
    } else {
        tail.move(to: NSPoint(x: r.minX + radius * 0.9, y: r.minY + 1))
        tail.line(to: NSPoint(x: r.minX + radius * 0.9 + w, y: r.minY + 1))
        tail.line(to: NSPoint(x: r.minX + radius * 0.6, y: r.minY - h))
    }
    tail.close()
    p.append(tail)
    return p
}

func drawText(_ s: String, in rect: NSRect, size: CGFloat, color: NSColor, weight: NSFont.Weight = .bold,
              yNudge: CGFloat = 0) {
    let font = NSFont.systemFont(ofSize: size, weight: weight)
    let attrs: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: color]
    let line = CTLineCreateWithAttributedString(NSAttributedString(string: s, attributes: attrs))
    var ascent: CGFloat = 0, descent: CGFloat = 0
    let width = CGFloat(CTLineGetTypographicBounds(line, &ascent, &descent, nil))
    // center the ink box approximated by cap height for Latin, full em box for others
    let inkHeight = s.unicodeScalars.allSatisfy({ $0.isASCII }) ? font.capHeight : (ascent - descent) * 0.72
    let baseline = rect.midY - inkHeight / 2 + yNudge * size
    let ctx = NSGraphicsContext.current!.cgContext
    ctx.saveGState()
    ctx.textPosition = CGPoint(x: rect.midX - width / 2, y: baseline)
    CTLineDraw(line, ctx)
    ctx.restoreGState()
}

// MARK: - App icon (1024 canvas, 824 content, macOS squircle-ish)
let iconImage = NSImage(size: NSSize(width: 1024, height: 1024), flipped: false) { _ in
    let content = NSRect(x: 100, y: 100, width: 824, height: 824)
    let shape = NSBezierPath(roundedRect: content, xRadius: 185, yRadius: 185)

    // shadow
    NSGraphicsContext.saveGraphicsState()
    let shadow = NSShadow(); shadow.shadowBlurRadius = 24; shadow.shadowOffset = NSSize(width: 0, height: -10)
    shadow.shadowColor = NSColor.black.withAlphaComponent(0.28); shadow.set()
    NSColor.black.setFill(); shape.fill()
    NSGraphicsContext.restoreGraphicsState()

    // background gradient
    let bg = NSGradient(colors: [NSColor(srgbRed: 0.18, green: 0.55, blue: 1.00, alpha: 1),
                                 NSColor(srgbRed: 0.42, green: 0.36, blue: 1.00, alpha: 1)])!
    bg.draw(in: shape, angle: -60)

    // subtle top highlight
    NSGraphicsContext.saveGraphicsState()
    shape.addClip()
    let hl = NSGradient(colors: [NSColor.white.withAlphaComponent(0.18), NSColor.white.withAlphaComponent(0)])!
    hl.draw(in: NSRect(x: 100, y: 500, width: 824, height: 424), angle: 90)
    NSGraphicsContext.restoreGraphicsState()

    // back bubble: white with "A"
    let back = NSRect(x: 190, y: 420, width: 420, height: 330)
    NSGraphicsContext.saveGraphicsState()
    let s1 = NSShadow(); s1.shadowBlurRadius = 30; s1.shadowOffset = NSSize(width: 0, height: -14)
    s1.shadowColor = NSColor.black.withAlphaComponent(0.22); s1.set()
    NSColor.white.setFill(); bubble(in: back, radius: 90, tailRight: false).fill()
    NSGraphicsContext.restoreGraphicsState()
    drawText("A", in: back, size: 230, color: NSColor(srgbRed: 0.12, green: 0.16, blue: 0.36, alpha: 1))

    // front bubble: navy with "가"
    let front = NSRect(x: 430, y: 250, width: 420, height: 330)
    NSGraphicsContext.saveGraphicsState()
    let s2 = NSShadow(); s2.shadowBlurRadius = 30; s2.shadowOffset = NSSize(width: 0, height: -14)
    s2.shadowColor = NSColor.black.withAlphaComponent(0.30); s2.set()
    NSColor(srgbRed: 0.10, green: 0.13, blue: 0.30, alpha: 1).setFill()
    bubble(in: front, radius: 90, tailRight: true).fill()
    NSGraphicsContext.restoreGraphicsState()
    drawText("가", in: front, size: 210, color: .white, yNudge: 0.04)
    return true
}
savePNG(iconImage, pixels: 1024, to: "AppIcon-1024.png")
savePNG(iconImage, pixels: 256, to: "logo-256.png")

// MARK: - Menu bar template icon (18pt, black + alpha; macOS tints it)
func menuBarImage(scale: CGFloat) -> NSImage {
    let pt: CGFloat = 18
    return NSImage(size: NSSize(width: pt, height: pt), flipped: false) { _ in
        let ctx = NSGraphicsContext.current!
        NSColor.black.setFill()
        // back bubble: outline made by filling, then erasing the inside
        let back = NSRect(x: 0.5, y: 7.0, width: 11.5, height: 9.0)
        NSBezierPath(roundedRect: back, xRadius: 2.6, yRadius: 2.6).fill()
        ctx.compositingOperation = .destinationOut
        NSBezierPath(roundedRect: back.insetBy(dx: 1.3, dy: 1.3), xRadius: 1.6, yRadius: 1.6).fill()
        ctx.compositingOperation = .sourceOver
        drawText("A", in: back, size: 6.8, color: .black, weight: .heavy)
        // front bubble: solid, knocked out of the back one for separation
        let front = NSRect(x: 7.0, y: 2.2, width: 10.5, height: 8.0)
        ctx.compositingOperation = .destinationOut
        bubble(in: front.insetBy(dx: -1.1, dy: -1.1), radius: 3.4, tailRight: true).fill()
        ctx.compositingOperation = .sourceOver
        bubble(in: front, radius: 2.4, tailRight: true).fill()
        return true
    }
}
savePNG(menuBarImage(scale: 1), pixels: 18, to: "MenuBarIcon.png")
savePNG(menuBarImage(scale: 2), pixels: 36, to: "MenuBarIcon@2x.png")
print("icons written to \(outDir.path)")
