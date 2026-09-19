struct GradientVertexInput {
    @location(0) v_pos: vec2<f32>,
    @location(1) @interpolate(flat) colors_1: vec4<u32>,
    @location(2) @interpolate(flat) colors_2: vec4<u32>,
    @location(3) @interpolate(flat) colors_3: vec4<u32>,
    @location(4) @interpolate(flat) colors_4: vec4<u32>,
    @location(5) @interpolate(flat) offsets: vec4<u32>,
    @location(6) direction: vec4<f32>,
}

struct GradientVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) raw_position: vec2<f32>,
    @location(1) @interpolate(flat) colors_1: vec4<u32>,
    @location(2) @interpolate(flat) colors_2: vec4<u32>,
    @location(3) @interpolate(flat) colors_3: vec4<u32>,
    @location(4) @interpolate(flat) colors_4: vec4<u32>,
    @location(5) @interpolate(flat) offsets: vec4<u32>,
    @location(6) direction: vec4<f32>,
}

@vertex
fn gradient_vs_main(input: GradientVertexInput) -> GradientVertexOutput {
    var output: GradientVertexOutput;

    output.position = globals.transform * vec4<f32>(input.v_pos, 0.0, 1.0);
    output.raw_position = input.v_pos;
    output.colors_1 = input.colors_1;
    output.colors_2 = input.colors_2;
    output.colors_3 = input.colors_3;
    output.colors_4 = input.colors_4;
    output.offsets = input.offsets;
    output.direction = input.direction;

    return output;
}

@fragment
fn gradient_fs_main(input: GradientVertexOutput) -> @location(0) vec4<f32> {
    let colors = array<vec4<f32>, 8>(
        unpack_color(input.colors_1.xy),
        unpack_color(input.colors_1.zw),
        unpack_color(input.colors_2.xy),
        unpack_color(input.colors_2.zw),
        unpack_color(input.colors_3.xy),
        unpack_color(input.colors_3.zw),
        unpack_color(input.colors_4.xy),
        unpack_color(input.colors_4.zw),
    );

    let offsets_1: vec4<f32> = unpack_u32(input.offsets.xy);
    let offsets_2: vec4<f32> = unpack_u32(input.offsets.zw);

    var offsets = array<f32, 8>(
        offsets_1.x,
        offsets_1.y,
        offsets_1.z,
        offsets_1.w,
        offsets_2.x,
        offsets_2.y,
        offsets_2.z,
        offsets_2.w,
    );

    return taim_gradient_sample(input.raw_position, input.direction, colors, offsets)
        * globals.opacity;
}
