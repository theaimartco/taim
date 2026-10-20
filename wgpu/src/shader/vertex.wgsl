// TAIM immutable quad triangles for deterministic WebGPU vertex dispatch.
// Explicit positions avoid vector integer div/mod in every vertex invocation.
const TAIM_QUAD_VERTICES: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(1.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 0.0),
    vec2<f32>(0.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0)
);

fn vertex_position(vertex_index: u32) -> vec2<f32> {
    // Triangle draw calls emit six vertices. Defensively clamp a bad index
    // instead of reading outside the static vertex array.
    return TAIM_QUAD_VERTICES[min(vertex_index, 5u)];
}
