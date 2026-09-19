@group(0) @binding(0) var source: texture_2d<f32>;

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    // One oversized triangle covers the target without the diagonal seam or
    // three redundant vertex shader invocations of a two-triangle quad.
    let position = vec2<f32>(
        f32((input.vertex_index << 1u) & 2u),
        f32(input.vertex_index & 2u),
    );

    var out: VertexOutput;
    out.position = vec4<f32>(
        position * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0),
        0.0,
        1.0,
    );

    return out;
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(source, vec2<i32>(position.xy), 0);
}
