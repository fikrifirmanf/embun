// Rounded rectangles and images, in physical pixels. Fills are a vertical
// gradient (top to bottom color), with an optional inner border. Output is
// premultiplied alpha.

struct Globals {
    size: vec2f,          // target size in physical px
    output_gamma: f32,    // 1 when the target is not an sRGB format
    _pad: f32,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

struct Instance {
    @location(0) rect: vec4f,    // x, y, w, h
    @location(1) params: vec4f,  // radius, border width, unused, unused
    @location(2) top: vec4f,     // linear, premultiplied
    @location(3) bottom: vec4f,
    @location(4) border: vec4f,
    @location(5) clip: vec4f,    // x, y, w, h
};

struct Varyings {
    @builtin(position) pos: vec4f,
    @location(0) p: vec2f,
    @location(1) @interpolate(flat) rect: vec4f,
    @location(2) @interpolate(flat) params: vec4f,
    @location(3) @interpolate(flat) top: vec4f,
    @location(4) @interpolate(flat) bottom: vec4f,
    @location(5) @interpolate(flat) border: vec4f,
    @location(6) @interpolate(flat) clip: vec4f,
};

@vertex
fn vs(@builtin(vertex_index) v: u32, inst: Instance) -> Varyings {
    let corners = array(vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0), vec2f(0.0, 1.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0));
    // One pixel of room for the antialiased edge.
    let lo = inst.rect.xy - vec2f(1.0);
    let hi = inst.rect.xy + inst.rect.zw + vec2f(1.0);
    let p = mix(lo, hi, corners[v]);
    var out: Varyings;
    out.pos = vec4f(p.x / g.size.x * 2.0 - 1.0, 1.0 - p.y / g.size.y * 2.0, 0.0, 1.0);
    out.p = p;
    out.rect = inst.rect;
    out.params = inst.params;
    out.top = inst.top;
    out.bottom = inst.bottom;
    out.border = inst.border;
    out.clip = inst.clip;
    return out;
}

fn sd_round_rect(p: vec2f, half: vec2f, r: f32) -> f32 {
    let q = abs(p) - half + vec2f(r);
    return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

fn to_gamma(c: vec3f) -> vec3f {
    let x = max(c, vec3f(0.0));
    return select(1.055 * pow(x, vec3f(1.0 / 2.4)) - 0.055, 12.92 * x, x <= vec3f(0.0031308));
}

// Coverage of the rounded rect at this pixel, cut by the clip rect.
fn coverage(in: Varyings) -> f32 {
    let c = in.clip;
    if (in.p.x < c.x || in.p.y < c.y || in.p.x > c.x + c.z || in.p.y > c.y + c.w) {
        return 0.0;
    }
    let half = in.rect.zw * 0.5;
    let r = min(in.params.x, min(half.x, half.y));
    let d = sd_round_rect(in.p - in.rect.xy - half, half, r);
    return clamp(0.5 - d, 0.0, 1.0);
}

fn finish(c: vec4f) -> vec4f {
    if (g.output_gamma > 0.0 && c.a > 0.0) {
        return vec4f(to_gamma(c.rgb / c.a) * c.a, c.a);
    }
    return c;
}

@fragment
fn fs_rect(in: Varyings) -> @location(0) vec4f {
    let cov = coverage(in);
    if (cov <= 0.0) {
        discard;
    }
    let t = clamp((in.p.y - in.rect.y) / max(in.rect.w, 1.0), 0.0, 1.0);
    var color = mix(in.top, in.bottom, t);
    let bw = in.params.y;
    if (bw > 0.0) {
        let half = in.rect.zw * 0.5;
        let r = min(in.params.x, min(half.x, half.y));
        let d = sd_round_rect(in.p - in.rect.xy - half, half, r);
        // 1 within `bw` of the edge, fading over a pixel inward.
        let ring = clamp(d + bw + 0.5, 0.0, 1.0);
        color = mix(color, in.border, ring);
    }
    return finish(color * cov);
}

@fragment
fn fs_image(in: Varyings) -> @location(0) vec4f {
    let cov = coverage(in);
    if (cov <= 0.0) {
        discard;
    }
    let uv = (in.p - in.rect.xy) / in.rect.zw;
    let c = textureSampleLevel(image, image_sampler, uv, 0.0);
    return finish(vec4f(c.rgb * c.a, c.a) * cov);
}
