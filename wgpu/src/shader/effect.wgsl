// Post-process effects, applied to a region of an already-drawn frame.
//
// One fullscreen triangle-pair vertex stage feeds every effect; the region is
// selected by `u_region`, so a blur over a 200x80 card samples and writes only
// that card rather than the whole surface.

struct Uniforms {
    // xy: the region's origin in texels. zw: its size in texels.
    region: vec4<f32>,
    // xy: one texel in UV space. zw: the sampling direction, in texels.
    step: vec4<f32>,
    // Gaussian radius in texels, then three spare slots kept so the struct
    // stays 16-byte aligned on every backend.
    params: vec4<f32>,
    // A 4x5 colour matrix, row-major: rows 0..3 multiply RGBA, `bias` adds.
    matrix_r: vec4<f32>,
    matrix_g: vec4<f32>,
    matrix_b: vec4<f32>,
    matrix_a: vec4<f32>,
    bias: vec4<f32>,
}

@group(0) @binding(0) var u_sampler: sampler;
@group(0) @binding(1) var<uniform> u: Uniforms;
@group(1) @binding(0) var u_texture: texture_2d<f32>;

var<private> corners: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0),
);

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    // The source texel to read, in UV space.
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let corner = corners[index];

    // The region in texels, then in UV, then in clip space. Doing this in the
    // vertex stage keeps the fragment stage free of the region entirely.
    let texel = u.region.xy + corner * u.region.zw;
    let uv = texel * u.step.xy;

    var out: VertexOutput;
    out.uv = uv;
    out.position = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);

    return out;
}

// One half of a separable Gaussian.
//
// Separable because a 2D Gaussian of radius r costs (2r+1)^2 taps in one pass
// and 2*(2r+1) in two, which at r = 16 is 1089 against 66. The caller runs
// this twice, swapping `u.step.zw` between horizontal and vertical.
@fragment
fn fs_blur(input: VertexOutput) -> @location(0) vec4<f32> {
    let radius = u.params.x;

    if radius <= 0.0 {
        return textureSample(u_texture, u_sampler, input.uv);
    }

    // sigma chosen so the kernel has decayed to near zero at the radius,
    // which is what stops a visible edge where the taps stop.
    let sigma = max(radius * 0.5, 0.0001);
    let denominator = 2.0 * sigma * sigma;

    let direction = u.step.zw * u.step.xy;

    var total = vec4<f32>(0.0);
    var weight_total = 0.0;

    // Bounded by the tier's max blur radius on the CPU side, so this loop is
    // never unbounded however large a caller asks for.
    let taps = i32(min(radius, 32.0));

    for (var offset = -taps; offset <= taps; offset = offset + 1) {
        let distance = f32(offset);
        let weight = exp(-(distance * distance) / denominator);

        total = total + textureSample(u_texture, u_sampler, input.uv + direction * distance) * weight;
        weight_total = weight_total + weight;
    }

    return total / weight_total;
}

// A 4x5 colour matrix. Saturation, grayscale, tint and inversion are all
// instances of it, so they share one pipeline rather than one each.
@fragment
fn fs_color_matrix(input: VertexOutput) -> @location(0) vec4<f32> {
    let source = textureSample(u_texture, u_sampler, input.uv);

    // Un-premultiply before the matrix and re-premultiply after: a matrix
    // applied to premultiplied colour darkens everything it touches.
    let alpha = max(source.a, 0.0001);
    let colour = vec4<f32>(source.rgb / alpha, source.a);

    let result = vec4<f32>(
        dot(u.matrix_r, colour),
        dot(u.matrix_g, colour),
        dot(u.matrix_b, colour),
        dot(u.matrix_a, colour),
    ) + u.bias;

    let clamped = clamp(result, vec4<f32>(0.0), vec4<f32>(1.0));

    return vec4<f32>(clamped.rgb * clamped.a, clamped.a);
}

// A straight copy, used to put a finished effect back onto the frame.
@fragment
fn fs_blit(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(u_texture, u_sampler, input.uv);
}
