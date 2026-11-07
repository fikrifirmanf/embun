# Capturing macOS glass for comparison

Embun is tuned against real macOS glass, not against memory. These tools put
Apple's own glass over Embun's test backdrops, screenshot it, and measure how
far Embun's render is from it.

- `main.swift` builds `EmbunCapture`, a small SwiftUI app. It shows a backdrop
  in a 960 x 600 pt window, screenshots it bare, adds glass and screenshots
  again. The shapes scene uses `.glassEffect(.regular)` on a lens, two merging
  round buttons and a toolbar. The controls scene uses real glass buttons
  (`.glass`, `.glassProminent` and circular) in all four control sizes and
  records where SwiftUI placed each one.
- `capture.sh` runs the app over every backdrop (photos, checker, solid, noise)
  in light and dark, for one setting of the system glass slider. The noise
  backdrop is there to measure how far the glass bends the content.
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
   Run `tools/capture/capture.sh clear all`.
5. Move the slider to the middle. Run `tools/capture/capture.sh middle all`.
6. Move the slider fully to Tinted. Run `tools/capture/capture.sh tinted all`.
7. Put the slider back where you like it.
8. Run `tools/capture/compare-all.sh`.

Each run opens a window in the middle of the screen for about three seconds
per backdrop, 16 times. Start it from a terminal window that is in front and
don't switch apps until it finishes. macOS only gives the capture window focus
while its terminal is in front, and glass looks different in a window without
focus. If focus is lost, the run stops with an error rather than saving the
wrong look.

To capture the look of a window without focus, add `inactive`:
`tools/capture/capture.sh clear all inactive`.

## Output

Captures land in `reference/macos-<version>/<setting>/`, or in
`reference/macos-<version>/inactive/<setting>/` for windows without focus.
Controls scene files start with `controls-`.

| File | What it is |
|---|---|
| `photos-light-bare.png` | The backdrop alone, as macOS drew it |
| `photos-light-glass.png` | The same with macOS glass on top |
| `photos-light-scene.txt` | Window size, scale and shape geometry in points |
| `photos-light-compare.png` | macOS, Embun and their difference (x4), side by side |
| `about.txt` | macOS version and build, slider setting, window focus, time |

The compare report gives two numbers per shape: Embun's error against macOS,
and the error of drawing no glass at all. The second is the yardstick. Embun is
improving when the first number falls; it is worse than nothing when the first
is larger than the second.

## Limits

- The slider position "middle" is set by eye, so middle captures are less
  exact than clear and tinted.
- macOS glass also reacts to the content under it. The backdrops are fixed.
- Noise-backdrop captures are not in git: random noise does not compress
  (about 76 MB for all settings). They stay on the capturing Mac.
- The first 18 captures, now in `inactive/`, were taken without focus by
  mistake. They are kept as the reference for unfocused windows.
- The capture is in sRGB because the app sets its window to sRGB. Colors on a
  P3 display are converted, which can shift saturated colors by a step or two.
