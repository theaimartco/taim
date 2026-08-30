// The display transform: scene-referred light in, panel signal out.
//
// This is a line-by-line mirror of `taim_core::color::display`. The two
// implementations must agree, because one of them decides what a screenshot
// looks like and the other decides what a person looks at, and a divergence
// between them is invisible until someone photographs a panel.
// `tests/gpu/display_transform.rs` pins them together.

struct Uniforms {
    // Compositing primaries -> display primaries. Rows 0..2, each in .xyz.
    conversion_0: vec4<f32>,
    conversion_1: vec4<f32>,
    conversion_2: vec4<f32>,
    // .xyz: luminance vector of the compositing primaries. .w: source peak, nits.
    compositing: vec4<f32>,
    // .xyz: luminance vector of the display primaries. .w: display peak, nits.
    display: vec4<f32>,
    // x: display black, nits. y: reference white, nits. z: diffuse white, nits.
    levels: vec4<f32>,
    // x: tone map (0 BT.2390, 1 clip). y: gamut map (0 desaturate, 1 clip).
    // z: transfer (0 linear, 1 sRGB, 2 PQ, 3 HLG).
    modes: vec4<f32>,
}

@group(0) @binding(0) var u_sampler: sampler;
@group(0) @binding(1) var<uniform> u: Uniforms;
@group(1) @binding(0) var u_texture: texture_2d<f32>;

// SMPTE ST 2084, table 4.
const PQ_M1: f32 = 0.1593017578125;
const PQ_M2: f32 = 78.84375;
const PQ_C1: f32 = 0.8359375;
const PQ_C2: f32 = 18.8515625;
const PQ_C3: f32 = 18.6875;
const PQ_PEAK_NITS: f32 = 10000.0;

// ITU-R BT.2100, table 5.
const HLG_A: f32 = 0.17883277;
const HLG_B: f32 = 0.28466892;
const HLG_C: f32 = 0.5599107;

const TONE_MAP_CLIP: f32 = 1.0;
const GAMUT_MAP_CLIP: f32 = 1.0;

const TRANSFER_SRGB: f32 = 1.0;
const TRANSFER_PQ: f32 = 2.0;
const TRANSFER_HLG: f32 = 3.0;

fn pq_encode(linear: f32) -> f32 {
    let powed = pow(max(linear, 0.0), PQ_M1);

    return pow((PQ_C1 + PQ_C2 * powed) / (1.0 + PQ_C3 * powed), PQ_M2);
}

fn pq_decode(encoded: f32) -> f32 {
    let clamped = clamp(encoded, 0.0, 1.0);
    let powed = pow(clamped, 1.0 / PQ_M2);
    let numerator = max(powed - PQ_C1, 0.0);
    let denominator = PQ_C2 - PQ_C3 * powed;

    if denominator <= 0.0 {
        return 0.0;
    }

    return pow(numerator / denominator, 1.0 / PQ_M1);
}

fn pq_from_nits(nits: f32) -> f32 {
    return pq_encode(nits / PQ_PEAK_NITS);
}

fn nits_from_pq(signal: f32) -> f32 {
    return pq_decode(signal) * PQ_PEAK_NITS;
}

fn srgb_encode(linear: f32) -> f32 {
    if linear <= 0.0031308 {
        return 12.92 * linear;
    }

    return 1.055 * pow(max(linear, 0.0), 1.0 / 2.4) - 0.055;
}

fn hlg_encode(linear: f32) -> f32 {
    let clamped = max(linear, 0.0);

    if clamped <= 1.0 / 12.0 {
        return sqrt(3.0 * clamped);
    }

    return HLG_A * log(12.0 * clamped - HLG_B) + HLG_C;
}

fn scene_to_nits(scene: f32) -> f32 {
    return scene * u.levels.z;
}

fn nits_to_scene(nits: f32) -> f32 {
    return nits / u.levels.z;
}

// ITU-R BT.2390's electro-optical transfer function adjustment.
fn eetf(nits: f32) -> f32 {
    let display_peak = u.display.w;

    let source_black = pq_from_nits(0.0);
    let source_peak = pq_from_nits(u.compositing.w);
    let range = source_peak - source_black;

    if range <= 1.1920929e-7 {
        return min(nits, display_peak);
    }

    let max_luminance = (pq_from_nits(display_peak) - source_black) / range;
    let min_luminance = (pq_from_nits(u.levels.x) - source_black) / range;
    let normalized = (pq_from_nits(nits) - source_black) / range;

    let knee = 1.5 * max_luminance - 0.5;

    var rolled = normalized;

    if normalized >= knee && knee < 1.0 {
        let t = (normalized - knee) / (1.0 - knee);
        let t2 = t * t;
        let t3 = t2 * t;

        rolled = (2.0 * t3 - 3.0 * t2 + 1.0) * knee
            + (t3 - 2.0 * t2 + t) * (1.0 - knee)
            + (-2.0 * t3 + 3.0 * t2) * max_luminance;
    }

    let lifted = rolled + min_luminance * pow(1.0 - rolled, 4.0);

    return clamp(nits_from_pq(lifted * range + source_black), 0.0, display_peak);
}

fn tone_map(scene: vec3<f32>) -> vec3<f32> {
    let luminance = dot(u.compositing.xyz, scene);

    if luminance <= 0.0 {
        return scene;
    }

    let nits = scene_to_nits(luminance);

    var mapped = eetf(nits);

    if u.modes.x == TONE_MAP_CLIP {
        mapped = min(nits, u.display.w);
    }

    // Scale the whole triple rather than compressing per channel, which would
    // desaturate bright colors toward white.
    return scene * (nits_to_scene(mapped) / luminance);
}

fn desaturate_into(color: vec3<f32>, ceiling: f32) -> vec3<f32> {
    if min(color.r, min(color.g, color.b)) >= 0.0
        && max(color.r, max(color.g, color.b)) <= ceiling {
        return color;
    }

    let grey = clamp(dot(u.display.xyz, color), 0.0, ceiling);
    let offset = color - vec3<f32>(grey);

    var factor = 1.0;

    for (var axis = 0; axis < 3; axis += 1) {
        let delta = offset[axis];

        if abs(delta) <= 1.1920929e-7 {
            continue;
        }

        var bound = -grey / delta;

        if delta > 0.0 {
            bound = (ceiling - grey) / delta;
        }

        factor = min(factor, max(bound, 0.0));
    }

    return clamp(
        vec3<f32>(grey) + offset * factor,
        vec3<f32>(0.0),
        vec3<f32>(ceiling)
    );
}

fn gamut_map(scene: vec3<f32>) -> vec3<f32> {
    let converted = vec3<f32>(
        dot(u.conversion_0.xyz, scene),
        dot(u.conversion_1.xyz, scene),
        dot(u.conversion_2.xyz, scene),
    );

    let ceiling = nits_to_scene(u.display.w);

    if u.modes.y == GAMUT_MAP_CLIP {
        return clamp(converted, vec3<f32>(0.0), vec3<f32>(ceiling));
    }

    return desaturate_into(converted, ceiling);
}

fn encode(component: f32) -> f32 {
    let nits = scene_to_nits(component);

    if u.modes.z == TRANSFER_PQ {
        return pq_encode(nits / PQ_PEAK_NITS);
    }

    let relative = nits / max(u.levels.y, 1.1754944e-38);

    if u.modes.z == TRANSFER_SRGB {
        return srgb_encode(relative);
    }

    if u.modes.z == TRANSFER_HLG {
        return hlg_encode(relative);
    }

    return relative;
}

var<private> corners: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0),
);

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let corner = corners[index];

    var out: VertexOutput;
    out.uv = corner;
    out.position = vec4<f32>(
        corner * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0),
        0.0,
        1.0
    );

    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let scene = textureSample(u_texture, u_sampler, input.uv);

    let mapped = gamut_map(tone_map(scene.rgb));

    return vec4<f32>(
        encode(mapped.r),
        encode(mapped.g),
        encode(mapped.b),
        scene.a
    );
}
