// Stage 2 Compute: path_coarse.wgsl
// Bins straight path segments into 16x16 pixel tiles.

struct Config {
    tiles_x: u32,
    tiles_y: u32,
    segment_count: u32,
    max_segments_per_tile: u32,
}

struct Segment {
    from_x: f32,
    from_y: f32,
    to_x: f32,
    to_y: f32,
}

@group(0) @binding(0) var<uniform> config: Config;
@group(0) @binding(1) var<storage, read> segments: array<Segment>;
@group(0) @binding(2) var<storage, read_write> tile_counts: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> tile_segments: array<u32>;

const TILE_SIZE: f32 = 16.0;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let seg_idx = global_id.x;
    if (seg_idx >= config.segment_count) {
        return;
    }

    let seg = segments[seg_idx];

    let min_x = min(seg.from_x, seg.to_x);
    let max_x = max(seg.from_x, seg.to_x);
    let min_y = min(seg.from_y, seg.to_y);
    let max_y = max(seg.from_y, seg.to_y);

    let tx_start = clamp(u32(floor(max(min_x, 0.0) / TILE_SIZE)), 0u, config.tiles_x - 1u);
    let tx_end = clamp(u32(ceil(max(max_x, 0.0) / TILE_SIZE)), 0u, config.tiles_x - 1u);
    let ty_start = clamp(u32(floor(max(min_y, 0.0) / TILE_SIZE)), 0u, config.tiles_y - 1u);
    let ty_end = clamp(u32(ceil(max(max_y, 0.0) / TILE_SIZE)), 0u, config.tiles_y - 1u);

    for (var ty = ty_start; ty <= ty_end; ty = ty + 1u) {
        for (var tx = tx_start; tx <= tx_end; tx = tx + 1u) {
            let tile_idx = ty * config.tiles_x + tx;
            let slot = atomicAdd(&tile_counts[tile_idx], 1u);
            if (slot < config.max_segments_per_tile) {
                let write_pos = tile_idx * config.max_segments_per_tile + slot;
                tile_segments[write_pos] = seg_idx;
            }
        }
    }
}
