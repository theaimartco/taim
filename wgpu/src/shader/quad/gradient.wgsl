struct GradientVertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) @interpolate(flat) colors_1: vec4<u32>,
    @location(1) @interpolate(flat) colors_2: vec4<u32>,
    @location(2) @interpolate(flat) colors_3: vec4<u32>,
    @location(3) @interpolate(flat) colors_4: vec4<u32>,
    @location(4) @interpolate(flat) offsets: vec4<u32>,
    @location(5) direction: vec4<f32>,
    @location(6) position_and_scale: vec4<f32>,
    @location(7) border_color: vec4<f32>,
    @location(8) border_radius: vec4<f32>,
    @location(9) border_width: f32,
    @location(10) snap: u32,
    @location(11) material: vec4<f32>,
    @location(15) transmission: f32,
}

struct GradientVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(1) @interpolate(flat) colors_1: vec4<u32>,
    @location(2) @interpolate(flat) colors_2: vec4<u32>,
    @location(3) @interpolate(flat) colors_3: vec4<u32>,
    @location(4) @interpolate(flat) colors_4: vec4<u32>,
    @location(5) @interpolate(flat) offsets: vec4<u32>,
    @location(6) direction: vec4<f32>,
    @location(7) position_and_scale: vec4<f32>,
    @location(8) border_color: vec4<f32>,
    @location(9) border_radius: vec4<f32>,
    @location(10) border_width: f32,
    @location(11) @interpolate(flat) material: vec4<f32>,
    @location(12) @interpolate(flat) transmission: f32,
}

@vertex
fn gradient_vs_main(input: GradientVertexInput) -> GradientVertexOutput {
    var out: GradientVertexOutput;

    let rect = taim_snap_rect(
        input.position_and_scale.xy * globals.scale,
        input.position_and_scale.zw * globals.scale,
        bool(input.snap),
    );

    var min_border_radius = min(input.position_and_scale.z, input.position_and_scale.w) * 0.5;
    var border_radius: vec4<f32> = vec4<f32>(
        min(input.border_radius.x, min_border_radius),
        min(input.border_radius.y, min_border_radius),
        min(input.border_radius.z, min_border_radius),
        min(input.border_radius.w, min_border_radius)
    );

    out.position = taim_quad_vertex(rect, input.vertex_index);
    out.colors_1 = input.colors_1;
    out.colors_2 = input.colors_2;
    out.colors_3 = input.colors_3;
    out.colors_4 = input.colors_4;
    out.offsets = input.offsets;
    out.direction = input.direction * globals.scale;
    out.position_and_scale = vec4<f32>(rect.position, rect.size);
    out.border_color = premultiply(input.border_color);
    out.border_radius = border_radius * globals.scale;
    out.border_width = input.border_width * globals.scale;
    out.material = input.material;
    out.transmission = input.transmission;

    return out;
}

@fragment
fn gradient_fs_main(input: GradientVertexOutput) -> @location(0) vec4<f32> {
    let pos = input.position_and_scale.xy;
    let scale = input.position_and_scale.zw;
    let sdf_point = taim_quad_sdf_point(input.position.xy, pos, scale);
    let dist = rounded_box_sdf(
        sdf_point,
        scale,
        input.border_radius * 2.0
    ) * 0.5;
    let coverage = taim_quad_coverage(dist);

    // Rounded corners and clipped edge pixels outside the quad need neither
    // eight color decodes nor a gradient-stop search or backdrop read.
    if (coverage <= 0.0) {
        return vec4<f32>(0.0);
    }

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

    var mixed_color = taim_gradient_sample(input.position.xy, input.direction, colors, offsets);

    if (input.border_width > 0.0) {
        mixed_color = mix(
            mixed_color,
            input.border_color,
            clamp(0.5 + dist + input.border_width, 0.0, 1.0)
        );
    }

    var highlight = 0.0;
    let refracts = input.transmission > 0.0 && has_backdrop();

    // The default material has IOR 1 and no transmission. Keep it on the
    // cheapest path: only calculate the analytic normal when a fragment can
    // actually use it for refraction or a specular response.
    if (refracts || input.material.z > 1.0) {
        let sdf_gradient = -rounded_box_gradient(sdf_point, scale, input.border_radius * 2.0);

        if (refracts) {
            let behind = quad_transmission(
                dist,
                sdf_gradient,
                input.material,
                input.transmission,
                input.position.xy
            );

            mixed_color = vec4<f32>(
                blend_transmission(
                    mixed_color.rgb,
                    behind * mixed_color.a,
                    input.material,
                    input.transmission,
                    dist,
                    sdf_gradient
                ),
                mixed_color.a
            );
        }

        if (input.material.z > 1.0) {
            highlight = quad_specular(
                dist,
                sdf_gradient,
                input.material,
                globals.light.xyz
            );
        }
    }

    // Additive light, premultiplied by coverage. Alpha is untouched.
    return mixed_color * coverage + vec4<f32>(vec3<f32>(highlight * coverage), 0.0);
}
