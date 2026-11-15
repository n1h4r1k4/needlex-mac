// Renders one of NeedleX's pages (in its browser demo mode) to a PNG.
// usage: snap <file.html> <query> <out.png> <width> <height> <delay seconds> [script.js]
// The view is drawn at 2x. JavaScript errors on the page are printed. The
// optional script runs before the page's own (e.g. a song for the demo).
import AppKit
import WebKit

let a = CommandLine.arguments
guard a.count == 7 || a.count == 8, let w = Double(a[4]), let h = Double(a[5]), let delay = Double(a[6]) else {
    print("usage: snap <file.html> <query> <out.png> <width> <height> <delay> [script.js]")
    exit(2)
}
var comps = URLComponents(url: URL(fileURLWithPath: a[1]), resolvingAgainstBaseURL: false)!
comps.query = a[2].isEmpty ? nil : a[2]
let out = URL(fileURLWithPath: a[3])

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let win = NSWindow(contentRect: NSRect(x: -10000, y: 0, width: w, height: h), styleMask: [.borderless], backing: .buffered, defer: false)
let cfg = WKWebViewConfiguration()
cfg.preferences.setValue(true, forKey: "allowFileAccessFromFileURLs")
cfg.userContentController.addUserScript(WKUserScript(
    source: "window.__errs=[];addEventListener('error',e=>__errs.push(e.message+' @'+e.lineno));addEventListener('unhandledrejection',e=>__errs.push('rejected: '+e.reason))",
    injectionTime: .atDocumentStart, forMainFrameOnly: true))
if a.count == 8, let extra = try? String(contentsOfFile: a[7], encoding: .utf8) {
    cfg.userContentController.addUserScript(WKUserScript(source: extra, injectionTime: .atDocumentStart, forMainFrameOnly: true))
}
let wv = WKWebView(frame: NSRect(x: 0, y: 0, width: w, height: h), configuration: cfg)
// the window is off screen: keep WebKit animating anyway (private, development only)
let occlusion = Selector(("_setWindowOcclusionDetectionEnabled:"))
if wv.responds(to: occlusion) {
    typealias F = @convention(c) (AnyObject, Selector, Bool) -> Void
    unsafeBitCast(wv.method(for: occlusion), to: F.self)(wv, occlusion, false)
}
win.contentView = wv
win.orderFrontRegardless()
wv.loadFileURL(comps.url!, allowingReadAccessTo: URL(fileURLWithPath: "/"))

DispatchQueue.main.asyncAfter(deadline: .now() + delay) {
    wv.evaluateJavaScript("JSON.stringify(window.__errs || [])") { r, _ in
        if let s = r as? String, s != "[]" { print("page errors: \(s)") }
    }
    wv.takeSnapshot(with: nil) { img, err in
        guard let img = img, let tiff = img.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff),
              let png = rep.representation(using: .png, properties: [:]) else {
            print("snapshot failed: \(String(describing: err))")
            exit(1)
        }
        try! png.write(to: out)
        exit(0)
    }
}
app.run()
