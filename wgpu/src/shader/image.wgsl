struct Globals {
    transform: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var atlas_sampler: sampler;
@group(1) @binding(0) var atlas_texture: texture_2d_array<f32>;

struct ImageInstance {
    @builtin(vertex_index) vertex: u32,
    @location(0) rotation_center: vec2<f32>,
    @location(1) clip: vec4<f32>,
    @location(2) radii: vec4<f32>,
    @location(3) tile: vec4<f32>,
    @location(4) rotation: vec2<f32>,
    @location(5) opacity: f32,
    @location(6) atlas_origin: vec2<f32>,
    @location(7) atlas_extent: vec2<f32>,
    @location(8) layer: i32,
}

struct ImageFragment {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) clip: vec4<f32>,
    @location(1) @interpolate(flat) radii: vec4<f32>,
    @location(2) @interpolate(flat) atlas_bounds: vec4<f32>,
    @location(3) @interpolate(flat) layer: i32,
    @location(4) @interpolate(flat) opacity: f32,
    @location(5) uv: vec2<f32>,
}

fn rotated_aabb(tile: vec4<f32>, center: vec2<f32>, rotation: vec2<f32>) -> vec4<f32> {
    let half = tile.zw * 0.5;
    let local_center = tile.xy + half - center;
    let cosine = rotation.x;
    let sine = rotation.y;
    let world_center = center + vec2<f32>(
        local_center.x * cosine - local_center.y * sine,
        local_center.x * sine + local_center.y * cosine,
    );
    let world_half = vec2<f32>(
        abs(cosine) * half.x + abs(sine) * half.y,
        abs(sine) * half.x + abs(cosine) * half.y,
    );
    return vec4<f32>(world_center - world_half, world_half * 2.0);
}

fn inverse_rotate(point: vec2<f32>, center: vec2<f32>, rotation: vec2<f32>) -> vec2<f32> {
    let delta = point - center;
    return center + vec2<f32>(
        delta.x * rotation.x + delta.y * rotation.y,
        -delta.x * rotation.y + delta.y * rotation.x,
    );
}

@vertex
fn vs_main(input: ImageInstance) -> ImageFragment {
    var out: ImageFragment;
    let bounds = rotated_aabb(input.tile, input.rotation_center, input.rotation);
    let minimum = max(bounds.xy, input.clip.xy);
    let maximum = min(bounds.xy + bounds.zw, input.clip.xy + input.clip.zw);
    let visible = vec4<f32>(minimum, max(maximum - minimum, vec2<f32>(0.0)));
    let pixel = visible.xy + vertex_position(input.vertex) * visible.zw;
    let source = inverse_rotate(pixel, input.rotation_center, input.rotation);
    let normalized = (source - input.tile.xy) / input.tile.zw;

    out.position = globals.transform * vec4<f32>(pixel, 0.0, 1.0);
    out.clip = input.clip;
    out.radii = min(input.radii, vec4<f32>(min(input.clip.z, input.clip.w) * 0.5));
    out.atlas_bounds = vec4<f32>(input.atlas_origin, input.atlas_origin + input.atlas_extent);
    out.layer = input.layer;
    out.opacity = input.opacity;
    out.uv = input.atlas_origin + normalized * input.atlas_extent;
    return out;
}

fn image_rounded_distance(point: vec2<f32>, extent: vec2<f32>, radii: vec4<f32>) -> f32 {
    let horizontal = select(radii.yz, radii.xw, point.x > 0.0);
    let radius = select(horizontal.y, horizontal.x, point.y > 0.0);
    let corner = abs(point) - extent + radius;
    return min(max(corner.x, corner.y), 0.0)
        + length(max(corner, vec2<f32>(0.0)))
        - radius;
}

@fragment
fn fs_main(input: ImageFragment) -> @location(0) vec4<f32> {
    let half = input.clip.zw * 0.5;
    let local = (input.position.xy - input.clip.xy - half) * 2.0;
    let distance = image_rounded_distance(local, input.clip.zw, input.radii * 2.0) * 0.5;
    let coverage = clamp(1.0 - distance, 0.0, 1.0);
    let inside = all(input.uv >= input.atlas_bounds.xy)
        && all(input.uv <= input.atlas_bounds.zw);
    let alpha = coverage * input.opacity * f32(inside);

    if alpha <= 0.0 {
        return vec4<f32>(0.0);
    }

    let texel = textureSample(atlas_texture, atlas_sampler, input.uv, input.layer);
    return premultiply(texel * vec4<f32>(1.0, 1.0, 1.0, alpha));
}
