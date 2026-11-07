//! Generated landscape tiles, so examples have colorful content behind the glass.

use embun::Image;

/// Sky top, sky near the horizon, far hills, near hills.
type Palette = ([u8; 3], [u8; 3], [u8; 3], [u8; 3]);

const PALETTES: [Palette; 6] = [
    ([255, 156, 120], [255, 214, 165], [70, 110, 120], [40, 70, 80]),
    ([120, 170, 255], [200, 225, 255], [80, 130, 90], [45, 85, 60]),
    ([60, 40, 110], [230, 120, 150], [50, 45, 90], [25, 25, 50]),
    ([250, 200, 90], [255, 240, 200], [160, 110, 70], [110, 70, 50]),
    ([20, 60, 90], [90, 170, 190], [30, 90, 100], [15, 45, 55]),
    ([240, 130, 180], [255, 200, 220], [70, 120, 110], [40, 80, 75]),
];

/// A `width` x `height` landscape, varied by `seed`.
pub fn landscape(seed: u32, width: u32, height: u32) -> Image {
    let (sky_top, sky_low, hill_far, hill_near) = PALETTES[seed as usize % PALETTES.len()];
    let mix = |a: [u8; 3], b: [u8; 3], t: f32| -> [u8; 3] {
        let t = t.clamp(0.0, 1.0);
        [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t) as u8)
    };
    let phase = seed as f32 * 1.7;
    let sun = (0.25 + 0.5 * ((seed * 37 % 100) as f32 / 100.0), 0.3);
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let (u, v) = (x as f32 / width as f32, y as f32 / height as f32);
            let far = 0.55 + 0.08 * (u * 7.0 + phase).sin() + 0.04 * (u * 17.0 + phase * 2.0).sin();
            let near = 0.72 + 0.07 * (u * 5.0 - phase).sin() + 0.03 * (u * 23.0 + phase).cos();
            let mut c = mix(sky_top, sky_low, v / far);
            let d = ((u - sun.0).powi(2) * (width as f32 / height as f32).powi(2) + (v - sun.1).powi(2)).sqrt();
            if d < 0.09 {
                c = [255, 248, 230];
            }
            if v > far {
                c = mix(hill_far, hill_near, (v - far) * 2.0);
            }
            if v > near {
                c = mix(hill_near, [hill_near[0] / 2, hill_near[1] / 2, hill_near[2] / 2], (v - near) * 3.0);
            }
            rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    Image::from_rgba(width, height, rgba)
}
