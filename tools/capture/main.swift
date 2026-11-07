// EmbunCapture: shows a backdrop with real macOS glass on it and screenshots the
// window twice, first without the glass ("bare") and then with it ("glass").
// Embun's compare example draws its own glass over the bare image and diffs the
// result against the macOS one.
//
// Usage: EmbunCapture --backdrop <png> --out <prefix> [--dark] [--scene shapes|controls] [--inactive]
// Writes <prefix>-bare.png, <prefix>-glass.png and <prefix>-scene.txt.
//
// The shapes scene uses .glassEffect on plain rectangles. The controls scene
// uses real SwiftUI glass buttons and records where SwiftUI placed each one.
//
// The window is made active and key, as an app in use is; glass looks
// different in inactive windows. --inactive leaves it inactive instead. Either
// way the capture fails if the window ends up in the other state.

import AppKit
import SwiftUI

/// Window size in points. Must match what compare reads from the scene file.
let viewport = CGSize(width: 960, height: 600)

struct GlassRect {
    let cx, cy, halfWidth, halfHeight, radius: CGFloat
}

/// The same shapes as Embun's snapshot example: a lens, two round buttons close
/// enough to merge, and a toolbar.
let shapes: [GlassRect] = [
    GlassRect(cx: viewport.width * 0.32, cy: viewport.height * 0.56, halfWidth: 96, halfHeight: 60, radius: 34),
    GlassRect(cx: viewport.width - 118, cy: viewport.height - 60, halfWidth: 28, halfHeight: 28, radius: 28),
    GlassRect(cx: viewport.width - 56, cy: viewport.height - 60, halfWidth: 28, halfHeight: 28, radius: 28),
    GlassRect(cx: viewport.width / 2, cy: 44, halfWidth: 230, halfHeight: 26, radius: 26),
]

/// Container spacing; Embun fits its merge radius (19 pt) to the neck this produces.
let mergeSpacing: CGFloat = 22

final class SceneState: ObservableObject {
    @Published var showGlass = false
    /// Frames of the controls scene's buttons in window points, in layout order.
    var controls: [(name: String, frame: CGRect, kind: String)] = []

    func record(_ name: String, _ frame: CGRect, _ kind: String) {
        if let i = controls.firstIndex(where: { $0.name == name }) {
            controls[i] = (name, frame, kind)
        } else {
            controls.append((name, frame, kind))
        }
    }
}

struct Backdrop: View {
    let image: NSImage

    var body: some View {
        Image(nsImage: image)
            .resizable()
            .aspectRatio(contentMode: .fill)
            .frame(width: viewport.width, height: viewport.height, alignment: .top)
            .clipped()
    }
}

struct SceneView: View {
    let backdrop: NSImage
    @ObservedObject var state: SceneState

    var body: some View {
        ZStack(alignment: .topLeading) {
            Backdrop(image: backdrop)
            if state.showGlass {
                GlassEffectContainer(spacing: mergeSpacing) {
                    ZStack(alignment: .topLeading) {
                        ForEach(shapes.indices, id: \.self) { i in
                            let s = shapes[i]
                            Color.clear
                                .frame(width: s.halfWidth * 2, height: s.halfHeight * 2)
                                .glassEffect(.regular, in: .rect(cornerRadius: s.radius))
                                .position(x: s.cx, y: s.cy)
                        }
                    }
                    .frame(width: viewport.width, height: viewport.height)
                }
            }
        }
        .frame(width: viewport.width, height: viewport.height)
    }
}

/// Real glass buttons in every control size. Labels are hidden so only the
/// glass is drawn; they still size the buttons like real ones.
struct ControlsView: View {
    let backdrop: NSImage
    @ObservedObject var state: SceneState

    var body: some View {
        ZStack(alignment: .topLeading) {
            Backdrop(image: backdrop)
            if state.showGlass {
                VStack(alignment: .leading, spacing: 56) {
                    HStack(spacing: 48) {
                        ForEach(sizes, id: \.name) { size in
                            textButton("glass-\(size.name)", size.size, prominent: false)
                        }
                    }
                    HStack(spacing: 48) {
                        ForEach(sizes, id: \.name) { size in
                            textButton("prominent-\(size.name)", size.size, prominent: true)
                        }
                    }
                    HStack(spacing: 48) {
                        ForEach(sizes, id: \.name) { size in
                            iconButton("circle-\(size.name)", size.size)
                        }
                    }
                }
                .padding(.leading, 140)
                .padding(.top, 150)
                // No hover highlight wherever the pointer happens to be.
                .allowsHitTesting(false)
            }
        }
        .frame(width: viewport.width, height: viewport.height)
    }

    let sizes: [(name: String, size: ControlSize)] = [
        ("small", .small), ("regular", .regular), ("large", .large), ("xlarge", .extraLarge),
    ]

    @ViewBuilder
    func textButton(_ name: String, _ size: ControlSize, prominent: Bool) -> some View {
        let button = Button {} label: { Text("Button").hidden() }.controlSize(size)
        Group {
            if prominent {
                // Without a tint the capture app has no accent and draws plain glass.
                button.buttonStyle(.glassProminent).tint(.blue)
            } else {
                button.buttonStyle(.glass)
            }
        }
        .onGeometryChange(for: CGRect.self, of: { $0.frame(in: .global) }) { state.record(name, $0, prominent ? "prominent" : "glass") }
    }

    func iconButton(_ name: String, _ size: ControlSize) -> some View {
        Button {} label: { Image(systemName: "plus").hidden() }
            .buttonStyle(.glass)
            .buttonBorderShape(.circle)
            .controlSize(size)
            .onGeometryChange(for: CGRect.self, of: { $0.frame(in: .global) }) { state.record(name, $0, "glass") }
    }
}

final class CaptureWindow: NSWindow {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data("EmbunCapture: \(message)\n".utf8))
    exit(1)
}

func argument(_ flag: String) -> String? {
    let args = CommandLine.arguments
    guard let i = args.firstIndex(of: flag), i + 1 < args.count else { return nil }
    return args[i + 1]
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    let state = SceneState()
    var window: CaptureWindow!
    let prefix: String
    let backdrop: NSImage
    let dark: Bool
    let controls: Bool
    let inactive: Bool

    init(prefix: String, backdrop: NSImage, dark: Bool, controls: Bool, inactive: Bool) {
        self.inactive = inactive
        self.prefix = prefix
        self.backdrop = backdrop
        self.dark = dark
        self.controls = controls
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        window = CaptureWindow(
            contentRect: NSRect(origin: .zero, size: viewport),
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        window.colorSpace = .sRGB
        window.isOpaque = true
        window.hasShadow = false
        window.contentView = controls
            ? NSHostingView(rootView: AnyView(ControlsView(backdrop: backdrop, state: state)))
            : NSHostingView(rootView: AnyView(SceneView(backdrop: backdrop, state: state)))
        window.center()
        if inactive {
            window.orderFront(nil)
        } else {
            window.makeKeyAndOrderFront(nil)
            // A plain activate() is only a request, and macOS refuses it when the
            // app was started from a terminal.
            NSApp.activate(ignoringOtherApps: true)
        }

        // Let the window settle, capture bare, show the glass, let its appear
        // animation finish, capture again.
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            self.screenshot("\(self.prefix)-bare.png")
            self.state.showGlass = true
            DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
                self.screenshot("\(self.prefix)-glass.png")
                self.writeScene()
                NSApp.terminate(nil)
            }
        }
    }

    func screenshot(_ path: String) {
        let active = NSApp.isActive && window.isKeyWindow
        if active == inactive {
            fail("the window is \(active ? "active" : "inactive") but this capture needs it \(inactive ? "inactive" : "active"). macOS only lets the capture take focus when the terminal that started it is the front window, so start it from a terminal in front and don't switch apps until it finishes.")
        }
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
        // -x: no sound, -o: no window shadow, -l: this window only.
        process.arguments = ["-x", "-o", "-l\(window.windowNumber)", path]
        do {
            try process.run()
        } catch {
            fail("could not run screencapture: \(error)")
        }
        process.waitUntilExit()
        if process.terminationStatus != 0 || !FileManager.default.fileExists(atPath: path) {
            fail("screencapture failed for \(path). Give your terminal Screen Recording permission in System Settings, Privacy & Security.")
        }
    }

    func writeScene() {
        var lines = ["# Embun capture scene. Units are points; origin top left.",
                     "viewport \(viewport.width) \(viewport.height)",
                     "scale \(window.backingScaleFactor)",
                     "appearance \(dark ? "dark" : "light")",
                     "window \(inactive ? "inactive" : "active")"]
        if controls {
            // Large and extra-large text buttons and all circle buttons are
            // capsules. Small and regular text buttons are rounded rectangles
            // whose radius, fitted to the 27.0.1 captures, is a quarter of the
            // height (5 pt and 6 pt). The name and kind follow the numbers;
            // compare reads only the numbers.
            if state.controls.isEmpty { fail("no control frames were recorded") }
            for c in state.controls {
                let f = c.frame
                let roundedRect = !c.name.hasPrefix("circle") && (c.name.hasSuffix("-small") || c.name.hasSuffix("-regular"))
                let radius = roundedRect ? f.height / 4 : f.height / 2
                lines.append("shape \(f.midX) \(f.midY) \(f.width / 2) \(f.height / 2) \(radius) \(c.kind) \(c.name)")
            }
        } else {
            for s in shapes {
                lines.append("shape \(s.cx) \(s.cy) \(s.halfWidth) \(s.halfHeight) \(s.radius)")
            }
        }
        let path = "\(prefix)-scene.txt"
        do {
            try (lines.joined(separator: "\n") + "\n").write(toFile: path, atomically: true, encoding: .utf8)
        } catch {
            fail("could not write \(path): \(error)")
        }
    }
}

guard let backdropPath = argument("--backdrop"), let prefix = argument("--out") else {
    fail("usage: EmbunCapture --backdrop <png> --out <prefix> [--dark] [--scene shapes|controls] [--inactive]")
}
guard let backdrop = NSImage(contentsOfFile: backdropPath) else {
    fail("could not load \(backdropPath)")
}

let app = NSApplication.shared
app.setActivationPolicy(.regular)
let scene = argument("--scene") ?? "shapes"
guard scene == "shapes" || scene == "controls" else { fail("--scene must be shapes or controls") }
let delegate = AppDelegate(
    prefix: prefix, backdrop: backdrop, dark: CommandLine.arguments.contains("--dark"), controls: scene == "controls",
    inactive: CommandLine.arguments.contains("--inactive"))
app.delegate = delegate
app.run()
