//! GPU Compute Rasterizer pipeline implementing Pillar I and IX.
//!
//! Orchestrates the multi-stage compute rasterizer (`path_coarse`, `coarse`, `fine`)
//! using WGSL compute shaders, storage buffers, and workgroup dispatching.

#[cfg(feature = "raster")]
use crate::core::Rectangle;
#[cfg(feature = "raster")]
use taim_raster::shader;
#[cfg(feature = "raster")]
use taim_raster::{Clip, TileGrid};
#[cfg(feature = "raster")]
use bytemuck::{Pod, Zeroable};

#[cfg(feature = "raster")]
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct PathCoarseConfig {
    tiles_x: u32,
    tiles_y: u32,
    segment_count: u32,
    max_segments_per_tile: u32,
}

#[cfg(feature = "raster")]
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
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

#[cfg(feature = "raster")]
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
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

#[cfg(feature = "raster")]
const MAX_SEGMENTS_PER_TILE: u32 = 128;
#[cfg(feature = "raster")]
const MAX_COMMANDS_PER_TILE: u32 = 64;

/// A GPU-resident compute rasterizer orchestrating Stages 2, 3, and 4.
#[cfg(feature = "raster")]
pub struct ComputeRasterizer {
    path_coarse_pipeline: wgpu::ComputePipeline,
    coarse_pipeline: wgpu::ComputePipeline,
    fine_pipeline: wgpu::ComputePipeline,

    path_coarse_layout: wgpu::BindGroupLayout,
    coarse_layout: wgpu::BindGroupLayout,
    fine_layout: wgpu::BindGroupLayout,

    path_coarse_config_buf: wgpu::Buffer,
    coarse_config_buf: wgpu::Buffer,
    fine_config_buf: wgpu::Buffer,
}

#[cfg(feature = "raster")]
impl ComputeRasterizer {
    /// Checks whether the graphics adapter supports the compute rasterizer requirements.
    pub fn is_supported(adapter: &wgpu::Adapter) -> bool {
        let limits = adapter.limits();
        limits.max_storage_buffers_per_shader_stage >= 4
            && limits.max_compute_workgroup_size_x >= 64
    }

    /// Checks whether the graphics device supports the compute rasterizer requirements.
    pub fn is_device_supported(device: &wgpu::Device) -> bool {
        let limits = device.limits();
        limits.max_storage_buffers_per_shader_stage >= 4
            && limits.max_compute_workgroup_size_x >= 64
    }

    /// Creates a new [`ComputeRasterizer`] compiled for `device`.
    pub fn new(device: &wgpu::Device) -> Self {
        // Create Path Coarse Bind Group Layout
        let path_coarse_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taim_raster::path_coarse layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create Coarse Bind Group Layout
        let coarse_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taim_raster::coarse layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create Fine Bind Group Layout
        let fine_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taim_raster::fine layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        // Create Pipelines
        let path_coarse_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("taim_raster::path_coarse module"),
            source: wgpu::ShaderSource::Wgsl(shader::PATH_COARSE.into()),
        });
        let path_coarse_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("taim_raster::path_coarse pipeline layout"),
            bind_group_layouts: &[Some(&path_coarse_layout)],
            immediate_size: 0,
        });
        let path_coarse_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("taim_raster::path_coarse pipeline"),
            layout: Some(&path_coarse_pipe_layout),
            module: &path_coarse_mod,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let coarse_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("taim_raster::coarse module"),
            source: wgpu::ShaderSource::Wgsl(shader::COARSE.into()),
        });
        let coarse_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("taim_raster::coarse pipeline layout"),
            bind_group_layouts: &[Some(&coarse_layout)],
            immediate_size: 0,
        });
        let coarse_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("taim_raster::coarse pipeline"),
            layout: Some(&coarse_pipe_layout),
            module: &coarse_mod,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let fine_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("taim_raster::fine module"),
            source: wgpu::ShaderSource::Wgsl(shader::FINE.into()),
        });
        let fine_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("taim_raster::fine pipeline layout"),
            bind_group_layouts: &[Some(&fine_layout)],
            immediate_size: 0,
        });
        let fine_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("taim_raster::fine pipeline"),
            layout: Some(&fine_pipe_layout),
            module: &fine_mod,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let path_coarse_config_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_raster::path_coarse config"),
            size: std::mem::size_of::<PathCoarseConfig>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let coarse_config_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_raster::coarse config"),
            size: std::mem::size_of::<CoarseConfig>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let fine_config_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_raster::fine config"),
            size: std::mem::size_of::<FineConfig>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            path_coarse_pipeline,
            coarse_pipeline,
            fine_pipeline,
            path_coarse_layout,
            coarse_layout,
            fine_layout,
            path_coarse_config_buf,
            coarse_config_buf,
            fine_config_buf,
        }
    }

    /// Dispatches the compute rasterizer passes in a single [`wgpu::CommandEncoder`].
    pub fn dispatch(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        scene: &taim_raster::Scene,
        output_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        damage: Option<Rectangle>,
    ) {
        if scene.segments().is_empty() || scene.draws().is_empty() || width == 0 || height == 0 {
            return;
        }

        let grid = TileGrid::new(width, height);
        let tiles_x = grid.columns();
        let tiles_y = grid.rows();
        let total_tiles = (tiles_x * tiles_y) as usize;

        // 1. Upload Uniforms
        let path_coarse_config = PathCoarseConfig {
            tiles_x,
            tiles_y,
            segment_count: scene.segments().len() as u32,
            max_segments_per_tile: MAX_SEGMENTS_PER_TILE,
        };
        queue.write_buffer(&self.path_coarse_config_buf, 0, bytemuck::bytes_of(&path_coarse_config));

        let damage_rect = damage.unwrap_or(Rectangle {
            x: 0.0,
            y: 0.0,
            width: width as f32,
            height: height as f32,
        });
        let coarse_config = CoarseConfig {
            tiles_x,
            tiles_y,
            draw_count: scene.draws().len() as u32,
            damage_min_x: damage_rect.x,
            damage_min_y: damage_rect.y,
            damage_max_x: damage_rect.x + damage_rect.width,
            damage_max_y: damage_rect.y + damage_rect.height,
            max_commands_per_tile: MAX_COMMANDS_PER_TILE,
        };
        queue.write_buffer(&self.coarse_config_buf, 0, bytemuck::bytes_of(&coarse_config));

        let fine_config = FineConfig {
            width,
            height,
            tiles_x,
            tiles_y,
            max_commands_per_tile: MAX_COMMANDS_PER_TILE,
            pad0: 0,
            pad1: 0,
            pad2: 0,
        };
        queue.write_buffer(&self.fine_config_buf, 0, bytemuck::bytes_of(&fine_config));

        // 2. Storage Buffers
        let segments_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_raster::segments buffer"),
            size: (scene.segment_bytes().len() as u64).max(32),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&segments_buf, 0, scene.segment_bytes());

        let draws_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_raster::draws buffer"),
            size: (scene.draw_bytes().len() as u64).max(32),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&draws_buf, 0, scene.draw_bytes());

        let clips_bytes = if scene.clips().is_empty() {
            vec![0u8; std::mem::size_of::<Clip>()]
        } else {
            scene.clip_bytes().to_vec()
        };
        let clips_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_raster::clips buffer"),
            size: (clips_bytes.len() as u64).max(32),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&clips_buf, 0, &clips_bytes);

        // Tile binning scratch buffers
        let tile_counts_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_raster::tile_counts buffer"),
            size: ((total_tiles * 4) as u64).max(32),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let tile_segments_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_raster::tile_segments buffer"),
            size: ((total_tiles * MAX_SEGMENTS_PER_TILE as usize * 4) as u64).max(32),
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        // Command lists scratch buffers
        let tile_cmd_counts_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_raster::tile_cmd_counts buffer"),
            size: ((total_tiles * 4) as u64).max(32),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let tile_commands_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_raster::tile_commands buffer"),
            size: ((total_tiles * MAX_COMMANDS_PER_TILE as usize * 16) as u64).max(32),
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        // Default materials buffer (at least 1 material with opaque white fallback)
        let default_material: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
        let materials_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("taim_raster::materials buffer"),
            size: 32,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&materials_buf, 0, bytemuck::cast_slice(&default_material));

        // Bind Groups
        let path_coarse_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("taim_raster::path_coarse bind group"),
            layout: &self.path_coarse_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.path_coarse_config_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: segments_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: tile_counts_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: tile_segments_buf.as_entire_binding(),
                },
            ],
        });

        let coarse_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("taim_raster::coarse bind group"),
            layout: &self.coarse_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.coarse_config_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: draws_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: clips_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: tile_cmd_counts_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: tile_commands_buf.as_entire_binding(),
                },
            ],
        });

        let fine_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("taim_raster::fine bind group"),
            layout: &self.fine_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.fine_config_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: tile_cmd_counts_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: tile_commands_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: materials_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(output_view),
                },
            ],
        });

        // Compute Pass Dispatching
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("taim_raster::compute pass"),
            timestamp_writes: None,
        });

        // Stage 2: Path Coarse (64 threads per workgroup)
        pass.set_pipeline(&self.path_coarse_pipeline);
        pass.set_bind_group(0, &path_coarse_bg, &[]);
        let seg_workgroups = (scene.segments().len() as u32).div_ceil(64);
        pass.dispatch_workgroups(seg_workgroups.max(1), 1, 1);

        // Stage 3: Coarse (16x16 tiles per workgroup)
        pass.set_pipeline(&self.coarse_pipeline);
        pass.set_bind_group(0, &coarse_bg, &[]);
        let tile_wg_x = tiles_x.div_ceil(16);
        let tile_wg_y = tiles_y.div_ceil(16);
        pass.dispatch_workgroups(tile_wg_x.max(1), tile_wg_y.max(1), 1);

        // Stage 4: Fine (16x16 pixels per tile workgroup)
        pass.set_pipeline(&self.fine_pipeline);
        pass.set_bind_group(0, &fine_bg, &[]);
        pass.dispatch_workgroups(tiles_x, tiles_y, 1);
    }
}
