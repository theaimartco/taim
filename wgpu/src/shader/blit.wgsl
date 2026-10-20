// TAIM immutable fullscreen-triangle pair. A constant LUT avoids mutable
// shader-private arrays, and index clamping protects malformed draw calls.
const TAIM_BLIT_UVS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0)
);

@group(0) @binding(0) var u_sampler: sampler;
@group(0) @binding(1) var<uniform> u_ratio: vec4<f32>;
@group(1) @binding(0) var u_texture: texture_2d<f32>;

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let uv = TAIM_BLIT_UVS[min(input.vertex_index, 5u)];

    var out: VertexOutput;
    out.uv = uv * u_ratio.xy;
    out.position = vec4<f32>(uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);

    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(u_texture, u_sampler, input.uv);
}
