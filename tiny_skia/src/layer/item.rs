use crate::core::renderer;
use crate::core::{Background, Rectangle, Transformation};

use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Quad {
    pub primitive: renderer::Quad,
    pub background: Background,
    pub transformation: Transformation,
}

#[derive(Debug, Clone)]
pub(crate) enum DrawBatch<T> {
    One {
        value: T,
        clip_bounds: Rectangle,
        transformation: Transformation,
    },
    Owned {
        values: Vec<T>,
        clip_bounds: Rectangle,
        transformation: Transformation,
    },
    Shared {
        values: Arc<[T]>,
        clip_bounds: Rectangle,
        transformation: Transformation,
    },
}

impl<T> DrawBatch<T> {
    pub fn one(value: T, clip_bounds: Rectangle, transformation: Transformation) -> Self {
        Self::One {
            value,
            clip_bounds,
            transformation,
        }
    }

    pub fn owned(values: Vec<T>, clip_bounds: Rectangle, transformation: Transformation) -> Self {
        Self::Owned {
            values,
            clip_bounds,
            transformation,
        }
    }

    pub fn shared(
        values: Arc<[T]>,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) -> Self {
        Self::Shared {
            values,
            clip_bounds,
            transformation,
        }
    }

    #[inline]
    pub fn transformation(&self) -> Transformation {
        match self {
            Self::One { transformation, .. }
            | Self::Owned { transformation, .. }
            | Self::Shared { transformation, .. } => *transformation,
        }
    }

    #[inline]
    pub fn clip_bounds(&self) -> Rectangle {
        match self {
            Self::One { clip_bounds, .. }
            | Self::Owned { clip_bounds, .. }
            | Self::Shared { clip_bounds, .. } => *clip_bounds,
        }
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        match self {
            Self::One { value, .. } => std::slice::from_ref(value),
            Self::Owned { values, .. } => values,
            Self::Shared { values, .. } => values,
        }
    }

    pub fn retained_bytes(&self) -> usize {
        match self {
            Self::One { .. } => 0,
            Self::Owned { values, .. } => values.capacity() * std::mem::size_of::<T>(),
            Self::Shared { values, .. } => values.len() * std::mem::size_of::<T>(),
        }
    }
}

impl<T: PartialEq> DrawBatch<T> {
    pub fn unchanged(&self, other: &Self) -> bool {
        if self.clip_bounds() != other.clip_bounds()
            || self.transformation() != other.transformation()
        {
            return false;
        }

        match (self, other) {
            (Self::Shared { values: a, .. }, Self::Shared { values: b, .. }) => Arc::ptr_eq(a, b),
            _ => self.as_slice() == other.as_slice(),
        }
    }
}
