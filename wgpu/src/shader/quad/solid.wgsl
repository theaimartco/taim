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

    var pos: vec2<f32> = (input.pos + min(input.shadow_offset, vec2<f32>(0.0, 0.0)) - input.shadow_blur_radius) * globals.scale;
    var scale: vec2<f32> = (input.scale + vec2<f32>(abs(input.shadow_offset.x), abs(input.shadow_offset.y)) + input.shadow_blur_radius * 2.0) * globals.scale;

    var pos_snap = vec2<f32>(0.0, 0.0);
    var scale_snap = vec2<f32>(0.0, 0.0);

    if bool(input.snap) {
        pos_snap = round(pos + vec2(0.001, 0.001)) - pos;
        scale_snap = round(pos + scale + vec2(0.001, 0.001)) - pos - pos_snap - scale;
    }

    let border_radius = min(input.border_radius, vec4(min(input.scale.x, input.scale.y) / 2.0));

    var transform: mat4x4<f32> = mat4x4<f32>(
        vec4<f32>(scale.x + scale_snap.x + 1.0, 0.0, 0.0, 0.0),
        vec4<f32>(0.0, scale.y + scale_snap.y + 1.0, 0.0, 0.0),
        vec4<f32>(0.0, 0.0, 1.0, 0.0),
        vec4<f32>(pos + pos_snap - vec2<f32>(0.5, 0.5), 0.0, 1.0)
    );

    out.position = globals.transform * transform * vec4<f32>(vertex_position(input.vertex_index), 0.0, 1.0);
    out.color = premultiply(input.color);
    out.border_color = premultiply(input.border_color);
    out.pos = input.pos * globals.scale + pos_snap;
    out.scale = input.scale * globals.scale + scale_snap;
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

    let sdf_point = -(input.position.xy - input.pos - input.scale * 0.5) * 2.0;

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

    var quad_alpha: f32 = clamp(0.5-dist, 0.0, 1.0);

    // Material shading. `sdf_point` is scaled by -2 relative to screen space,
    // so its gradient is negated to point outwards on screen.
    let sdf_gradient = -rounded_box_gradient(sdf_point, input.scale, input.border_radius * 2.0);

    // What the quad transmits is part of its own colour, not something
    // composited over it -- the hardware's alpha blend against the
    // destination is what shows content through a *translucent* quad, and
    // that shows it undisplaced. This is the refracted version.
    //
    // Gated so an opaque quad -- nearly every quad -- costs one comparison
    // and no texture read.
    if input.transmission > 0.0 && has_backdrop() {
        let behind = quad_transmission(
            dist,
            sdf_gradient,
            input.material,
            input.transmission,
            input.position.xy
        );

        // `mixed_color` is premultiplied (see `premultiply` in the vertex
        // stage), so the sampled backdrop is premultiplied by the same alpha
        // before mixing. Mixing a straight colour into a premultiplied one
        // brightens every translucent pane.
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

    var quad_color = mixed_color * quad_alpha;

    let highlight = quad_specular(
        dist,
        sdf_gradient,
        input.material,
        globals.light.xyz
    );

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
