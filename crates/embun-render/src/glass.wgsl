// Glass pass: one fullscreen triangle. Pixels outside every shape show the
// backdrop; pixels inside are frosted, lensed and recolored, and a one-pixel
// outline sits just outside the shape. Frost and color math run on
// gamma-encoded values, which fits the macOS 27 captures better than linear.

struct Uniforms {
    viewport: vec2f,        // logical px
    backdrop_size: vec2f,   // texels
    scale_factor: f32,      // physical px per logical px
    backdrop_scale: f32,    // backdrop texels per logical px
    scroll: f32,            // backdrop texels
    shape_count: u32,
    blur: f32,              // Gaussian sigma, logical px
    outline_light: f32,     // backdrop darkening beside left/right edges
    lensing: f32,
    rim: f32,
    fringe: f32,
    shadow: f32,
    merge_k: f32,
    outline_dark: f32,
    output_gamma: f32,      // 1 when the target is not an sRGB format
    bevel: f32,             // logical px
    _pad1: f32,
    _pad2: f32,
    body_light: vec4f,      // offset, gain, curve, chroma
    body_dark: vec4f,
    shapes: array<vec4f, 16>, // center.xy, half_size.xy
    extra: array<vec4f, 16>,  // radius, appearance (0 light, 1 dark), unused, unused
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var backdrop: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}

fn spow(x: f32, y: f32) -> f32 {
    return pow(max(x, 1e-6), y);
}

fn sd_round_box(p: vec2f, b: vec2f, r_in: f32) -> f32 {
    let r = min(r_in, min(b.x, b.y));
    let q = abs(p) - b + vec2f(r);
    return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// Smooth minimum: blends two distance fields so nearby shapes merge.
fn smin(a: f32, b: f32, k: f32) -> f32 {
    if (k <= 0.0) {
        return min(a, b);
    }
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

fn scene(p: vec2f) -> f32 {
    var d = 1e5;
    for (var i = 0u; i < u.shape_count; i++) {
        let s = u.shapes[i];
        d = smin(d, sd_round_box(p - s.xy, s.zw, u.extra[i].x), u.merge_k);
    }
    return d;
}

// Appearance blended by distance, so merged shapes share one tint smoothly.
fn appearance_at(p: vec2f) -> f32 {
    var a = 0.0;
    var ws = 0.0;
    for (var i = 0u; i < u.shape_count; i++) {
        let s = u.shapes[i];
        let di = sd_round_box(p - s.xy, s.zw, u.extra[i].x);
        let w = exp(-max(di, 0.0) / 16.0);
        a += w * u.extra[i].y;
        ws += w;
    }
    return a / max(ws, 1e-4);
}

fn backdrop_linear(css: vec2f, lod: f32) -> vec3f {
    let texel = vec2f(css.x * u.backdrop_scale, css.y * u.backdrop_scale + u.scroll);
    return textureSampleLevel(backdrop, samp, texel / u.backdrop_size, lod).rgb;
}

fn to_gamma(c: vec3f) -> vec3f {
    return pow(max(c, vec3f(1e-6)), vec3f(1.0 / 2.2));
}

const FROST_TAPS: u32 = 24u;

// Gaussian frost: golden-angle taps whose radii follow the Gaussian's radial
// distribution, so equal weights give a Gaussian. Each tap reads a mip level
// that already averages the gaps between taps. Taps are averaged gamma-encoded.
fn frost(p: vec2f, sigma: f32) -> vec3f {
    if (sigma < 0.25) {
        return to_gamma(backdrop_linear(p, 0.0));
    }
    let lod = max(log2(sigma * u.backdrop_scale * 0.5), 0.0);
    var acc = vec3f(0.0);
    for (var i = 0u; i < FROST_TAPS; i++) {
        let f = f32(i);
        let a = f * 2.39996323;
        let rr = sigma * sqrt(-2.0 * log(1.0 - (f + 0.5) / f32(FROST_TAPS)));
        acc += to_gamma(backdrop_linear(p + vec2f(cos(a), sin(a)) * rr, lod));
    }
    return acc / f32(FROST_TAPS);
}

fn finish(c: vec3f) -> vec4f {
    if (u.output_gamma > 0.5) {
        return vec4f(c, 1.0);
    }
    return vec4f(pow(max(c, vec3f(0.0)), vec3f(2.2)), 1.0);
}

@fragment
fn fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    let p = pos.xy / u.scale_factor;
    let d = scene(p);

    var bg = to_gamma(backdrop_linear(p, 0.0));
    if (u.shadow > 0.0) {
        let ds = scene(p - vec2f(0.0, 6.0));
        var sh = exp(-max(ds, 0.0) / 14.0);
        if (ds < 0.0) {
            sh = 1.0;
        }
        bg = bg * (1.0 - sh * 0.22 * u.shadow);
    }

    // Coverage of the shape, and of the shape grown by one device pixel.
    let cov = clamp(0.5 - d * u.scale_factor, 0.0, 1.0);
    let outer = clamp(1.5 - d * u.scale_factor, 0.0, 1.0);
    let outline = max(u.outline_light, u.outline_dark);
    if (outer <= 0.0 || (cov <= 0.0 && outline <= 0.0)) {
        return finish(bg);
    }

    // Outward surface normal from the distance field.
    let e = vec2f(0.75, 0.0);
    let g = vec2f(scene(p + e.xy) - scene(p - e.xy), scene(p + e.yx) - scene(p - e.yx));
    var n = vec2f(0.0, -1.0);
    if (length(g) > 1e-5) {
        n = normalize(g);
    }

    // Lensing: near the rim, sample further inward, like a convex drop.
    let edge = 1.0 - clamp(-d / u.bevel, 0.0, 1.0);
    let disp = -n * spow(edge, 2.2) * u.bevel * 0.9 * u.lensing;

    var col = frost(p + disp, u.blur);
    if (u.fringe > 0.0 && edge > 0.03) {
        let f = 0.12 * u.fringe;
        col.r = frost(p + disp * (1.0 + f), u.blur).r;
        col.b = frost(p + disp * (1.0 - f), u.blur).b;
    }

    // Body: a tone curve on luminance plus a chroma gain (fitted to macOS).
    let app = appearance_at(p);
    let body = mix(u.body_light, u.body_dark, app);
    let luma = dot(col, vec3f(0.2126, 0.7152, 0.0722));
    col = vec3f(body.x + body.y * luma + body.z * luma * luma) + body.w * (col - vec3f(luma));

    // Optional rim light from the top left, with a weaker bounce opposite.
    if (u.rim > 0.0) {
        let ld = dot(n, normalize(vec2f(-0.55, -0.85)));
        let rim = 1.0 - smoothstep(0.0, 1.6, -d);
        let spec = spow(max(ld, 0.0), 1.5) * 0.85 + spow(max(-ld, 0.0), 1.5) * 0.35;
        col = mix(col, vec3f(1.0), rim * spec * u.rim * mix(1.0, 0.7, app));
        col += vec3f(spow(edge, 3.0) * 0.10 * u.rim * (0.4 + 0.6 * max(ld, 0.0)));
    }
    col = clamp(col, vec3f(0.0), vec3f(1.0));

    // Outline: macOS darkens the backdrop in the pixel just outside the shape;
    // top and bottom get two thirds of the left and right sides' darkening.
    var out = bg;
    if (outline > 0.0) {
        let darken = mix(u.outline_light, u.outline_dark, app) * mix(0.66, 1.0, abs(n.x));
        out = mix(bg, bg * (1.0 - darken), outer);
    }
    return finish(mix(out, col, cov));
}
