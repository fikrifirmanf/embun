# Capturing macOS glass for comparison

Embun is tuned against real macOS glass, not against memory. These tools put
Apple's own glass over Embun's test backdrops, screenshot it, and measure how
far Embun's render is from it.

- `main.swift` builds `EmbunCapture`, a small SwiftUI app. It shows a backdrop
  in a 960 x 600 pt window, screenshots it bare, adds `.glassEffect(.regular)`
  shapes (a lens, two merging round buttons, a toolbar) and screenshots again.
- `capture.sh` runs the app over every backdrop (photos, checker, solid) in
  light and dark, for one setting of the system glass slider.
- `compare-all.sh` renders Embun over each bare screenshot with the same
  shapes and prints the error per shape.

## Checklist

Do this on the Mac that is the reference (macOS 27.0.1). It takes about five
minutes.

1. Use the built-in Retina display, or any display at 2x scaling.
2. Turn off anything that changes how windows look: Increase contrast, Reduce
   transparency, and third-party window tools.
3. Give your terminal Screen Recording permission (System Settings, Privacy &
   Security, Screen Recording). The first run asks for it if it is missing.
4. In System Settings, search for "glass" and move the slider fully to Clear.
   Run `tools/capture/capture.sh clear`.
5. Move the slider to the middle. Run `tools/capture/capture.sh middle`.
6. Move the slider fully to Tinted. Run `tools/capture/capture.sh tinted`.
7. Put the slider back where you like it.
8. Run `tools/capture/compare-all.sh`.

Each run opens a window in the middle of the screen for about three seconds
per backdrop. Don't move the mouse over it while it runs.

## Output

Captures land in `reference/macos-<version>/<setting>/`:

| File | What it is |
|---|---|
| `photos-light-bare.png` | The backdrop alone, as macOS drew it |
| `photos-light-glass.png` | The same with macOS glass on top |
| `photos-light-scene.txt` | Window size, scale and shape geometry in points |
| `photos-light-compare.png` | macOS, Embun and their difference (x4), side by side |
| `about.txt` | macOS version and build, slider setting, time |

The compare report gives two numbers per shape: Embun's error against macOS,
and the error of drawing no glass at all. The second is the yardstick. Embun is
improving when the first number falls; it is worse than nothing when the first
is larger than the second.

## Limits

- The slider position "middle" is set by eye, so middle captures are less
  exact than clear and tinted.
- macOS glass also reacts to the content under it and to window focus. The
  capture window is focused, and the backdrops are fixed.
- The capture is in sRGB because the app sets its window to sRGB. Colors on a
  P3 display are converted, which can shift saturated colors by a step or two.
