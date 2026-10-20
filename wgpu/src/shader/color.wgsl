// TAIM enforces a bounded coverage channel before alpha compositing.
// HDR RGB values remain unbounded: only opacity is normalized.
fn premultiply(color: vec4<f32>) -> vec4<f32> {
    let alpha = clamp(color.a, 0.0, 1.0);
    return vec4<f32>(color.xyz * alpha, alpha);
}

fn unpack_color(data: vec2<u32>) -> vec4<f32> {
    return premultiply(unpack_u32(data));
}

fn unpack_u32(data: vec2<u32>) -> vec4<f32> {
    let rg: vec2<f32> = unpack2x16float(data.x);
    let ba: vec2<f32> = unpack2x16float(data.y);

    return vec4<f32>(rg.y, rg.x, ba.y, ba.x);
}
