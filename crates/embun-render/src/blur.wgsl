// One direction of a separable Gaussian blur, run at a single mip level.
// The first pass reads the sRGB backdrop and converts each tap back to sRGB
// encoding; the second reads the first pass's encoded output. Blurring
// encoded values matches the macOS 27 captures better than linear.

struct BlurUniforms {
    step: vec2f,     // one texel along the blur direction, in uv
    sigma: f32,      // in texels of this mip level
    to_gamma: f32,   // 1 when the source is linear (the sRGB backdrop)
};

struct VOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
};

@group(0) @binding(0) var<uniform> b: BlurUniforms;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

/// Taps on each side are capped; the renderer picks a mip level that keeps
/// three sigmas under this.
const MAX_TAPS: u32 = 32u;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VOut {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    var o: VOut;
    o.pos = vec4f(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2f(p.x, 1.0 - p.y);
    return o;
}

fn fetch(uv: vec2f) -> vec3f {
    let c = textureSampleLevel(src, samp, uv, 0.0).rgb;
    if (b.to_gamma > 0.5) {
        let x = max(c, vec3f(0.0));
        return select(1.055 * pow(x, vec3f(1.0 / 2.4)) - 0.055, 12.92 * x, x <= vec3f(0.0031308));
    }
    return c;
}

@fragment
fn fs(v: VOut) -> @location(0) vec4f {
    let taps = min(u32(ceil(b.sigma * 3.0)), MAX_TAPS);
    let k = -0.5 / max(b.sigma * b.sigma, 1e-4);
    var acc = fetch(v.uv);
    var total = 1.0;
    for (var i = 1u; i <= taps; i++) {
        let x = f32(i);
        let w = exp(k * x * x);
        acc += w * (fetch(v.uv + b.step * x) + fetch(v.uv - b.step * x));
        total += 2.0 * w;
    }
    return vec4f(acc / total, 1.0);
}
