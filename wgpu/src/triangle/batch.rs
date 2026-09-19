use super::draw::Layer;
use super::{Pipeline, Storage};

use crate::core::{Rectangle, Size, Transformation};
use crate::graphics::mesh::{self, Mesh};

pub type Batch = Vec<Item>;

#[derive(Debug)]
pub enum Item {
    Group {
        transformation: Transformation,
        meshes: Vec<Mesh>,
    },
    Cached {
        transformation: Transformation,
        opacity: f32,
        cache: mesh::Cache,
    },
}

pub struct State {
    pipeline_id: u64,
    msaa: Option<super::msaa::State>,
    transient: Vec<Layer>,
    transient_count: usize,
    retained: Storage,
}

impl State {
    pub fn new(device: &wgpu::Device, pipeline: &Pipeline) -> Self {
        Self {
            pipeline_id: pipeline.id,
            msaa: pipeline
                .msaa
                .as_ref()
                .map(|pipeline| super::msaa::State::new(device, pipeline)),
            transient: Vec::new(),
            transient_count: 0,
            retained: Storage::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        pipeline: &Pipeline,
        device: &wgpu::Device,
        belt: &mut wgpu::util::StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
        items: &[Item],
        scale: Transformation,
        target_size: Size<u32>,
    ) {
        if self.pipeline_id != pipeline.id {
            self.pipeline_id = pipeline.id;
            self.msaa = pipeline
                .msaa
                .as_ref()
                .map(|pipeline| super::msaa::State::new(device, pipeline));
            self.transient.clear();
            self.transient_count = 0;
            self.retained = Storage::new();
        }

        let projection = self
            .msaa
            .as_mut()
            .zip(pipeline.msaa.as_ref())
            .map(|(state, pipeline)| {
                state.prepare(device, encoder, belt, pipeline, target_size) * scale
            })
            .unwrap_or_else(|| {
                Transformation::orthographic(target_size.width, target_size.height) * scale
            });

        for item in items {
            match item {
                Item::Group {
                    transformation,
                    meshes,
                } => {
                    if self.transient.len() == self.transient_count {
                        self.transient.push(Layer::new(
                            device,
                            &pipeline.solid,
                            &pipeline.gradient,
                        ));
                    }
                    self.transient[self.transient_count].prepare_geometry(
                        device,
                        encoder,
                        belt,
                        &pipeline.solid,
                        &pipeline.gradient,
                        meshes,
                        projection * *transformation,
                        1.0,
                    );
                    self.transient_count += 1;
                }
                Item::Cached {
                    transformation,
                    opacity,
                    cache,
                } => self.retained.prepare(
                    device,
                    encoder,
                    belt,
                    &pipeline.solid,
                    &pipeline.gradient,
                    cache,
                    projection * *transformation,
                    *opacity,
                ),
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        pipeline: &Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        start: usize,
        batch: &Batch,
        bounds: Rectangle,
        screen_transformation: Transformation,
    ) -> usize {
        let mut transient_offset = 0;
        let layers = batch.iter().filter_map(|item| match item {
            Item::Group { transformation, .. } => {
                let layer = &self.transient[start + transient_offset];
                transient_offset += 1;
                Some((layer, screen_transformation * *transformation))
            }
            Item::Cached {
                transformation,
                cache,
                ..
            } => self
                .retained
                .get(cache)
                .map(|(layer, _)| (layer, screen_transformation * *transformation)),
        });

        render_layers(
            encoder,
            target,
            self.msaa.as_ref().zip(pipeline.msaa.as_ref()),
            pipeline,
            bounds,
            layers,
        );
        transient_offset
    }

    pub fn trim(&mut self) {
        self.retained.trim();
        self.transient_count = 0;
    }
}

fn render_layers<'a>(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    mut msaa: Option<(&super::msaa::State, &super::msaa::Pipeline)>,
    pipeline: &'a Pipeline,
    bounds: Rectangle,
    layers: impl Iterator<Item = (&'a Layer, Transformation)>,
) {
    {
        let mut pass = if let Some((state, _)) = &mut msaa {
            state.render_pass(encoder)
        } else {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("taim.triangle.pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            })
        };

        for (layer, transformation) in layers {
            layer.render(
                &pipeline.solid,
                &pipeline.gradient,
                bounds,
                transformation,
                &mut pass,
            );
        }
    }

    if let Some((state, pipeline)) = msaa {
        state.render(pipeline, encoder, target);
    }
}
