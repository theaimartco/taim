struct SolidVertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) color: vec4<f32>,
    @location(1) pos: vec2<f32>,
    @location(2) scale: vec2<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) border_radius: vec4<f32>,
    @location(5) border_width: f32,
    @location(6) shadow_color: vec4<f32>,
    @location(7) shadow_offset: vec2<f32>,
    @location(8) shadow_blur_radius: f32,
    @location(9) snap: u32,
    @location(10) material: vec4<f32>,
    @location(11) transmission: f32,
}

struct SolidVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) border_color: vec4<f32>,
    @location(2) pos: vec2<f32>,
    @location(3) scale: vec2<f32>,
    @location(4) border_radius: vec4<f32>,
    @location(5) border_width: f32,
    @location(6) shadow_color: vec4<f32>,
    @location(7) shadow_offset: vec2<f32>,
    @location(8) shadow_blur_radius: f32,
    @location(9) @interpolate(flat) material: vec4<f32>,
    @location(10) @interpolate(flat) transmission: f32,
}

@vertex
fn solid_vs_main(input: SolidVertexInput) -> SolidVertexOutput {
    var out: SolidVertexOutput;

    let rect = taim_snap_rect(
        (input.pos + min(input.shadow_offset, vec2<f32>(0.0)) - input.shadow_blur_radius) * globals.scale,
        (input.scale + abs(input.shadow_offset) + input.shadow_blur_radius * 2.0) * globals.scale,
        bool(input.snap),
    );

    let border_radius = min(input.border_radius, vec4(min(input.scale.x, input.scale.y) / 2.0));

    out.position = taim_quad_vertex(rect, input.vertex_index);
    out.color = premultiply(input.color);
    out.border_color = premultiply(input.border_color);
    out.pos = input.pos * globals.scale + rect.position_delta;
    out.scale = input.scale * globals.scale + rect.size_delta;
    out.border_radius = border_radius * globals.scale;
    out.border_width = input.border_width * globals.scale;
    out.shadow_color = premultiply(input.shadow_color);
    out.shadow_offset = input.shadow_offset * globals.scale;
    out.shadow_blur_radius = input.shadow_blur_radius * globals.scale;
    out.material = input.material;
    out.transmission = input.transmission;

    return out;
}

@fragment
fn solid_fs_main(
    input: SolidVertexOutput
) -> @location(0) vec4<f32> {
    var mixed_color: vec4<f32> = input.color;

    let sdf_point = taim_quad_sdf_point(input.position.xy, input.pos, input.scale);

    var dist = rounded_box_sdf(
        sdf_point,
        input.scale,
        input.border_radius * 2.0
    ) / 2.0;

    if (input.border_width > 0.0) {
        mixed_color = mix(
            input.color,
            input.border_color,
            clamp(0.5 + dist + input.border_width, 0.0, 1.0)
        );
    }

    let quad_alpha = taim_quad_coverage(dist);

    if (quad_alpha <= 0.0 && input.shadow_color.a <= 0.0) {
        return vec4<f32>(0.0);
    }

    var quad_color = mixed_color * quad_alpha;
    var highlight = 0.0;
    let refracts = input.transmission > 0.0 && has_backdrop();

    // The default material has IOR 1 and no transmission. Avoid calculating
    // an analytic normal for this overwhelmingly common case.
    if (refracts || input.material.z > 1.0) {
        let sdf_gradient = -rounded_box_gradient(
            sdf_point,
            input.scale,
            input.border_radius * 2.0
        );

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
            quad_color = mixed_color * quad_alpha;
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

    // Additive light, premultiplied by coverage to match `quad_color`. Alpha
    // is untouched: a highlight adds light, not opacity.
    quad_color = quad_color + vec4<f32>(vec3<f32>(highlight * quad_alpha), 0.0);

    if input.shadow_color.a > 0.0 {
        var shadow_dist: f32 = rounded_box_sdf(
            -(input.position.xy - input.pos - input.shadow_offset - input.scale/2.0) * 2.0,
            input.scale,
            input.border_radius * 2.0
        ) / 2.0;
        let shadow_alpha = 1.0 - smoothstep(-input.shadow_blur_radius, input.shadow_blur_radius, max(shadow_dist, 0.0));

        return mix(quad_color, input.shadow_color, (1.0 - quad_alpha) * shadow_alpha);
    } else {
        return quad_color;
    }
}
