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
    // x: Gaussian radius in texels. y: noise amplitude, 0..1.
    // z: noise seed. w: spare, keeping the struct 16-byte aligned.
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

// A 32-bit integer hash, from the PCG family.
//
// Deterministic: the same pixel and seed always produce the same value, so a
// noise overlay does not shimmer between frames of an otherwise static
// screen -- which is what "deterministic material noise" has to mean, and
// what a `fract(sin(dot(...)))` hash cannot promise across GPUs.
fn hash(value: u32) -> u32 {
    var state = value * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;

    return (word >> 22u) ^ word;
}

// Material noise: a fine, signed grain over the source.
//
// Grain rather than a colour shift, so it breaks up the banding a wide
// gradient or a heavy blur produces without changing what anything looks
// like. The amplitude is deliberately small; noise a user can *see* as noise
// is a bug in a UI.
@fragment
fn fs_noise(input: VertexOutput) -> @location(0) vec4<f32> {
    let source = textureSample(u_texture, u_sampler, input.uv);

    let amount = u.params.y;

    if amount <= 0.0 {
        return source;
    }

    // Pixel coordinates rather than UV: grain must be one texel across
    // whatever the surface size, or it turns into blotches when scaled.
    let pixel = vec2<u32>(input.uv / u.step.xy);
    let seed = bitcast<u32>(u.params.z);
    let noise = f32(hash(pixel.x ^ hash(pixel.y ^ hash(seed)))) / 4294967295.0;

    // Signed, so grain neither brightens nor darkens the image overall.
    let grain = (noise - 0.5) * 2.0 * amount;

    let alpha = max(source.a, 0.0001);
    let colour = clamp(source.rgb / alpha + vec3<f32>(grain), vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(colour * source.a, source.a);
}

// The mask atlas. Bound alongside the source in group 1, and only by the mask
// pipeline, which has a bind group layout of its own -- so no other pass
// carries a binding it never reads.
//
// Group 1 rather than a group of its own: `max_bind_groups` is 2 on the
// portability floor this renderer targets, and a third group is a validation
// error on any device at that limit.
@group(1) @binding(1) var u_mask: texture_2d<f32>;

// Clips the source to a mask.
//
// The mask's red channel is its coverage: 1 keeps a pixel, 0 erases it, and
// the values between antialias the edge. Multiplying *premultiplied* colour
// and alpha together is correct here and is why this pass, unlike the colour
// matrix, does not un-premultiply first -- scaling coverage scales both.
@fragment
fn fs_mask(input: VertexOutput) -> @location(0) vec4<f32> {
    let source = textureSample(u_texture, u_sampler, input.uv);

    // The mask is addressed across the effect's region rather than the
    // surface: a mask is a shape for *this* region, and stretching it over
    // the whole frame would put it somewhere else entirely.
    let local = (input.uv / u.step.xy - u.region.xy) / u.region.zw;

    // Outside the region there is no mask, and no pixel to keep.
    if local.x < 0.0 || local.x > 1.0 || local.y < 0.0 || local.y > 1.0 {
        return vec4<f32>(0.0);
    }

    let coverage = textureSample(u_mask, u_sampler, vec2<f32>(local.x, local.y)).r;

    return source * coverage;
}
