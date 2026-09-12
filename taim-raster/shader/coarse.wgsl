// Stage 3 Compute: coarse.wgsl
// Walks draw order per tile, resolves clips and culls against damage rectangle.

struct CoarseConfig {
    tiles_x: u32,
    tiles_y: u32,
    draw_count: u32,
    damage_min_x: f32,
    damage_min_y: f32,
    damage_max_x: f32,
    damage_max_y: f32,
    max_commands_per_tile: u32,
}

struct DrawObject {
    path: u32,
    clip: u32,
    material: u32,
    opacity: f32,
}

struct Clip {
    path: u32,
    parent: u32,
}

struct Command {
    draw_idx: u32,
    material: u32,
    opacity: f32,
    pad: u32,
}

@group(0) @binding(0) var<uniform> config: CoarseConfig;
@group(0) @binding(1) var<storage, read> draws: array<DrawObject>;
@group(0) @binding(2) var<storage, read> clips: array<Clip>;
@group(0) @binding(3) var<storage, read_write> tile_cmd_counts: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> tile_commands: array<Command>;

const TILE_SIZE: f32 = 16.0;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let tx = global_id.x;
    let ty = global_id.y;

    if (tx >= config.tiles_x || ty >= config.tiles_y) {
        return;
    }

    let tile_min_x = f32(tx) * TILE_SIZE;
    let tile_min_y = f32(ty) * TILE_SIZE;
    let tile_max_x = tile_min_x + TILE_SIZE;
    let tile_max_y = tile_min_y + TILE_SIZE;

    // Cull tile if completely disjoint from damage region
    if (tile_max_x <= config.damage_min_x || tile_min_x >= config.damage_max_x ||
        tile_max_y <= config.damage_min_y || tile_min_y >= config.damage_max_y) {
        return;
    }

    let tile_idx = ty * config.tiles_x + tx;

    // Process draw objects in painter order
    for (var i = 0u; i < config.draw_count; i = i + 1u) {
        let draw = draws[i];
        
        let slot = atomicAdd(&tile_cmd_counts[tile_idx], 1u);
        if (slot < config.max_commands_per_tile) {
            let write_pos = tile_idx * config.max_commands_per_tile + slot;
            tile_commands[write_pos] = Command(i, draw.material, draw.opacity, 0u);
        }
    }
}
