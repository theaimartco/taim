use super::draw::Layer;
use super::pipeline::{gradient, solid};

use crate::core::Transformation;
use crate::graphics::mesh::{self, Mesh};

use rustc_hash::FxHashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;

#[derive(Debug)]
struct Retained {
    layer: Layer,
    transformation: Transformation,
    opacity: f32,
    version: usize,
    meshes: Arc<[Mesh]>,
}

/// Device-resident mesh cache keyed by TAIM mesh identity and version.
#[derive(Debug, Default)]
pub struct Storage {
    retained: FxHashMap<mesh::Id, Retained>,
}

impl Storage {
    pub fn new() -> Self {
        Self::default()
    }

    pub(super) fn get(&self, cache: &mesh::Cache) -> Option<(&Layer, &Arc<[Mesh]>)> {
        if cache.is_empty() {
            return None;
        }
        self.retained
            .get(&cache.id())
            .map(|retained| (&retained.layer, &retained.meshes))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        solid: &solid::Pipeline,
        gradient: &gradient::Pipeline,
        cache: &mesh::Cache,
        transformation: Transformation,
        opacity: f32,
    ) {
        if cache.is_empty() {
            let _ = self.retained.remove(&cache.id());
            return;
        }

        match self.retained.entry(cache.id()) {
            Entry::Occupied(entry) => {
                let retained = entry.into_mut();
                if retained.version != cache.version() {
                    retained.layer.prepare_geometry(
                        device,
                        encoder,
                        belt,
                        solid,
                        gradient,
                        cache.batch(),
                        transformation,
                        opacity,
                    );
                    retained.meshes = Arc::clone(cache.batch());
                    retained.version = cache.version();
                    retained.transformation = transformation;
                    retained.opacity = opacity;
                } else if retained.transformation != transformation || retained.opacity != opacity {
                    retained.layer.prepare_transforms(
                        encoder,
                        belt,
                        cache.batch(),
                        transformation,
                        opacity,
                    );
                    retained.transformation = transformation;
                    retained.opacity = opacity;
                }
            }
            Entry::Vacant(entry) => {
                let mut layer = Layer::new(device, solid, gradient);
                layer.prepare_geometry(
                    device,
                    encoder,
                    belt,
                    solid,
                    gradient,
                    cache.batch(),
                    transformation,
                    opacity,
                );
                let _ = entry.insert(Retained {
                    layer,
                    transformation,
                    opacity,
                    version: cache.version(),
                    meshes: Arc::clone(cache.batch()),
                });
            }
        }
    }

    pub fn trim(&mut self) {
        self.retained
            .retain(|_, retained| Arc::strong_count(&retained.meshes) > 1);
    }
}
