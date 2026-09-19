use super::{Pipeline, Primitive};

use rustc_hash::FxHashMap;

use std::any::{Any, TypeId};
use std::collections::hash_map::Entry;

/// Type-indexed custom primitive pipelines for the active target format.
#[derive(Default)]
pub struct Storage {
    active_format: Option<wgpu::TextureFormat>,
    pipelines: FxHashMap<(TypeId, Option<wgpu::TextureFormat>), Box<dyn Pipeline>>,
}

impl Storage {
    pub(crate) fn pipeline_mut<P: Primitive>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> &mut P::Pipeline {
        self.active_format = Some(format);
        let key = (TypeId::of::<P>(), self.active_format);

        let pipeline = match self.pipelines.entry(key) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(Box::new(P::Pipeline::new(device, queue, format))),
        };

        (pipeline.as_mut() as &mut dyn Any)
            .downcast_mut()
            .expect("primitive pipeline type must match its primitive key")
    }

    pub(crate) fn pipeline<P: Primitive>(&self) -> &P::Pipeline {
        self.get::<P>()
            .and_then(|pipeline| pipeline.downcast_ref())
            .expect("primitive pipeline must be prepared before drawing")
    }

    /// Returns whether a value exists for type key `T`.
    pub fn has<T: 'static>(&self) -> bool {
        self.pipelines
            .contains_key(&(TypeId::of::<T>(), self.active_format))
    }

    /// Stores a pipeline under type key `T`.
    pub fn store<T: 'static, P: Pipeline>(&mut self, pipeline: P) {
        let _ = self
            .pipelines
            .insert((TypeId::of::<T>(), self.active_format), Box::new(pipeline));
    }

    /// Returns a stored pipeline through dynamic typing.
    pub fn get<T: 'static>(&self) -> Option<&dyn Any> {
        self.pipelines
            .get(&(TypeId::of::<T>(), self.active_format))
            .map(|pipeline| pipeline.as_ref() as &dyn Any)
    }

    /// Returns mutable access to a stored pipeline through dynamic typing.
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut dyn Any> {
        self.pipelines
            .get_mut(&(TypeId::of::<T>(), self.active_format))
            .map(|pipeline| pipeline.as_mut() as &mut dyn Any)
    }

    /// Drops all custom pipelines and their cached GPU resources.
    pub fn clear(&mut self) {
        self.pipelines.clear();
        self.active_format = None;
    }

    /// Trims cached data in every custom pipeline.
    pub fn trim(&mut self) {
        for pipeline in self.pipelines.values_mut() {
            pipeline.trim();
        }
    }
}
