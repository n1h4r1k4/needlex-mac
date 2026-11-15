// Keeps the top of an image (full width) and saves it as a JPEG.
// usage: crop <in.png> <out.jpg> <height in pixels>
import AppKit
let a = CommandLine.arguments
let src = NSBitmapImageRep(data: try! Data(contentsOf: URL(fileURLWithPath: a[1])))!
let h = Int(a[3])!
let cg = src.cgImage!.cropping(to: CGRect(x: 0, y: 0, width: src.pixelsWide, height: h))!
let rep = NSBitmapImageRep(cgImage: cg)
try! rep.representation(using: .jpeg, properties: [.compressionFactor: 0.82])!.write(to: URL(fileURLWithPath: a[2]))
