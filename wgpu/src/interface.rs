use super::*;

impl core::Renderer for Renderer {
    fn start_layer(&mut self, bounds: Rectangle) {
        self.layers.push_clip(bounds);
    }

    fn end_layer(&mut self) {
        self.layers.pop_clip();
    }

    fn start_transformation(&mut self, transformation: Transformation) {
        self.layers.push_transformation(transformation);
    }

    fn end_transformation(&mut self) {
        self.layers.pop_transformation();
    }

    fn start_opacity(&mut self, opacity: f32) {
        self.layers.push_opacity(opacity);
    }

    fn end_opacity(&mut self) {
        self.layers.pop_opacity();
    }

    fn fill_quad(&mut self, mut quad: core::renderer::Quad, background: impl Into<Background>) {
        let opacity = self.layers.opacity();
        quad.border.color = quad.border.color.scale_alpha(opacity);
        quad.shadow.color = quad.shadow.color.scale_alpha(opacity);
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_quad(quad, background.into().scale_alpha(opacity), transformation);
    }

    fn replay_retained_layer(&mut self, key: u64, revision: u64, bounds: Rectangle) -> bool {
        let transformation = self.layers.transformation();
        let opacity = self.layers.opacity();
        let scale_factor = self.scale_factor;
        let retained = self.retained_frames.take(key);
        let had_cached_frame = retained.is_some();

        if let Some(retained) = retained
            && retained.key == key
            && retained.revision == revision
            && retained.bounds == bounds
            && retained.transformation == transformation
            && retained.opacity == opacity
            && retained.scale_factor == scale_factor
        {
            let range = self.layers.replay_owned(retained.display_list);
            self.retained_layer_statistics.hits += 1;
            self.active_retained_frames.push(ActiveRetainedFrame {
                key,
                revision,
                bounds,
                transformation,
                opacity,
                scale_factor,
                range,
            });
            return true;
        }

        self.retained_layer_statistics.misses += 1;
        if had_cached_frame {
            self.retained_layer_statistics.evictions += 1;
        }

        false
    }

    fn start_retained_layer(&mut self, key: u64, revision: u64, bounds: Rectangle) {
        let start = self.layers.active_count();
        let transformation = self.layers.transformation();
        let opacity = self.layers.opacity();
        self.layers.push_clip(bounds);
        self.recording_retained_frames.push(RecordingRetainedFrame {
            key,
            revision,
            bounds,
            transformation,
            opacity,
            scale_factor: self.scale_factor,
            start,
        });
    }

    fn end_retained_layer(&mut self) {
        let recording = self
            .recording_retained_frames
            .pop()
            .expect("end_retained_layer without start_retained_layer");
        self.layers.pop_clip();
        let end = self.layers.active_count();

        self.active_retained_frames.push(ActiveRetainedFrame {
            key: recording.key,
            revision: recording.revision,
            bounds: recording.bounds,
            transformation: recording.transformation,
            opacity: recording.opacity,
            scale_factor: recording.scale_factor,
            range: recording.start..end,
        });
    }

    fn retained_layer_statistics(&self) -> core::renderer::RetainedLayerStatistics {
        self.retained_layer_statistics
    }

    fn allocate_image(
        &mut self,
        _handle: &core::image::Handle,
        _callback: impl FnOnce(Result<core::image::Allocation, core::image::Error>) + Send + 'static,
    ) {
        #[cfg(feature = "image")]
        self.image_cache
            .get_mut()
            .allocate_image(_handle, _callback);
    }

    fn hint(&mut self, scale_factor: f32) {
        self.scale_factor = Some(scale_factor);
    }

    fn scale_factor(&self) -> Option<f32> {
        Some(self.scale_factor? * self.layers.transformation().scale_factor())
    }

    fn set_light(&mut self, light: core::material::SceneLight) {
        self.light = light;
    }

    fn light(&self) -> core::material::SceneLight {
        self.light
    }

    fn tick(&mut self) {
        #[cfg(feature = "image")]
        self.image_cache.get_mut().receive();
    }

    fn reset(&mut self, new_bounds: Rectangle) {
        if !self.active_retained_frames.is_empty() {
            let active =
                innermost_retained_frames(std::mem::take(&mut self.active_retained_frames));
            let ranges = active
                .iter()
                .map(|frame| frame.range.clone())
                .collect::<Vec<_>>();
            let display_lists = self.layers.take_ranges(&ranges, new_bounds);

            for (active, display_list) in active.into_iter().zip(display_lists) {
                let bytes = display_list.estimated_bytes();
                let evictions = self.retained_frames.insert(
                    active.key,
                    RetainedFrame {
                        key: active.key,
                        revision: active.revision,
                        bounds: active.bounds,
                        transformation: active.transformation,
                        opacity: active.opacity,
                        scale_factor: active.scale_factor,
                        display_list,
                    },
                    bytes,
                );
                self.retained_layer_statistics.evictions += evictions as u64;
            }
        } else {
            self.layers.reset(new_bounds);
        }
    }
}

impl core::text::Renderer for Renderer {
    type Font = Font;
    type Paragraph = Paragraph;
    type Editor = Editor;

    const ICON_FONT: Font = Font::new("Taim-Icons");
    const CHECKMARK_ICON: char = '\u{f00c}';
    const ARROW_DOWN_ICON: char = '\u{e800}';
    const TAIM_LOGO: char = '\u{e801}';
    const SCROLL_UP_ICON: char = '\u{e802}';
    const SCROLL_DOWN_ICON: char = '\u{e803}';
    const SCROLL_LEFT_ICON: char = '\u{e804}';
    const SCROLL_RIGHT_ICON: char = '\u{e805}';

    fn default_font(&self) -> Self::Font {
        self.settings.default_font
    }

    fn default_size(&self) -> Pixels {
        self.settings.default_text_size
    }

    fn fill_paragraph(
        &mut self,
        text: &Self::Paragraph,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let color = color.scale_alpha(self.layers.opacity());
        let (layer, transformation) = self.layers.current_mut();

        layer.draw_paragraph(text, position, color, clip_bounds, transformation);
    }

    fn fill_editor(
        &mut self,
        editor: &Self::Editor,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let color = color.scale_alpha(self.layers.opacity());
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_editor(editor, position, color, clip_bounds, transformation);
    }

    fn fill_text(
        &mut self,
        text: core::Text,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let color = color.scale_alpha(self.layers.opacity());
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_text(text, position, color, clip_bounds, transformation);
    }
}

impl graphics::text::Renderer for Renderer {
    fn fill_raw(&mut self, mut raw: graphics::text::Raw) {
        raw.color = raw.color.scale_alpha(self.layers.opacity());
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_text_raw(raw, transformation);
    }
}

#[cfg(feature = "image")]
impl core::image::Renderer for Renderer {
    type Handle = core::image::Handle;

    fn load_image(
        &self,
        handle: &Self::Handle,
    ) -> Result<core::image::Allocation, core::image::Error> {
        self.image_cache
            .borrow_mut()
            .load_image(&self.engine.device, &self.engine.queue, handle)
    }

    fn measure_image(&self, handle: &Self::Handle) -> Option<core::Size<u32>> {
        self.image_cache.borrow_mut().measure_image(handle)
    }

    fn draw_image(&mut self, mut image: core::Image, bounds: Rectangle, clip_bounds: Rectangle) {
        image.opacity *= self.layers.opacity();
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_raster(image, bounds, clip_bounds, transformation);
    }
}

#[cfg(feature = "svg")]
impl core::svg::Renderer for Renderer {
    fn measure_svg(&self, handle: &core::svg::Handle) -> core::Size<u32> {
        self.image_cache.borrow_mut().measure_svg(handle)
    }

    fn draw_svg(&mut self, mut svg: core::Svg, bounds: Rectangle, clip_bounds: Rectangle) {
        svg.opacity *= self.layers.opacity();
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_svg(svg, bounds, clip_bounds, transformation);
    }
}

impl graphics::mesh::Renderer for Renderer {
    fn draw_mesh(&mut self, mesh: graphics::Mesh) {
        debug_assert!(
            !mesh.indices().is_empty(),
            "Mesh must not have empty indices"
        );

        debug_assert!(
            mesh.indices().len().is_multiple_of(3),
            "Mesh indices length must be a multiple of 3"
        );

        let opacity = self.layers.opacity();
        if !opacity.is_finite() || opacity <= 0.0 {
            return;
        }

        let mesh = mesh.scale_alpha(opacity);
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_mesh(mesh, transformation);
    }

    fn draw_mesh_cache(&mut self, cache: mesh::Cache) {
        let opacity = self.layers.opacity();
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_mesh_cache(cache, transformation, opacity);
    }
}

#[cfg(feature = "geometry")]
impl graphics::geometry::Renderer for Renderer {
    type Geometry = Geometry;
    type Frame = geometry::Frame;

    fn new_frame(&self, bounds: Rectangle) -> Self::Frame {
        geometry::Frame::new(bounds)
    }

    fn draw_geometry(&mut self, geometry: Self::Geometry) {
        let opacity = self.layers.opacity();
        if !opacity.is_finite() || opacity <= 0.0 {
            return;
        }

        let (layer, transformation) = self.layers.current_mut();

        match geometry {
            Geometry::Live {
                meshes,
                images,
                text,
            } => {
                layer.draw_mesh_group(
                    meshes
                        .into_iter()
                        .map(|mesh| mesh.scale_alpha(opacity))
                        .collect(),
                    transformation,
                );

                for image in images {
                    layer.draw_image(image.scale_alpha(opacity), transformation);
                }

                layer.draw_text_group(
                    text.into_iter()
                        .map(|text| text.scale_alpha(opacity))
                        .collect(),
                    transformation,
                );
            }
            Geometry::Cached(cache) => {
                if let Some(meshes) = cache.meshes {
                    layer.draw_mesh_cache(meshes, transformation, opacity);
                }

                if let Some(images) = cache.images {
                    for image in images.iter().cloned() {
                        layer.draw_image(image.scale_alpha(opacity), transformation);
                    }
                }

                if let Some(text) = cache.text {
                    layer.draw_text_cache(text.scale_alpha(opacity), transformation);
                }
            }
        }
    }
}

impl primitive::Renderer for Renderer {
    fn draw_primitive(&mut self, bounds: Rectangle, primitive: impl Primitive) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_primitive(bounds, primitive, transformation);
    }
}

impl graphics::compositor::Default for crate::Renderer {
    type Compositor = window::Compositor;
}

impl graphics::text::CacheStatistics for Renderer {
    fn text_cache_statistics(&self) -> Option<graphics::text::cache::Statistics> {
        Some(self.text.cache_statistics())
    }
}

impl renderer::Headless for Renderer {
    async fn new(settings: renderer::Settings, backend: Option<&str>) -> Option<Self> {
        if backend.is_some_and(|backend| backend != "wgpu") {
            return None;
        }

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY),
            flags: wgpu::InstanceFlags::empty(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok()?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("taim_wgpu [headless]"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits {
                    max_bind_groups: 2,
                    ..wgpu::Limits::default()
                },
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .await
            .ok()?;

        let engine = Engine::new(
            &adapter,
            device,
            queue,
            if graphics::color::GAMMA_CORRECTION {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
            Some(graphics::Antialiasing::MSAAx4),
            Shell::headless(),
        );

        Some(Self::new(engine, settings))
    }

    fn name(&self) -> String {
        "wgpu".to_owned()
    }

    fn screenshot(
        &mut self,
        size: Size<u32>,
        scale_factor: f32,
        background_color: Color,
    ) -> Vec<u8> {
        self.screenshot(
            &Viewport::with_physical_size(size, scale_factor),
            background_color,
        )
    }
}
