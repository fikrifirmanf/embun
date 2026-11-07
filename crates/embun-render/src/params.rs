/// Which parts of the glass effect are drawn.
///
/// The default is what macOS 27 draws for resting glass controls: frost,
/// lensing, color fringe and merging. The outline and the edge highlight are
/// part of the measured look and always drawn. Rim light and the drop shadow are off,
/// because macOS draws neither on resting controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlassLayers {
    pub frost: bool,
    pub lensing: bool,
    pub rim: bool,
    pub fringe: bool,
    pub shadow: bool,
    pub merge: bool,
}

impl Default for GlassLayers {
    fn default() -> Self {
        Self { frost: true, lensing: true, rim: false, fringe: true, shadow: false, merge: true }
    }
}

/// How the glass recolors the backdrop, per pixel and channel, on
/// gamma-encoded values from 0 to 1:
///
/// `offset + gain * L + curve * L² + chroma * (c - L) + sharp_gain * Ls + sharp_chroma * (s - Ls) + blend * e`
///
/// where `c` is the frosted (blurred) backdrop with luminance `L`, `s` is the
/// unblurred backdrop with luminance `Ls`, and `e` is the lighter of `c` and
/// `s` for light glass, the darker for dark glass. Focused macOS glass mixes
/// in the sharp backdrop, most at the clear setting; unfocused glass does not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub offset: f32,
    pub gain: f32,
    pub curve: f32,
    /// Scales color away from gray; above `gain` means more saturated.
    pub chroma: f32,
    pub sharp_gain: f32,
    pub sharp_chroma: f32,
    /// Weight of the lighten (light glass) or darken (dark glass) blend of
    /// the sharp backdrop over the frost.
    pub blend: f32,
}

/// How tinted (prominent) glass is drawn, per channel on gamma-encoded values:
///
/// `amount * t + offset + gain * L + curve * L²`
///
/// where `t` is the shape's tint color and `L` the luminance of the frosted
/// backdrop. A shape's tint strength mixes from the plain glass to this.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tint {
    pub amount: f32,
    pub offset: f32,
    pub gain: f32,
    pub curve: f32,
}

/// Shader inputs derived from a single glass level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlassParams {
    /// Frost blur, as a Gaussian standard deviation in logical pixels.
    pub blur_sigma: f32,
    /// Body for light glass.
    pub light: Body,
    /// Body for dark glass.
    pub dark: Body,
    /// Strength of the rim lensing; 0 is none. Focused macOS glass measures
    /// 1.3 at the clear setting and shows no bending at tinted.
    pub lensing: f32,
    /// Width of the band at the rim where the backdrop bends, in logical pixels.
    pub bevel: f32,
    /// Strength of the rim light, 0 to 1.
    pub rim: f32,
    /// Strength of the color fringe at the rim, 0 or 1.
    pub fringe: f32,
    /// Strength of the drop shadow, 0 or 1.
    pub shadow: f32,
    /// Smooth-minimum radius in logical pixels; 0 turns merging off.
    pub merge_k: f32,
    /// How much the one-pixel outline just outside the shape darkens the
    /// backdrop beside its left and right sides, for light and dark glass.
    pub outline: [f32; 2],
    /// The same above and below the shape. Corners blend between the two.
    pub outline_ends: [f32; 2],
    /// Brightening along the inside of the top and bottom edges, for light and
    /// dark glass, at its peak (gamma-encoded, 0 to 1). It fades over about
    /// two device pixels.
    pub highlight: [f32; 2],
    /// Tinted glass for light and dark appearance, or `None` to draw tinted
    /// shapes as plain glass, as macOS does in a window without focus.
    pub tint: Option<[Tint; 2]>,
}

/// Merge radius in logical pixels. Fitted to the neck macOS draws between two
/// 56 pt circles 6 pt apart in a `GlassEffectContainer` with spacing 22.
const MERGE_K: f32 = 19.0;

/// The macOS 27 default position of the glass slider.
pub const DEFAULT_LEVEL: f32 = 0.5;

/// Columns of the measured tables: blur sigma (pt); offset, gain, curve,
/// chroma, sharp gain, sharp chroma, blend for light glass; the same seven
/// for dark glass; outline at the sides (light, dark); outline at the ends (light,
/// dark); highlight (light, dark); lensing; bevel (pt).
const COLUMNS: usize = 23;

/// Focused windows, measured from macOS 27.0.1 captures (tools/capture) at
/// the slider's clear, middle and tinted positions, which map to levels 0,
/// 0.5 and 1. The blur stays at 16 pt; the slider changes how much of the
/// sharp backdrop shows through and how far the edge bends it.
const FOCUSED: [[f32; COLUMNS]; 3] = [
    [
        16.0, 0.426, -0.089, 0.099, 0.256, 0.146, 0.395, 0.416, 0.119, 0.299, -0.449, -0.004, 0.117, 0.434, 0.644,
        0.273, 0.714, 0.084, 0.0, 0.053, 0.204, 1.3, 13.0,
    ],
    [
        16.0, 0.529, 0.162, 0.035, 0.518, -0.014, 0.092, 0.283, 0.122, 0.661, -0.423, 0.558, -0.007, 0.133, 0.378,
        0.273, 0.714, 0.084, 0.0, 0.053, 0.204, 0.8, 14.0,
    ],
    [
        16.0, 0.701, 0.296, -0.01, 0.555, 0.009, 0.016, 0.0, 0.125, 0.496, -0.203, 0.524, 0.008, 0.015, 0.0, 0.273,
        0.714, 0.084, 0.0, 0.053, 0.204, 0.0, 14.0,
    ],
];

/// Tinted glass in focused windows: amount, offset, gain, curve for light,
/// then dark. Fitted to `.glassProminent` buttons tinted system blue, inside
/// 4 pt of the edge: RMS error 4.7, 3.5 and 2.2 out of 255 (light) and 5.0,
/// 4.7 and 2.4 (dark) at clear, middle and tinted.
const TINTED: [[f32; 8]; 3] = [
    [0.972, -0.146, 0.261, -0.096, 0.943, 0.030, 0.049, 0.007],
    [0.980, -0.102, 0.169, -0.051, 0.942, 0.029, 0.064, -0.013],
    [0.992, -0.056, 0.082, -0.014, 0.975, 0.011, 0.041, -0.017],
];

/// Unfocused windows, measured the same way from captures of a window
/// without focus. Frosted only, no sharp backdrop and no highlight; the
/// outline at the ends is two thirds of the sides.
const UNFOCUSED: [[f32; COLUMNS]; 3] = [
    [
        6.0, 0.405, 0.598, -0.078, 1.131, 0.0, 0.0, 0.0, 0.088, 0.939, -0.313, 0.984, 0.0, 0.0, 0.0, 0.264, 0.536,
        0.174, 0.354, 0.0, 0.0, 1.0, 18.0,
    ],
    [
        12.0, 0.527, 0.462, -0.047, 0.914, 0.0, 0.0, 0.0, 0.087, 0.95, -0.329, 1.01, 0.0, 0.0, 0.0, 0.24, 0.536, 0.158,
        0.354, 0.0, 0.0, 0.8, 18.0,
    ],
    [
        16.0, 0.698, 0.281, -0.015, 0.583, 0.0, 0.0, 0.0, 0.103, 0.521, -0.208, 0.539, 0.0, 0.0, 0.0, 0.202, 0.536,
        0.133, 0.354, 0.0, 0.0, 0.6, 18.0,
    ],
];

impl GlassParams {
    /// Maps the glass slider (0 = clear, 1 = tinted) to shader inputs for a
    /// focused window, the look of the app in use.
    ///
    /// Frost, body, outline and highlight follow the measured macOS values,
    /// interpolated between the three captured slider positions.
    /// `reduce_transparency` overrides the level with a nearly opaque surface
    /// (not yet measured).
    pub fn from_level(level: f32, layers: GlassLayers, reduce_transparency: bool) -> Self {
        let mut p = Self::from_table(&FOCUSED, level, layers, reduce_transparency);
        p.tint = Some(if reduce_transparency {
            [Tint { amount: 1.0, offset: 0.0, gain: 0.0, curve: 0.0 }; 2]
        } else {
            let t = interpolate(&TINTED, level);
            let tint = |i: usize| Tint { amount: t[i], offset: t[i + 1], gain: t[i + 2], curve: t[i + 3] };
            [tint(0), tint(4)]
        });
        p
    }

    /// The same for a window without focus, where macOS glass is frosted
    /// more, shows no sharp backdrop, and draws tinted shapes untinted.
    pub fn unfocused(level: f32, layers: GlassLayers, reduce_transparency: bool) -> Self {
        Self::from_table(&UNFOCUSED, level, layers, reduce_transparency)
    }

    fn from_table(table: &[[f32; COLUMNS]; 3], level: f32, layers: GlassLayers, reduce: bool) -> Self {
        let m = interpolate(table, level);
        let (light, dark) = if reduce {
            let opaque = |offset| Body {
                offset,
                gain: 0.08,
                curve: 0.0,
                chroma: 0.08,
                sharp_gain: 0.0,
                sharp_chroma: 0.0,
                blend: 0.0,
            };
            (opaque(0.9), opaque(0.11))
        } else {
            let body = |i: usize| Body {
                offset: m[i],
                gain: m[i + 1],
                curve: m[i + 2],
                chroma: m[i + 3],
                sharp_gain: m[i + 4],
                sharp_chroma: m[i + 5],
                blend: m[i + 6],
            };
            (body(1), body(8))
        };
        Self {
            blur_sigma: match (layers.frost, reduce) {
                (false, _) => 0.0,
                (true, true) => 20.0,
                (true, false) => m[0],
            },
            light,
            dark,
            lensing: if layers.lensing && !reduce { m[21] } else { 0.0 },
            bevel: m[22],
            rim: if layers.rim { 1.0 } else { 0.0 },
            fringe: if layers.fringe && !reduce { 1.0 } else { 0.0 },
            shadow: if layers.shadow { 1.0 } else { 0.0 },
            merge_k: if layers.merge { MERGE_K } else { 0.0 },
            outline: [m[15], m[16]],
            outline_ends: [m[17], m[18]],
            highlight: [m[19], m[20]],
            tint: None,
        }
    }
}

/// The row for `level`, interpolated between the captured positions 0, 0.5
/// and 1. Out-of-range levels clamp; NaN falls back to the default.
fn interpolate<const N: usize>(table: &[[f32; N]; 3], level: f32) -> [f32; N] {
    let l = if level.is_finite() { level.clamp(0.0, 1.0) } else { DEFAULT_LEVEL };
    let (a, b, t) = if l <= 0.5 { (0, 1, l * 2.0) } else { (1, 2, l * 2.0 - 1.0) };
    std::array::from_fn(|i| lerp(table[a][i], table[b][i], t))
}

impl Default for GlassParams {
    fn default() -> Self {
        Self::from_level(DEFAULT_LEVEL, GlassLayers::default(), false)
    }
}

/// Exact at both ends, so the captured slider positions return measured values.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_slider_positions_return_the_measured_values() {
        let row_body = |row: &[f32; COLUMNS], i: usize| Body {
            offset: row[i],
            gain: row[i + 1],
            curve: row[i + 2],
            chroma: row[i + 3],
            sharp_gain: row[i + 4],
            sharp_chroma: row[i + 5],
            blend: row[i + 6],
        };
        type Make = fn(f32, GlassLayers, bool) -> GlassParams;
        for (table, make) in [(&FOCUSED, GlassParams::from_level as Make), (&UNFOCUSED, GlassParams::unfocused as Make)] {
            for (level, row) in [0.0, 0.5, 1.0].into_iter().zip(table) {
                let p = make(level, GlassLayers::default(), false);
                assert_eq!(p.blur_sigma, row[0]);
                assert_eq!(p.light, row_body(row, 1));
                assert_eq!(p.dark, row_body(row, 8));
                assert_eq!(p.outline, [row[15], row[16]]);
                assert_eq!(p.outline_ends, [row[17], row[18]]);
                assert_eq!(p.highlight, [row[19], row[20]]);
                assert_eq!((p.lensing, p.bevel), (row[21], row[22]));
            }
        }
    }

    #[test]
    fn levels_between_captures_interpolate() {
        let p = GlassParams::unfocused(0.25, GlassLayers::default(), false);
        assert!((p.blur_sigma - 9.0).abs() < 1e-5, "blur was {}", p.blur_sigma);
        assert!((p.light.offset - (0.405 + 0.527) / 2.0).abs() < 1e-5);
        let p = GlassParams::from_level(0.75, GlassLayers::default(), false);
        assert!((p.light.blend - (0.283 + 0.0) / 2.0).abs() < 1e-5, "blend was {}", p.light.blend);
    }

    #[test]
    fn focused_glass_shows_more_sharp_backdrop_at_clear_than_tinted() {
        // The sharp backdrop enters through the sharp gain and the blend.
        let sharp = |b: Body| b.sharp_gain + b.blend;
        let at = |level| GlassParams::from_level(level, GlassLayers::default(), false);
        for body in [|p: GlassParams| p.light, |p: GlassParams| p.dark] {
            assert!(sharp(body(at(0.0))) > sharp(body(at(0.5))));
            assert!(sharp(body(at(0.5))) > sharp(body(at(1.0))));
        }
        let unfocused = GlassParams::unfocused(0.0, GlassLayers::default(), false);
        assert_eq!((unfocused.light.sharp_gain, unfocused.light.blend), (0.0, 0.0));
    }

    #[test]
    fn tint_is_measured_when_focused_and_off_without_focus() {
        for (level, row) in [0.0, 0.5, 1.0].into_iter().zip(&TINTED) {
            let [light, dark] = GlassParams::from_level(level, GlassLayers::default(), false).tint.unwrap();
            assert_eq!([light.amount, light.offset, light.gain, light.curve], row[..4]);
            assert_eq!([dark.amount, dark.offset, dark.gain, dark.curve], row[4..]);
        }
        // Prominent buttons lose their color in a window without focus.
        assert_eq!(GlassParams::unfocused(0.5, GlassLayers::default(), false).tint, None);
        // Reduce Transparency draws the tint color solid.
        let [light, _] = GlassParams::from_level(0.5, GlassLayers::default(), true).tint.unwrap();
        assert_eq!((light.amount, light.gain), (1.0, 0.0));
    }

    #[test]
    fn level_is_clamped_and_nan_falls_back_to_default() {
        let layers = GlassLayers::default();
        assert_eq!(GlassParams::from_level(-3.0, layers, false), GlassParams::from_level(0.0, layers, false));
        assert_eq!(GlassParams::from_level(7.0, layers, false), GlassParams::from_level(1.0, layers, false));
        assert_eq!(GlassParams::from_level(f32::NAN, layers, false), GlassParams::default());
    }

    #[test]
    fn reduce_transparency_wins_over_level_and_layers() {
        for level in [0.0, 0.5, 1.0] {
            let p = GlassParams::from_level(level, GlassLayers::default(), true);
            assert_eq!(p.light.offset, 0.9);
            assert_eq!(p.dark.gain, 0.08);
            assert_eq!(p.light.sharp_gain, 0.0);
            assert_eq!(p.lensing, 0.0);
            assert_eq!(p.fringe, 0.0);
        }
    }

    #[test]
    fn disabled_layers_are_zero() {
        let off = GlassLayers { frost: false, lensing: false, rim: false, fringe: false, shadow: false, merge: false };
        let p = GlassParams::from_level(0.5, off, false);
        assert_eq!((p.blur_sigma, p.lensing, p.rim, p.fringe, p.shadow, p.merge_k), (0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
    }
}
