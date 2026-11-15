// Tiles same-size images into one JPEG, with a thin dark gap between them.
// usage: grid <out.jpg> <columns> <image>...
import AppKit
let a = CommandLine.arguments
let out = a[1], cols = Int(a[2])!
let imgs = a[3...].map { NSImage(contentsOfFile: $0)! }
let w = imgs[0].size.width, h = imgs[0].size.height, gap: CGFloat = 8
let rows = (imgs.count + cols - 1) / cols
let size = NSSize(width: w * CGFloat(cols) + gap * CGFloat(cols - 1), height: h * CGFloat(rows) + gap * CGFloat(rows - 1))
let canvas = NSImage(size: size)
canvas.lockFocus()
NSColor(white: 0.08, alpha: 1).setFill(); NSRect(origin: .zero, size: size).fill()
for (i, im) in imgs.enumerated() {
    let c = i % cols, r = i / cols
    im.draw(in: NSRect(x: CGFloat(c) * (w + gap), y: CGFloat(rows - 1 - r) * (h + gap), width: w, height: h))
}
canvas.unlockFocus()
let rep = NSBitmapImageRep(data: canvas.tiffRepresentation!)!
try! rep.representation(using: .jpeg, properties: [.compressionFactor: 0.8])!.write(to: URL(fileURLWithPath: out))
