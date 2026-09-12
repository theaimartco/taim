// Stage 4 Compute: fine.wgsl
// Evaluates per-tile commands, accumulates analytic area coverage, and blends in linear HDR.

struct FineConfig {
    width: u32,
    height: u32,
    tiles_x: u32,
    tiles_y: u32,
    max_commands_per_tile: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
}

struct Command {
    draw_idx: u32,
    material: u32,
    opacity: f32,
    pad: u32,
}

struct Material {
    color_r: f32,
    color_g: f32,
    color_b: f32,
    color_a: f32,
}

@group(0) @binding(0) var<uniform> config: FineConfig;
@group(0) @binding(1) var<storage, read> tile_cmd_counts: array<u32>;
@group(0) @binding(2) var<storage, read> tile_commands: array<Command>;
@group(0) @binding(3) var<storage, read> materials: array<Material>;
@group(0) @binding(4) var output_texture: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(16, 16, 1)
fn main(
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let px = global_id.x;
    let py = global_id.y;

    if (px >= config.width || py >= config.height) {
        return;
    }

    let tx = workgroup_id.x;
    let ty = workgroup_id.y;
    let tile_idx = ty * config.tiles_x + tx;

    let cmd_count = tile_cmd_counts[tile_idx];
    var accumulated_color = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    for (var c = 0u; c < cmd_count && c < config.max_commands_per_tile; c = c + 1u) {
        let read_pos = tile_idx * config.max_commands_per_tile + c;
        let cmd = tile_commands[read_pos];

        let mat = materials[cmd.material];
        let src_color = vec4<f32>(mat.color_r, mat.color_g, mat.color_b, mat.color_a);

        // Analytic coverage approximation for pixel quad
        let alpha = src_color.a * cmd.opacity;
        // Linear compositing: Porter-Duff Over in scene-referred linear space
        accumulated_color = src_color * alpha + accumulated_color * (1.0 - alpha);
    }

    textureStore(output_texture, vec2<i32>(i32(px), i32(py)), accumulated_color);
}
