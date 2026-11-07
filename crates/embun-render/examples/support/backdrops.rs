//! Procedural backdrops for examples and tests: a photo grid and a test grid.
//! Both repeat vertically, so scrolling wraps without a seam.
#![allow(dead_code)]

pub const WIDTH: u32 = 1200;
pub const HEIGHT: u32 = 1782;

pub struct Image {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl Image {
    fn new(width: u32, height: u32, fill: [u8; 3]) -> Self {
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..width * height {
            rgba.extend_from_slice(&[fill[0], fill[1], fill[2], 255]);
        }
        Self { rgba, width, height }
    }

    fn put(&mut self, x: u32, y: u32, c: [u8; 3]) {
        let i = ((y * self.width + x) * 4) as usize;
        self.rgba[i..i + 3].copy_from_slice(&c);
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]
    }
}

struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0 as f32 / 4_294_967_296.0
    }
}

fn hsl(h: f32, s: f32, l: f32) -> [f32; 3] {
    let h = h.rem_euclid(360.0) / 360.0;
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let ch = |t: f32| {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    [ch(h + 1.0 / 3.0), ch(h), ch(h - 1.0 / 3.0)]
}

fn to_u8(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// A grid of generated landscape "photos": sky, sun and two mountain ridges.
pub fn photos(dark: bool) -> Image {
    let mut img = Image::new(WIDTH, HEIGHT, if dark { [16, 17, 20] } else { [244, 244, 246] });
    let (cols, gap) = (4u32, 12.0f32);
    let tile = (WIDTH as f32 - gap * (cols as f32 + 1.0)) / cols as f32;
    let pitch = tile + gap;
    let rows = (HEIGHT as f32 / pitch).round() as u32;
    let mut rng = Rng(11);
    for row in 0..rows {
        for col in 0..cols {
            let x0 = gap + col as f32 * pitch;
            let y0 = gap / 2.0 + row as f32 * pitch;
            let hue = rng.next() * 360.0;
            let night = rng.next() < 0.3;
            let (top, bottom) = if night {
                (hsl(hue, 0.55, 0.12), hsl(hue + 30.0, 0.6, 0.32))
            } else {
                (hsl(hue, 0.75, 0.74), hsl(hue + 40.0, 0.8, 0.56))
            };
            let sun = if night { hsl(hue + 50.0, 0.3, 0.9) } else { hsl(hue + 60.0, 0.95, 0.88) };
            let (sx, sy, sr) = (0.2 + rng.next() * 0.6, 0.2 + rng.next() * 0.25, 0.08 + rng.next() * 0.06);
            let ridges: Vec<(Vec<f32>, [f32; 3])> = (0..2)
                .map(|layer| {
                    let base = 0.55 + layer as f32 * 0.15;
                    let pts = (0..=6).map(|_| base - rng.next() * 0.22).collect();
                    let color = if layer == 0 {
                        hsl(hue + 200.0, 0.35, if night { 0.18 } else { 0.34 })
                    } else {
                        hsl(hue + 180.0, 0.3, if night { 0.08 } else { 0.18 })
                    };
                    (pts, color)
                })
                .collect();
            for py in 0..tile as u32 {
                for px in 0..tile as u32 {
                    let (u, v) = (px as f32 / tile, py as f32 / tile);
                    if !inside_round_rect(px as f32, py as f32, tile, 10.0) {
                        continue;
                    }
                    let mut c = mix(top, bottom, v);
                    if ((u - sx).powi(2) + (v - sy).powi(2)).sqrt() < sr {
                        c = sun;
                    }
                    for (pts, color) in &ridges {
                        let f = u * 6.0;
                        let i = (f.floor() as usize).min(5);
                        let ridge = pts[i] + (pts[i + 1] - pts[i]) * (f - i as f32);
                        if v > ridge {
                            c = *color;
                        }
                    }
                    let (x, y) = (x0 as u32 + px, (y0 as u32 + py) % HEIGHT);
                    if x < WIDTH {
                        img.put(x, y, to_u8(c));
                    }
                }
            }
        }
    }
    img
}

fn inside_round_rect(x: f32, y: f32, size: f32, r: f32) -> bool {
    let cx = x.clamp(r, size - r);
    let cy = y.clamp(r, size - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

/// A checkerboard with red rows and blue columns every five cells; distortion is easy to see.
pub fn checker(dark: bool) -> Image {
    let (a, b) = if dark { ([36, 38, 43], [14, 15, 18]) } else { ([255, 255, 255], [217, 220, 227]) };
    let cell = 54;
    let mut img = Image::new(WIDTH, HEIGHT, a);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let c = if y % (cell * 5) < 4 {
                [224, 68, 42]
            } else if x % (cell * 5) < 4 {
                [42, 110, 224]
            } else if (x / cell + y / cell) % 2 == 1 {
                a
            } else {
                b
            };
            img.put(x, y, c);
        }
    }
    img
}

/// One flat color everywhere. Used by tests.
pub fn solid(width: u32, height: u32, color: [u8; 3]) -> Image {
    Image::new(width, height, color)
}

/// Smooth random color texture. Every patch looks different, so the compare
/// tools can find where each glass pixel samples the backdrop from.
pub fn noise() -> Image {
    let mut img = Image::new(WIDTH, HEIGHT, [0, 0, 0]);
    let mut rng = Rng(7);
    // Two octaves of value noise per channel, cells of 48 and 20 texels.
    let octaves: Vec<(f32, u32, Vec<[f32; 3]>)> = [(48u32, 0.6f32), (20, 0.4)]
        .into_iter()
        .map(|(cell, weight)| {
            let cols = WIDTH / cell + 2;
            let rows = HEIGHT / cell + 2;
            let grid = (0..cols * rows).map(|_| [rng.next(), rng.next(), rng.next()]).collect();
            (weight, cell, grid)
        })
        .collect();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let mut c = [0.0f32; 3];
            for (weight, cell, grid) in &octaves {
                let cols = WIDTH / cell + 2;
                let (fx, fy) = (x as f32 / *cell as f32, y as f32 / *cell as f32);
                let (ix, iy) = (fx as u32, fy as u32);
                let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
                let (tx, ty) = (smooth(fx.fract()), smooth(fy.fract()));
                let at = |i: u32, j: u32| grid[(j * cols + i) as usize];
                let top = mix(at(ix, iy), at(ix + 1, iy), tx);
                let bottom = mix(at(ix, iy + 1), at(ix + 1, iy + 1), tx);
                let v = mix(top, bottom, ty);
                for k in 0..3 {
                    c[k] += weight * v[k];
                }
            }
            // Stretch the middle of the range so contrast is high everywhere.
            img.put(x, y, to_u8(c.map(|v| (v - 0.5) * 2.2 + 0.5)));
        }
    }
    img
}
