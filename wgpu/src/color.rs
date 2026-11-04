use std::borrow::Cow;

/// Retained resources for exact offscreen texture conversion.
///
/// The source and destination always have identical dimensions, so this pass
/// uses `textureLoad` instead of a sampler. Besides producing an exact pixel
/// copy, this removes a sampler, a ratio uniform, and one bind group from the
/// screenshot path.
pub struct Pipeline {
    format: wgpu::TextureFormat,
    texture_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

impl Pipeline {
    /// Compiles a conversion pipeline for `format` once.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        Self::with_encoding(device, format, false)
    }

    fn with_encoding(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        encode_srgb: bool,
    ) -> Self {
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("taim_wgpu.offscreen.blit.texture_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("taim_wgpu.offscreen.blit.pipeline_layout"),
            bind_group_layouts: &[Some(&texture_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("taim_wgpu.offscreen.blit.shader"),
            source: wgpu::ShaderSource::Wgsl(if encode_srgb {
                Cow::Owned(include_str!("shader/screenshot.wgsl").replace(
                    "const ENCODE_SRGB: bool = false;",
                    "const ENCODE_SRGB: bool = true;",
                ))
            } else {
                Cow::Borrowed(include_str!("shader/screenshot.wgsl"))
            }),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("taim_wgpu.offscreen.blit.pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // This is a format conversion, not compositing. Replacing the
                    // destination also avoids reading it and preserves RGB under
                    // partially transparent pixels.
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Cw,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            format,
            texture_layout,
            pipeline,
        }
    }

    /// Returns the output format this pipeline was compiled for.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Converts `source`, consuming it and returning an exact-size texture.
    pub fn convert(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: wgpu::Texture,
    ) -> wgpu::Texture {
        debug_assert_ne!(source.format(), self.format);

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("taim_wgpu.offscreen.conversion.target_texture"),
            size: source.size(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = &texture.create_view(&wgpu::TextureViewDescriptor::default());

        let source_view = source.create_view(&wgpu::TextureViewDescriptor::default());
        let texture_bind_group = self.bind_source(device, &source_view);
        self.render(encoder, view, &texture_bind_group);
        texture
    }

    fn bind_source(&self, device: &wgpu::Device, source: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("taim_wgpu.offscreen.blit.texture_bind_group"),
            layout: &self.texture_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source),
            }],
        })
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        texture_bind_group: &wgpu::BindGroup,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("taim_wgpu.offscreen.blit.render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, texture_bind_group, &[]);
        pass.draw(0..3, 0..1);

        drop(pass);
    }
}

/// Retained SDR intermediate for surfaces that cannot be copied from, or do
/// not perform sRGB encoding. Effects always read this owned texture.
pub(crate) struct SurfaceTarget {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pipeline: Pipeline,
    source: wgpu::BindGroup,
}

impl SurfaceTarget {
    pub fn new(
        device: &wgpu::Device,
        size: wgpu::Extent3d,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let format = if crate::graphics::color::GAMMA_CORRECTION {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("taim.surface.sdr.intermediate"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let pipeline = Pipeline::with_encoding(
            device,
            surface_format,
            crate::graphics::color::GAMMA_CORRECTION && !surface_format.is_srgb(),
        );
        let source = pipeline.bind_source(device, &view);
        Self {
            texture,
            view,
            pipeline,
            source,
        }
    }
    pub fn fits(&self, size: wgpu::Extent3d, format: wgpu::TextureFormat) -> bool {
        self.texture.size() == size && self.pipeline.format == format
    }
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        self.pipeline.render(encoder, view, &self.source);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait<T>(future: impl std::future::Future<Output = T>) -> T {
        struct Wake(std::thread::Thread);
        impl std::task::Wake for Wake {
            fn wake(self: std::sync::Arc<Self>) {
                self.0.unpark();
            }
        }
        let waker = std::task::Waker::from(std::sync::Arc::new(Wake(std::thread::current())));
        let mut context = std::task::Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                std::task::Poll::Ready(result) => return result,
                std::task::Poll::Pending => std::thread::park(),
            }
        }
    }

    #[test]
    #[ignore = "requires a real GPU adapter; run explicitly for surface color acceptance"]
    fn copyless_surface_encodes_sdr_once() {
        let instance = wgpu::Instance::default();
        let adapter = wait(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("GPU adapter");
        let (device, queue) =
            wait(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("GPU device");
        let size = wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        };
        let target = SurfaceTarget::new(&device, size, wgpu::TextureFormat::Rgba8Unorm);
        let surface = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256 * 8,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.25,
                            g: 0.5,
                            b: 0.75,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        target.resolve(
            &mut encoder,
            &surface.create_view(&wgpu::TextureViewDescriptor::default()),
        );
        encoder.copy_texture_to_buffer(
            surface.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: None,
                },
            },
            size,
        );
        let submission = queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                sender.send(result).expect("map receiver");
            });
        let _ = device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(5)),
            })
            .expect("GPU completion");
        receiver
            .recv()
            .expect("map callback")
            .expect("readback map");
        let pixels = readback.slice(..).get_mapped_range();
        let expected = if crate::graphics::color::GAMMA_CORRECTION {
            [137u8, 188, 225, 255]
        } else {
            [64u8, 128, 191, 255]
        };
        for row in 0..8 {
            for pixel in pixels[row * 256..row * 256 + 32].chunks_exact(4) {
                for (actual, expected) in pixel.iter().zip(expected) {
                    assert!(
                        actual.abs_diff(expected) <= 1,
                        "surface pixel {pixel:?}, expected {expected}"
                    );
                }
            }
        }
        drop(pixels);
        readback.unmap();
    }
}
