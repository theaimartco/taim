use crate::graphics::cache::{self, Cached};
use crate::graphics::mesh;
use crate::graphics::{Image, Text};
use crate::text;

use std::sync::Arc;

#[derive(Debug)]
pub enum Geometry {
    Live {
        meshes: Vec<mesh::Mesh>,
        images: Vec<Image>,
        text: Vec<Text>,
    },
    Cached(Cache),
}

#[derive(Debug, Clone, Default)]
pub struct Cache {
    pub meshes: Option<mesh::Cache>,
    pub images: Option<Arc<[Image]>>,
    pub text: Option<text::Cache>,
}

impl Cached for Geometry {
    type Cache = Cache;

    fn load(cache: &Cache) -> Self {
        Self::Cached(cache.clone())
    }

    fn cache(self, group: cache::Group, previous: Option<Cache>) -> Cache {
        let Self::Live {
            meshes,
            images,
            text,
        } = self
        else {
            let Self::Cached(cache) = self else {
                unreachable!()
            };
            return cache;
        };

        let meshes: Arc<[mesh::Mesh]> = meshes.into();
        let images: Arc<[Image]> = images.into();
        let Some(mut cache) = previous else {
            return Cache {
                meshes: (!meshes.is_empty()).then(|| mesh::Cache::new(meshes)),
                images: (!images.is_empty()).then_some(images),
                text: text::Cache::new(group, text),
            };
        };

        cache.meshes = if meshes.is_empty() {
            None
        } else if let Some(mut retained) = cache.meshes.take() {
            if retained.batch().as_ref() != meshes.as_ref() {
                retained.update(meshes);
            }
            Some(retained)
        } else {
            Some(mesh::Cache::new(meshes))
        };

        cache.images = if images.is_empty() {
            None
        } else if cache
            .images
            .as_ref()
            .is_some_and(|retained| retained.as_ref() == images.as_ref())
        {
            cache.images
        } else {
            Some(images)
        };

        cache.text = if text.is_empty() {
            None
        } else if let Some(mut retained) = cache.text.take() {
            retained.update(text);
            Some(retained)
        } else {
            text::Cache::new(group, text)
        };
        cache
    }
}
