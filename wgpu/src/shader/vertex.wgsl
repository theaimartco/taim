// Immutable quad positions without global array constructors. Some GLES
// compilers reject the struct-wrapped array generated for a WGSL constant LUT.
fn vertex_position(vertex_index: u32) -> vec2<f32> {
    let index = min(vertex_index, 5u);
    return vec2<f32>(
        select(0.0, 1.0, index == 0u || index == 1u || index == 5u),
        select(0.0, 1.0, index == 0u || index == 4u || index == 5u),
    );
}
