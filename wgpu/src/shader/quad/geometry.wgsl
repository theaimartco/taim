// Geometry utilities shared by every instanced TAIM quad pipeline.
struct TaimSnappedRect {
    position: vec2<f32>,
    size: vec2<f32>,
    position_delta: vec2<f32>,
    size_delta: vec2<f32>,
}

fn taim_snap_rect(position: vec2<f32>, size: vec2<f32>, enabled: bool) -> TaimSnappedRect {
    var position_delta = vec2<f32>(0.0);
    var size_delta = vec2<f32>(0.0);

    if enabled {
        let snapped_min = round(position + vec2<f32>(0.001));
        let snapped_max = round(position + size + vec2<f32>(0.001));
        position_delta = snapped_min - position;
        size_delta = snapped_max - snapped_min - size;
    }

    return TaimSnappedRect(
        position + position_delta,
        size + size_delta,
        position_delta,
        size_delta,
    );
}

fn taim_quad_vertex(rect: TaimSnappedRect, vertex_index: u32) -> vec4<f32> {
    let corner = vertex_position(vertex_index);
    let pixel = rect.position - vec2<f32>(0.5) + corner * (rect.size + vec2<f32>(1.0));
    return globals.transform * vec4<f32>(pixel, 0.0, 1.0);
}

fn taim_quad_sdf_point(fragment: vec2<f32>, position: vec2<f32>, size: vec2<f32>) -> vec2<f32> {
    return (position + size * 0.5 - fragment) * 2.0;
}

fn taim_quad_coverage(distance: f32) -> f32 {
    return clamp(0.5 - distance, 0.0, 1.0);
}
