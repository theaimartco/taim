// TAIM bounded gradient interpolation. A stop factor outside [0,1]
// must not extrapolate color past the selected endpoint.
fn interpolate_color(from_: vec4<f32>, to_: vec4<f32>, factor: f32) -> vec4<f32> {
    let progress = clamp(factor, 0.0, 1.0);
    return mix(from_, to_, progress);
}
