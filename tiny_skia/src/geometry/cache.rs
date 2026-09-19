use crate::Primitive;
use crate::core::Rectangle;
use crate::graphics::cache::{self, Cached};
use crate::graphics::{Image, Text};

use std::sync::Arc;

#[derive(Debug)]
pub enum Geometry {
    Live {
        text: Vec<Text>,
        images: Vec<Image>,
        primitives: Vec<Primitive>,
        clip_bounds: Rectangle,
    },
    Cache(Cache),
}

#[derive(Debug, Clone)]
pub struct Cache {
    pub text: Arc<[Text]>,
    pub images: Arc<[Image]>,
    pub primitives: Arc<[Primitive]>,
    pub clip_bounds: Rectangle,
}

impl Cached for Geometry {
    type Cache = Cache;

    fn load(cache: &Cache) -> Self {
        Self::Cache(cache.clone())
    }

    fn cache(self, _group: cache::Group, previous: Option<Cache>) -> Cache {
        let Self::Live {
            primitives,
            images,
            text,
            clip_bounds,
        } = self
        else {
            let Self::Cache(cache) = self else {
                unreachable!()
            };
            return cache;
        };

        let Some(previous) = previous else {
            return Cache {
                primitives: primitives.into(),
                images: images.into(),
                text: text.into(),
                clip_bounds,
            };
        };

        Cache {
            primitives: retain_equal(previous.primitives, primitives),
            images: retain_equal(previous.images, images),
            text: retain_equal(previous.text, text),
            clip_bounds,
        }
    }
}

fn retain_equal<T: PartialEq>(previous: Arc<[T]>, current: Vec<T>) -> Arc<[T]> {
    if previous.as_ref() == current.as_slice() {
        previous
    } else {
        current.into()
    }
}
