use super::Text;
use crate::graphics::cache;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone)]
pub struct Cache {
    pub(super) id: Id,
    pub(super) group: cache::Group,
    pub(super) text: Arc<[Text]>,
    pub(super) version: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub(super) u64);

impl Cache {
    pub fn new(group: cache::Group, text: Vec<Text>) -> Option<Self> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);

        if text.is_empty() {
            return None;
        }

        Some(Self {
            id: Id(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
            group,
            text: text.into(),
            version: 0,
        })
    }

    pub fn update(&mut self, text: Vec<Text>) {
        if self.text.as_ref() == text.as_slice() {
            return;
        }

        self.text = text.into();
        self.version = self.version.wrapping_add(1);
    }

    pub fn scale_alpha(mut self, factor: f32) -> Self {
        if factor == 1.0 {
            return self;
        }

        let scaled: Vec<_> = self
            .text
            .iter()
            .cloned()
            .map(|text| text.scale_alpha(factor))
            .collect();

        if self.text.as_ref() != scaled.as_slice() {
            self.text = scaled.into();
            self.version = self.version.wrapping_add(1);
        }

        self
    }
}
