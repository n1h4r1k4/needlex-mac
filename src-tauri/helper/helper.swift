// needlex-helper – small companion process for NeedleX on macOS.
//
// stdin  (one command per line):
//   audio on | audio off
//   snap <x> <y> <w> <h> <out.jpg>      (global points, top-left origin)
//   loc                                 where the Mac is (for the weather)
//   cal                                 today's and tomorrow's events
// stdout (one event per line):
//   lvl <peak 0…1>                      ~30×/s while audio is on and not silent
//   snap ok | snap err <reason>
//   loc <lat> <lon> <city> | loc err <reason>
//   cal <json array> | cal err <reason>
//   log <text>
//
// * Audio level: a Core Audio process tap on the system output mix
//   (macOS 14.2+, asks for "System Audio Recording"). Only the peak level is
//   computed; no audio is stored or sent anywhere.
// * Snap: ScreenCaptureKit screenshot of a small screen area with NeedleX's
//   own windows left out (macOS 14+, asks for "Screen Recording").
// * Loc / cal: Location Services (to about a kilometre) and the Calendar
//   app's events, for the weather and calendar pages of the island; macOS
//   asks once for each.

import AppKit
import AudioToolbox
import CoreAudio
import CoreLocation
import EventKit
import Foundation
import ScreenCaptureKit
import UniformTypeIdentifiers

setvbuf(stdout, nil, _IOLBF, 0)
let parentPid = getppid()
let outLock = NSLock()
func say(_ s: String) {
    outLock.lock()
    print(s)
    fflush(stdout)
    outLock.unlock()
}

// ------------------------------------------------------------------ audio

final class LevelTap {
    private var tapID = AudioObjectID(kAudioObjectUnknown)
    private var aggID = AudioObjectID(kAudioObjectUnknown)
    private var procID: AudioDeviceIOProcID?
    private let queue = DispatchQueue(label: "needlex.tap", qos: .userInteractive)
    private let lock = NSLock()
    private var peak: Float = 0
    private var heardSomething = false
    private var timer: DispatchSourceTimer?
    private(set) var running = false

    func start() {
        guard !running else { return }
        guard #available(macOS 14.2, *) else {
            say("log audio tap needs macOS 14.2 or later")
            return
        }
        do {
            try create()
            running = true
            let t = DispatchSource.makeTimerSource(queue: .global(qos: .userInteractive))
            t.schedule(deadline: .now(), repeating: .milliseconds(33))
            t.setEventHandler { [weak self] in self?.tick() }
            t.resume()
            timer = t
        } catch {
            say("log audio tap failed: \(error)")
            destroy()
        }
    }

    func stop() {
        timer?.cancel()
        timer = nil
        destroy()
        running = false
    }

    private func tick() {
        lock.lock()
        let p = peak
        peak = 0
        if p > 0.0005 { heardSomething = true }
        let report = heardSomething
        lock.unlock()
        // Until real sound has come through (e.g. permission not granted yet,
        // which yields pure silence) stay quiet so the UI keeps its fallback.
        if report { say(String(format: "lvl %.4f", min(1, p))) }
    }

    @available(macOS 14.2, *)
    private func create() throws {
        let desc = CATapDescription(stereoGlobalTapButExcludeProcesses: [])
        desc.uuid = UUID()
        desc.isPrivate = true
        desc.muteBehavior = .unmuted
        var tap = AudioObjectID(kAudioObjectUnknown)
        try check(AudioHardwareCreateProcessTap(desc, &tap), "create tap")
        tapID = tap

        let outputUID = try defaultOutputUID()
        let agg: [String: Any] = [
            kAudioAggregateDeviceNameKey: "NeedleX Level",
            kAudioAggregateDeviceUIDKey: "needlex-level-" + UUID().uuidString,
            kAudioAggregateDeviceMainSubDeviceKey: outputUID,
            kAudioAggregateDeviceIsPrivateKey: true,
            kAudioAggregateDeviceIsStackedKey: false,
            kAudioAggregateDeviceTapAutoStartKey: true,
            kAudioAggregateDeviceSubDeviceListKey: [[kAudioSubDeviceUIDKey: outputUID]],
            kAudioAggregateDeviceTapListKey: [[
                kAudioSubTapDriftCompensationKey: true,
                kAudioSubTapUIDKey: desc.uuid.uuidString,
            ]],
        ]
        var aggDev = AudioObjectID(kAudioObjectUnknown)
        try check(AudioHardwareCreateAggregateDevice(agg as CFDictionary, &aggDev), "create aggregate device")
        aggID = aggDev

        var pid: AudioDeviceIOProcID?
        try check(AudioDeviceCreateIOProcIDWithBlock(&pid, aggDev, queue) { [weak self] _, input, _, _, _ in
            self?.consume(input)
        }, "create io proc")
        procID = pid
        try check(AudioDeviceStart(aggDev, pid), "start device")
    }

    private func consume(_ input: UnsafePointer<AudioBufferList>) {
        var m: Float = 0
        for buf in UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: input)) {
            guard let data = buf.mData else { continue }
            let n = Int(buf.mDataByteSize) / MemoryLayout<Float>.size
            let f = data.bindMemory(to: Float.self, capacity: n)
            for i in 0..<n {
                let v = abs(f[i])
                if v > m { m = v }
            }
        }
        lock.lock()
        if m > peak { peak = m }
        lock.unlock()
    }

    private func destroy() {
        if aggID != kAudioObjectUnknown {
            if let p = procID {
                AudioDeviceStop(aggID, p)
                AudioDeviceDestroyIOProcID(aggID, p)
            }
            AudioHardwareDestroyAggregateDevice(aggID)
        }
        if tapID != kAudioObjectUnknown, #available(macOS 14.2, *) {
            AudioHardwareDestroyProcessTap(tapID)
        }
        procID = nil
        aggID = AudioObjectID(kAudioObjectUnknown)
        tapID = AudioObjectID(kAudioObjectUnknown)
        lock.lock()
        heardSomething = false
        peak = 0
        lock.unlock()
    }

    private func defaultOutputUID() throws -> String {
        var dev = AudioObjectID(kAudioObjectUnknown)
        var size = UInt32(MemoryLayout<AudioObjectID>.size)
        var addr = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultSystemOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain)
        try check(AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &addr, 0, nil, &size, &dev), "default output")
        var uid: Unmanaged<CFString>?
        size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
        addr.mSelector = kAudioDevicePropertyDeviceUID
        try check(AudioObjectGetPropertyData(dev, &addr, 0, nil, &size, &uid), "output uid")
        guard let u = uid?.takeRetainedValue() else { throw TapError.message("no output uid") }
        return u as String
    }
}

enum TapError: Error { case message(String) }
func check(_ status: OSStatus, _ what: String) throws {
    if status != noErr { throw TapError.message("\(what) (\(status))") }
}

let tap = LevelTap()

/// Restart the tap when the default output changes (headphones plugged in…).
func watchOutputDevice() {
    var addr = AudioObjectPropertyAddress(
        mSelector: kAudioHardwarePropertyDefaultSystemOutputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain)
    AudioObjectAddPropertyListenerBlock(AudioObjectID(kAudioObjectSystemObject), &addr, DispatchQueue.main) { _, _ in
        if tap.running {
            tap.stop()
            tap.start()
        }
    }
}

// ------------------------------------------------------------------ snap

var cachedContent: (SCShareableContent, Date)?

@available(macOS 14.0, *)
func snap(_ rect: CGRect, to path: String) async {
    do {
        let content: SCShareableContent
        if let (c, at) = cachedContent, Date().timeIntervalSince(at) < 5 {
            content = c
        } else {
            content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
            cachedContent = (content, Date())
        }
        let mid = CGPoint(x: rect.midX, y: rect.midY)
        guard let display = content.displays.first(where: { $0.frame.contains(mid) }) ?? content.displays.first else {
            say("snap err no display")
            return
        }
        let ours = content.applications.filter { $0.processID == parentPid || $0.processID == getpid() }
        let filter = SCContentFilter(display: display, excludingApplications: ours, exceptingWindows: [])
        let cfg = SCStreamConfiguration()
        let local = rect.offsetBy(dx: -display.frame.minX, dy: -display.frame.minY)
        cfg.sourceRect = local
        let scale = CGFloat(filter.pointPixelScale)
        cfg.width = max(1, Int(local.width * scale))
        cfg.height = max(1, Int(local.height * scale))
        cfg.showsCursor = false
        let img = try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: cfg)
        let url = URL(fileURLWithPath: path) as CFURL
        guard let dest = CGImageDestinationCreateWithURL(url, UTType.jpeg.identifier as CFString, 1, nil) else {
            say("snap err cannot write")
            return
        }
        CGImageDestinationAddImage(dest, img, [kCGImageDestinationLossyCompressionQuality: 0.86] as CFDictionary)
        say(CGImageDestinationFinalize(dest) ? "snap ok" : "snap err cannot write")
    } catch {
        cachedContent = nil
        say("snap err \(error.localizedDescription.replacingOccurrences(of: "\n", with: " "))")
    }
}

// ------------------------------------------------------------------ location

final class Locator: NSObject, CLLocationManagerDelegate {
    private let manager = CLLocationManager()
    private var pending = false

    override init() {
        super.init()
        manager.delegate = self
        manager.desiredAccuracy = kCLLocationAccuracyKilometer
    }

    /// Main thread only.
    func request() {
        pending = true
        switch manager.authorizationStatus {
        case .notDetermined: manager.requestWhenInUseAuthorization() // answer arrives below
        case .denied, .restricted: finish("loc err denied")
        default: manager.requestLocation()
        }
    }

    private func finish(_ line: String) {
        guard pending else { return }
        pending = false
        say(line)
    }

    func locationManagerDidChangeAuthorization(_ m: CLLocationManager) {
        guard pending else { return }
        switch m.authorizationStatus {
        case .notDetermined: break
        case .denied, .restricted: finish("loc err denied")
        default: m.requestLocation()
        }
    }

    func locationManager(_ m: CLLocationManager, didUpdateLocations locations: [CLLocation]) {
        guard pending, let l = locations.last else { return }
        CLGeocoder().reverseGeocodeLocation(l) { marks, _ in
            let m = marks?.first
            let city = m?.locality ?? m?.subAdministrativeArea ?? m?.administrativeArea ?? ""
            DispatchQueue.main.async {
                self.finish(String(format: "loc %.3f %.3f ", l.coordinate.latitude, l.coordinate.longitude) + city)
            }
        }
    }

    func locationManager(_ m: CLLocationManager, didFailWithError error: Error) {
        finish("loc err " + error.localizedDescription.replacingOccurrences(of: "\n", with: " "))
    }
}

let locator = Locator()

// ------------------------------------------------------------------ calendar

let eventStore = EKEventStore()

func hexColor(_ c: CGColor?) -> String {
    guard let c = c, let s = c.converted(to: CGColorSpace(name: CGColorSpace.sRGB)!, intent: .defaultIntent, options: nil),
          let p = s.components, p.count >= 3 else { return "#8e8e93" }
    return String(format: "#%02x%02x%02x", Int(p[0] * 255), Int(p[1] * 255), Int(p[2] * 255))
}

func sendEvents() {
    let cal = Calendar.current
    let start = cal.startOfDay(for: Date())
    let end = cal.date(byAdding: .day, value: 2, to: start)!
    let events = eventStore.events(matching: eventStore.predicateForEvents(withStart: start, end: end, calendars: nil))
        .sorted { $0.startDate < $1.startDate }
        .prefix(40)
    let list: [[String: Any]] = events.map { e in [
        "title": e.title ?? "",
        "start": e.startDate.timeIntervalSince1970 * 1000,
        "end": e.endDate.timeIntervalSince1970 * 1000,
        "allDay": e.isAllDay,
        "color": hexColor(e.calendar?.cgColor),
    ] }
    guard let data = try? JSONSerialization.data(withJSONObject: list),
          let json = String(data: data, encoding: .utf8) else { say("cal err encode"); return }
    say("cal " + json)
}

func calendarEvents() {
    let answer: (Bool) -> Void = { ok in ok ? sendEvents() : say("cal err denied") }
    if #available(macOS 14.0, *) {
        switch EKEventStore.authorizationStatus(for: .event) {
        case .fullAccess: sendEvents()
        case .notDetermined: eventStore.requestFullAccessToEvents { ok, _ in answer(ok) }
        default: say("cal err denied")
        }
    } else {
        switch EKEventStore.authorizationStatus(for: .event) {
        case .authorized: sendEvents()
        case .notDetermined: eventStore.requestAccess(to: .event) { ok, _ in answer(ok) }
        default: say("cal err denied")
        }
    }
}

// ------------------------------------------------------------------ main loop

func handle(_ line: String) {
    let parts = line.split(separator: " ", omittingEmptySubsequences: true).map(String.init)
    guard let cmd = parts.first else { return }
    switch cmd {
    case "audio":
        DispatchQueue.main.async {
            if parts.count > 1 && parts[1] == "on" { tap.start() } else { tap.stop() }
        }
    case "snap":
        guard parts.count >= 6,
              let x = Double(parts[1]), let y = Double(parts[2]),
              let w = Double(parts[3]), let h = Double(parts[4]) else {
            say("snap err bad request")
            return
        }
        let path = parts[5...].joined(separator: " ")
        if #available(macOS 14.0, *) {
            Task { await snap(CGRect(x: x, y: y, width: w, height: h), to: path) }
        } else {
            say("snap err needs macOS 14")
        }
    case "loc":
        DispatchQueue.main.async { locator.request() }
    case "cal":
        DispatchQueue.main.async { calendarEvents() }
    case "quit":
        tap.stop()
        exit(0)
    default:
        break
    }
}

watchOutputDevice()
Thread.detachNewThread {
    while let line = readLine() {
        handle(line.trimmingCharacters(in: .whitespaces))
    }
    // NeedleX closed our stdin (quit or crashed): clean up and leave. The
    // main thread can be stuck in a permission prompt, so don't wait on it
    // for long – coreaudiod drops our private tap when the process ends.
    DispatchQueue.main.async {
        tap.stop()
        exit(0)
    }
    Thread.sleep(forTimeInterval: 1.5)
    exit(0)
}
// also leave if NeedleX disappears without closing the pipe
Thread.detachNewThread {
    while getppid() == parentPid {
        Thread.sleep(forTimeInterval: 2)
    }
    exit(0)
}
RunLoop.main.run()
