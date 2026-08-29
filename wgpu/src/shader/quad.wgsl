struct Globals {
    transform: mat4x4<f32>,
    scale: f32,
    // The scene's key light, normalised, in `xyz`.
    //
    // `vec4` rather than `vec3`: WGSL aligns both to 16 bytes, so declaring a
    // `vec3` makes the host and the shader disagree about the struct's size.
    light: vec4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;

fn rounded_box_sdf(p: vec2<f32>, size: vec2<f32>, corners: vec4<f32>) -> f32 {
    var box_half = select(corners.yz, corners.xw, p.x > 0.0);
    var corner = select(box_half.y, box_half.x, p.y > 0.0);
    var q = abs(p) - size + corner;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2(0.0))) - corner;
}

// The gradient of `rounded_box_sdf`, which is its surface normal in 2D.
//
// Analytic rather than sampled: the signed distance of a rounded rectangle has
// a closed form, so its gradient does too, and a bevelled edge costs no extra
// SDF evaluations and no geometry.
//
// Outside the corner core the nearest point is on the rounding arc, so the
// gradient points along `q`. Inside it the nearest edge is the dominant axis.
// `sign(p)` undoes the `abs` that folded the rectangle into one quadrant.
fn rounded_box_gradient(p: vec2<f32>, size: vec2<f32>, corners: vec4<f32>) -> vec2<f32> {
    var box_half = select(corners.yz, corners.xw, p.x > 0.0);
    var corner = select(box_half.y, box_half.x, p.y > 0.0);
    var q = abs(p) - size + corner;

    var gradient: vec2<f32>;

    if max(q.x, q.y) > 0.0 {
        // `max(q, 0)` is non-zero here, so `normalize` is safe.
        gradient = normalize(max(q, vec2<f32>(0.0)));
    } else {
        gradient = select(vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), q.x > q.y);
    }

    return gradient * sign(p);
}
