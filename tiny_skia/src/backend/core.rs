use crate::backend::retained::{RetainedFrame, innermost};
use crate::core::{self, Background, Rectangle, Transformation, renderer};
use crate::{ActiveRetainedFrame, RecordingRetainedFrame, Renderer};

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

    fn fill_quad(&mut self, mut quad: renderer::Quad, background: impl Into<Background>) {
        let opacity = self.layers.opacity();
        if opacity <= 0.0 {
            return;
        }

        quad.border.color = quad.border.color.scale_alpha(opacity);
        quad.shadow.color = quad.shadow.color.scale_alpha(opacity);
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_quad(quad, background.into().scale_alpha(opacity), transformation);
    }

    fn replay_retained_layer(&mut self, key: u64, revision: u64, bounds: Rectangle) -> bool {
        let transformation = self.layers.transformation();
        let opacity = self.layers.opacity();
        let retained = self.retained_frames.take(key);
        let had_cached_frame = retained.is_some();

        if let Some(retained) = retained
            && retained.key == key
            && retained.revision == revision
            && retained.bounds == bounds
            && retained.transformation == transformation
            && retained.opacity == opacity
        {
            let range = self.layers.replay_owned(retained.display_list);
            let sequence = self.active_retained_frames.len();
            self.retained_layer_statistics.hits += 1;
            self.active_retained_frames.push(ActiveRetainedFrame {
                key,
                revision,
                bounds,
                transformation,
                opacity,
                range,
                sequence,
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
        let sequence = self.active_retained_frames.len();

        self.active_retained_frames.push(ActiveRetainedFrame {
            key: recording.key,
            revision: recording.revision,
            bounds: recording.bounds,
            transformation: recording.transformation,
            opacity: recording.opacity,
            range: recording.start..end,
            sequence,
        });
    }

    fn retained_layer_statistics(&self) -> core::renderer::RetainedLayerStatistics {
        self.retained_layer_statistics
    }

    fn allocate_image(
        &mut self,
        _handle: &core::image::Handle,
        callback: impl FnOnce(Result<core::image::Allocation, core::image::Error>) + Send + 'static,
    ) {
        #[cfg(feature = "image")]
        callback(self.engine.raster_pipeline.load(_handle));

        #[cfg(not(feature = "image"))]
        callback(Err(core::image::Error::Unsupported));
    }

    fn hint(&mut self, _scale_factor: f32) {}

    fn scale_factor(&self) -> Option<f32> {
        None
    }

    fn reset(&mut self, new_bounds: Rectangle) {
        if self.active_retained_frames.is_empty() {
            self.layers.reset(new_bounds);
            return;
        }

        let active = innermost(std::mem::take(&mut self.active_retained_frames));
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
                    display_list,
                },
                bytes,
            );
            self.retained_layer_statistics.evictions += evictions as u64;
        }
    }
}
