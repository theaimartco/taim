// Deferred G-Buffer Lighting Pass (Pillar III)
//
// Evaluates physically-based materials, slab refraction with chromatic dispersion,
// and GGX specular highlights across the 3-target G-buffer.

const IOR_AIR: f32 = 1.0;
const MIN_ROUGHNESS: f32 = 0.03;
const PI: f32 = 3.14159265358979323846;

struct LightUniforms {
    light_dir: vec3<f32>,
    pixels_per_mm: f32,
    light_color: vec3<f32>,
    light_intensity: f32,
    viewport_width: f32,
    viewport_height: f32,
    pad0: f32,
    pad1: f32,
}

struct GpuMaterial {
    roughness: f32,
    thickness_mm: f32,
    ior: f32,
    dispersion: f32,
    transmission: f32,
    pad0: f32,
    pad1: f32,
    pad2: f32,
}

@group(0) @binding(0) var<uniform> uniforms: LightUniforms;
@group(0) @binding(1) var<storage, read> material_table: array<GpuMaterial>;
@group(0) @binding(2) var rt0_tex: texture_2d<f32>; // Emissive + Coverage
@group(0) @binding(3) var rt1_tex: texture_2d<f32>; // Octahedral Normal + Roughness + Thickness
@group(0) @binding(4) var rt2_tex: texture_2d<u32>; // Material ID
@group(0) @binding(5) var backdrop_tex: texture_2d<f32>; // Backdrop
@group(0) @binding(6) var backdrop_sampler: sampler;

// Octahedral normal decoding from 2D parameterization.
fn decode_octahedral(e: vec2<f32>) -> vec3<f32> {
    var v = vec3<f32>(e.x, e.y, 1.0 - abs(e.x) - abs(e.y));
    if (v.z < 0.0) {
        let s = vec2<f32>(select(-1.0, 1.0, v.x >= 0.0), select(-1.0, 1.0, v.y >= 0.0));
        let flipped = (vec2<f32>(1.0, 1.0) - abs(v.yx)) * s;
        v.x = flipped.x;
        v.y = flipped.y;
    }
    return normalize(v);
}

fn fresnel_schlick(cos_theta: f32, f0: f32) -> f32 {
    let cosine = clamp(cos_theta, 0.0, 1.0);
    return f0 + (1.0 - f0) * pow(1.0 - cosine, 5.0);
}

fn ggx_specular(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let h = normalize(v + l);
    let n_dot_h = max(dot(n, h), 0.0);
    let alpha = max(roughness, MIN_ROUGHNESS);
    let alpha2 = alpha * alpha;

    let d = n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0;
    return alpha2 / max(PI * d * d, 1e-6);
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Fullscreen triangle: covers [-1, 1] x [-1, 1]
    let x = f32(i32(vertex_index << 1u) & 2) * 2.0 - 1.0;
    let y = f32(i32(vertex_index & 2u) & 2) * 2.0 - 1.0;

    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let coord = vec2<i32>(i32(in.position.x), i32(in.position.y));

    let rt0 = textureLoad(rt0_tex, coord, 0);
    let rt1 = textureLoad(rt1_tex, coord, 0);
    let rt2 = textureLoad(rt2_tex, coord, 0);

    let coverage = rt0.a;
    if (coverage <= 0.0) {
        return textureSample(backdrop_tex, backdrop_sampler, in.uv);
    }

    let emissive = rt0.rgb;
    let normal = decode_octahedral(rt1.rg);
    let roughness = max(rt1.b, MIN_ROUGHNESS);
    let thickness_mm = rt1.a;
    let material_id = rt2.r;

    let mat = material_table[material_id];
    let view_dir = vec3<f32>(0.0, 0.0, 1.0);

    // Slab refraction offset
    let normal_proj = normal.xy;
    let offset_uv = normal_proj * (thickness_mm * (1.0 - 1.0 / max(mat.ior, 1.0)) * uniforms.pixels_per_mm)
        / vec2<f32>(uniforms.viewport_width, uniforms.viewport_height);

    // Chromatic dispersion
    let disp_uv = offset_uv * (mat.dispersion * 0.5);
    let sample_r = textureSample(backdrop_tex, backdrop_sampler, in.uv + offset_uv + disp_uv).r;
    let sample_g = textureSample(backdrop_tex, backdrop_sampler, in.uv + offset_uv).g;
    let sample_b = textureSample(backdrop_tex, backdrop_sampler, in.uv + offset_uv - disp_uv).b;
    let refracted_backdrop = vec3<f32>(sample_r, sample_g, sample_b);

    // GGX specular with SceneLight
    let spec = ggx_specular(normal, view_dir, uniforms.light_dir, roughness);
    let f0 = pow((mat.ior - IOR_AIR) / (mat.ior + IOR_AIR), 2.0);
    let fresnel = fresnel_schlick(max(dot(normal, view_dir), 0.0), f0);

    let light_contribution = uniforms.light_color * (spec * uniforms.light_intensity);
    let lit_surface = mix(refracted_backdrop, light_contribution, fresnel) + emissive;

    let orig_backdrop = textureSample(backdrop_tex, backdrop_sampler, in.uv).rgb;
    let blended = mix(orig_backdrop, lit_surface, coverage);

    return vec4<f32>(blended, 1.0);
}
