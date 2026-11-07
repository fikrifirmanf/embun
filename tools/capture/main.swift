// EmbunCapture: shows a backdrop with real macOS glass on it and screenshots the
// window twice, first without the glass ("bare") and then with it ("glass").
// Embun's compare example draws its own glass over the bare image and diffs the
// result against the macOS one.
//
// Usage: EmbunCapture --backdrop <png> --out <prefix> [--dark]
// Writes <prefix>-bare.png, <prefix>-glass.png and <prefix>-scene.txt.

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
}

struct SceneView: View {
    let backdrop: NSImage
    @ObservedObject var state: SceneState

    var body: some View {
        ZStack(alignment: .topLeading) {
            Image(nsImage: backdrop)
                .resizable()
                .aspectRatio(contentMode: .fill)
                .frame(width: viewport.width, height: viewport.height, alignment: .top)
                .clipped()
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

    init(prefix: String, backdrop: NSImage, dark: Bool) {
        self.prefix = prefix
        self.backdrop = backdrop
        self.dark = dark
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
        window.contentView = NSHostingView(rootView: SceneView(backdrop: backdrop, state: state))
        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate()

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
                     "appearance \(dark ? "dark" : "light")"]
        for s in shapes {
            lines.append("shape \(s.cx) \(s.cy) \(s.halfWidth) \(s.halfHeight) \(s.radius)")
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
    fail("usage: EmbunCapture --backdrop <png> --out <prefix> [--dark]")
}
guard let backdrop = NSImage(contentsOfFile: backdropPath) else {
    fail("could not load \(backdropPath)")
}

let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = AppDelegate(prefix: prefix, backdrop: backdrop, dark: CommandLine.arguments.contains("--dark"))
app.delegate = delegate
app.run()
