// What is behind a transmissive quad.
//
// Bound at group 1 by every quad draw. A quad that does not transmit binds
// the pipeline's own 1x1 empty texture here and never samples it -- the
// fragment shader's `transmission <= 0.0` early-out returns first -- so the
// common opaque path costs one comparison and no texture read.
//
// This is a copy of the frame as it stood *before* the transmissive layer
// drew, taken by `Renderer::render`. It is the whole frame at frame
// resolution, so a fragment's own screen position indexes it directly.

@group(1) @binding(0) var backdrop_texture: texture_2d<f32>;
@group(1) @binding(1) var backdrop_sampler: sampler;

// Samples the backdrop at a screen position displaced by `offset_px`.
//
// Takes pixels rather than UV because everything upstream -- the SDF, the
// refraction offset, the quad's own scale -- is in pixels, and converting
// once here keeps the one division in one place.
fn sample_backdrop(position_px: vec2<f32>, offset_px: vec2<f32>) -> vec3<f32> {
    let size = vec2<f32>(textureDimensions(backdrop_texture));

    // A zero-sized backdrop means nothing was captured. Returning black
    // would punch a hole in the quad; returning the fragment's own colour is
    // the caller's job, so signal it by sampling nothing.
    if size.x <= 0.0 || size.y <= 0.0 {
        return vec3<f32>(0.0);
    }

    let uv = (position_px + offset_px) / size;

    return textureSampleLevel(backdrop_texture, backdrop_sampler, uv, 0.0).rgb;
}

// Whether a backdrop is actually bound, as opposed to the 1x1 placeholder.
fn has_backdrop() -> bool {
    return textureDimensions(backdrop_texture).x > 1u;
}

// What a transmissive quad shows of the scene behind it, per channel.
//
// Lives here rather than in `material.wgsl` because it samples a texture,
// and `material.wgsl` is compiled standalone against the CPU reference by
// `tests/gpu/material_shading.rs` -- the physics has to stay free of
// bindings for that to keep working. The arithmetic is
// `dispersed_refraction_offsets`; this is the sampling.
//
// `distance` and `gradient` come from the same rounded-rectangle signed
// distance field the quad is drawn with, so the normal is analytic and the
// bevel costs no geometry.
fn quad_transmission(
    distance: f32,
    gradient: vec2<f32>,
    packed: vec4<f32>,
    transmission: f32,
    position_px: vec2<f32>,
) -> vec3<f32> {
    let material = unpack_material(packed, transmission);

    let bevel = max(material.thickness_mm, 1.0);
    let normal = sdf_normal(distance, gradient, bevel);
    let view = vec3<f32>(0.0, 0.0, 1.0);

    // Head-on this is 1.0 and the offsets below are zero: a pane viewed
    // straight through displaces nothing, which is exactly why a blur
    // passes for glass in a screenshot and stops passing when it moves.
    let cos_theta = clamp_finite(dot(normal, view), 0.0, 1.0, 1.0);

    // The direction the surface tilts is the direction it bends what it
    // transmits. Zero in the flat interior, growing towards the bevel.
    let tilt = normal.xy;
    let tilt_length = length(tilt);

    if tilt_length <= 1.1754944e-38 {
        return sample_backdrop(position_px, vec2<f32>(0.0));
    }

    let direction = tilt / tilt_length;
    let offsets = dispersed_refraction_offsets(cos_theta, material);

    return vec3<f32>(
        sample_backdrop(position_px, direction * offsets.r).r,
        sample_backdrop(position_px, direction * offsets.g).g,
        sample_backdrop(position_px, direction * offsets.b).b,
    );
}
