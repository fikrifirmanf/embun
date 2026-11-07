// Glass pass: one fullscreen triangle. Pixels outside every shape show the
// backdrop; pixels inside are frosted, lensed and recolored, and a one-pixel
// outline sits just outside the shape. Frost and color math run on
// sRGB-encoded values, which fits the macOS 27 captures better than linear.

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
    blur_lod: f32,          // mip level that holds the blurred backdrop
    _pad1: f32,
    body_light: vec4f,      // offset, gain, curve, chroma
    body_dark: vec4f,
    sharp: vec4f,           // sharp gain, sharp chroma: light .xy, dark .zw
    edges: vec4f,           // outline at the ends (light, dark), highlight (light, dark)
    shapes: array<vec4f, 16>, // center.xy, half_size.xy
    extra: array<vec4f, 16>,  // radius, appearance (0 light, 1 dark), unused, unused
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var backdrop: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var blurred: texture_2d<f32>; // gamma-encoded, at mip blur_lod

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}

fn spow(x: f32, y: f32) -> f32 {
    return pow(max(x, 1e-6), y);
}

// Apple's continuous corners, fitted to macOS 27 captures: the corner curve
// starts CORNER_REACH times the radius from the corner and follows a
// superellipse with exponent CORNER_POWER. When the radius approaches half the
// shorter side (capsules, circles) it blends back to a circular arc, as macOS
// does.
const CORNER_REACH: f32 = 1.24;
const CORNER_POWER: f32 = 2.6;

fn sd_round_box(p: vec2f, b: vec2f, r_in: f32) -> f32 {
    let half = min(b.x, b.y);
    let r = min(r_in, half);
    let t = clamp((half - r) / max((CORNER_REACH - 1.0) * r, 1e-4), 0.0, 1.0);
    let a = r * mix(1.0, CORNER_REACH, t);
    let n = mix(2.0, CORNER_POWER, t);
    let q = abs(p) - b + vec2f(a);
    if (q.x > 0.0 && q.y > 0.0) {
        // Distance to the superellipse, corrected by the gradient length.
        let m = pow(pow(q.x, n) + pow(q.y, n), 1.0 / n);
        let grad = pow(q / m, vec2f(n - 1.0));
        return (m - a) / max(length(grad), 1e-4);
    }
    return max(q.x, q.y) - a;
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

// Exact sRGB transfer functions. The body was fitted to sRGB-encoded PNG
// values, so the shader must encode the same way, especially in dark tones.
fn to_gamma(c: vec3f) -> vec3f {
    let x = max(c, vec3f(0.0));
    return select(1.055 * pow(x, vec3f(1.0 / 2.4)) - 0.055, 12.92 * x, x <= vec3f(0.0031308));
}

fn to_linear(c: vec3f) -> vec3f {
    let x = max(c, vec3f(0.0));
    return select(pow((x + 0.055) / 1.055, vec3f(2.4)), x / 12.92, x <= vec3f(0.04045));
}

// Frost: the backdrop blurred by a separable Gaussian before this pass.
fn frost(p: vec2f) -> vec3f {
    if (u.blur < 0.25) {
        return to_gamma(backdrop_linear(p, 0.0));
    }
    let texel = vec2f(p.x * u.backdrop_scale, p.y * u.backdrop_scale + u.scroll);
    return textureSampleLevel(blurred, samp, texel / u.backdrop_size, u.blur_lod).rgb;
}

fn finish(c: vec3f) -> vec4f {
    if (u.output_gamma > 0.5) {
        return vec4f(c, 1.0);
    }
    return vec4f(to_linear(c), 1.0);
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

    // Far outside every shape: just the backdrop.
    if (d * u.scale_factor > 3.0) {
        return finish(bg);
    }

    // Surface gradient of the distance field. Its length is 1 along simple
    // edges and falls below 1 where shapes merge (toward 0 in the middle of a
    // neck); `n` is the normalized outward normal.
    let e = vec2f(0.75, 0.0);
    let grad = vec2f(scene(p + e.xy) - scene(p - e.xy), scene(p + e.yx) - scene(p - e.yx)) / (2.0 * e.x);
    var n = vec2f(0.0, -1.0);
    if (length(grad) > 1e-5) {
        n = normalize(grad);
    }

    // Coverage of the shape, and of the shape grown by one device pixel. The
    // distance is divided by the gradient length so merged shapes keep a
    // one-pixel edge and outline.
    let dp = d * u.scale_factor / max(length(grad), 0.25);
    let cov = clamp(0.5 - dp, 0.0, 1.0);
    let outer = clamp(1.5 - dp, 0.0, 1.0);
    let outline = max(max(u.outline_light, u.outline_dark), max(u.edges.x, u.edges.y));
    if (outer <= 0.0 || (cov <= 0.0 && outline <= 0.0)) {
        return finish(bg);
    }

    // Lensing: near the rim, sample further inward, like a convex drop. Using
    // the unnormalized gradient keeps the shift continuous across a neck,
    // instead of flipping sides at its center.
    let edge = 1.0 - clamp(-d / u.bevel, 0.0, 1.0);
    let disp = -grad * spow(edge, 2.2) * u.bevel * 0.9 * u.lensing;

    var col = frost(p + disp);
    if (u.fringe > 0.0 && edge > 0.03) {
        let f = 0.12 * u.fringe;
        col.r = frost(p + disp * (1.0 + f)).r;
        col.b = frost(p + disp * (1.0 - f)).b;
    }

    // Body: a tone curve on the frosted luminance plus a chroma gain, and a
    // share of the unblurred backdrop (fitted to macOS).
    let app = appearance_at(p);
    let body = mix(u.body_light, u.body_dark, app);
    let sharp_mix = mix(u.sharp.xy, u.sharp.zw, app);
    let to_luma = vec3f(0.2126, 0.7152, 0.0722);
    let luma = dot(col, to_luma);
    col = vec3f(body.x + body.y * luma + body.z * luma * luma) + body.w * (col - vec3f(luma));
    if (sharp_mix.x != 0.0 || sharp_mix.y != 0.0) {
        let s = to_gamma(backdrop_linear(p + disp, 0.0));
        let ls = dot(s, to_luma);
        col += vec3f(sharp_mix.x * ls) + sharp_mix.y * (s - vec3f(ls));
    }

    // Highlight along the inside of the top and bottom edges, fading over
    // about two device pixels (a Gaussian fitted to macOS).
    let highlight = mix(u.edges.z, u.edges.w, app);
    if (highlight > 0.0) {
        let depth = max(-dp, 0.0);
        col += vec3f(highlight * exp(-depth * depth / 3.44) * n.y * n.y);
    }

    // Optional rim light from the top left, with a weaker bounce opposite.
    if (u.rim > 0.0) {
        let ld = dot(n, normalize(vec2f(-0.55, -0.85)));
        let rim = 1.0 - smoothstep(0.0, 1.6, -d);
        let spec = spow(max(ld, 0.0), 1.5) * 0.85 + spow(max(-ld, 0.0), 1.5) * 0.35;
        col = mix(col, vec3f(1.0), rim * spec * u.rim * mix(1.0, 0.7, app));
        col += vec3f(spow(edge, 3.0) * 0.10 * u.rim * (0.4 + 0.6 * max(ld, 0.0)));
    }
    col = clamp(col, vec3f(0.0), vec3f(1.0));

    // Outline: macOS darkens the backdrop in the pixel just outside the shape,
    // with one strength beside the sides and another above and below.
    var out = bg;
    if (outline > 0.0) {
        let side = mix(u.outline_light, u.outline_dark, app);
        let ends = mix(u.edges.x, u.edges.y, app);
        let darken = mix(ends, side, abs(n.x));
        out = mix(bg, bg * (1.0 - darken), outer);
    }
    return finish(mix(out, col, cov));
}
