// Shared TAIM gradient sampler. Packed stop slots above 1.0 are inactive.
fn taim_gradient_noise(pixel: vec2<f32>) -> f32 {
    let interleaved = fract(52.9829189 * fract(dot(pixel, vec2<f32>(0.06711056, 0.00583715))));
    return (interleaved - 0.5) * (0.6 / 255.0);
}

fn taim_gradient_sample(
    point: vec2<f32>,
    endpoints: vec4<f32>,
    packed_colors: array<vec4<f32>, 8>,
    packed_offsets: array<f32, 8>,
) -> vec4<f32> {
    // Mutable locals are required for dynamic array indexing on all WGPU
    // backends currently supported by TAIM.
    var colors = packed_colors;
    var offsets = packed_offsets;
    var last = 0;

    for (var slot = 1; slot < 8; slot++) {
        if offsets[slot] > 1.0 {
            break;
        }
        last = slot;
    }

    let axis = endpoints.zw - endpoints.xy;
    let denominator = max(dot(axis, axis), 1.1754944e-38);
    let progress = dot(point - endpoints.xy, axis) / denominator;
    let noise = taim_gradient_noise(point);

    if progress <= offsets[0] {
        return colors[0] + noise;
    }
    if progress >= offsets[last] {
        return colors[last] + noise;
    }

    for (var slot = 0; slot < last; slot++) {
        let next = slot + 1;
        if progress <= offsets[next] {
            let amount = smoothstep(offsets[slot], offsets[next], progress);
            return interpolate_color(colors[slot], colors[next], amount) + noise;
        }
    }

    return colors[last] + noise;
}
