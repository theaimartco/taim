struct Viewport {
    size: vec2<u32>,
    _padding: vec2<u32>,
};

struct GlyphInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) position: vec2<i32>,
    @location(1) size: vec2<u32>,
    @location(2) atlas_origin: vec2<u32>,
    @location(3) color: vec4<f32>,
    @location(4) plane: u32,
};

struct GlyphOutput {
    @invariant @builtin(position) position: vec4<f32>,
    @location(0) atlas_pixel: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) plane: u32,
};

@group(0) @binding(0)
var color_atlas: texture_2d<f32>;

@group(0) @binding(1)
var mask_atlas: texture_2d<f32>;

@group(1) @binding(0)
var<uniform> viewport: Viewport;

@vertex
fn vertex(input: GlyphInput) -> GlyphOutput {
    // The four strip corners are encoded directly. This avoids dynamically
    // indexing a shader-local array on GLES compilers.
    let index = min(input.vertex_index, 3u);
    let corner = vec2<u32>(index & 1u, (index >> 1u) & 1u);
    let offset = corner * input.size;
    let pixel = input.position + vec2<i32>(offset);
    let safe_viewport = max(viewport.size, vec2<u32>(1u));

    var output: GlyphOutput;
    output.position = vec4<f32>(
        2.0 * vec2<f32>(pixel) / vec2<f32>(safe_viewport) - 1.0,
        0.0,
        1.0,
    );
    output.position.y = -output.position.y;
    output.atlas_pixel = vec2<f32>(input.atlas_origin + offset);
    output.color = input.color;
    output.plane = input.plane;
    return output;
}

@fragment
fn fragment(input: GlyphOutput) -> @location(0) vec4<f32> {
    let coordinate = vec2<i32>(floor(input.atlas_pixel));

    if input.plane == 0u {
        let bitmap = textureLoad(color_atlas, coordinate, 0);
        return vec4<f32>(bitmap.rgb, bitmap.a * input.color.a);
    }

    let coverage = textureLoad(mask_atlas, coordinate, 0).r;
    return vec4<f32>(input.color.rgb, input.color.a * coverage);
}
