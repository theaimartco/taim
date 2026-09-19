use crate::{Bytes, Size};

use rustc_hash::FxHasher;

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

/// A stable reference to raster image data.
#[derive(Clone, PartialEq, Eq)]
pub enum Handle {
    /// Data loaded from a filesystem path.
    Path(Id, PathBuf),
    /// Encoded image bytes.
    Bytes(Id, Bytes),
    /// Decoded RGBA pixels.
    Rgba {
        /// Stable content identifier.
        id: Id,
        /// Pixel width.
        width: u32,
        /// Pixel height.
        height: u32,
        /// RGBA pixels.
        pixels: Bytes,
    },
}

impl Handle {
    /// Creates a path-backed handle.
    pub fn from_path<T: Into<PathBuf>>(path: T) -> Handle {
        let path = path.into();
        Self::Path(Id::path(&path), path)
    }

    /// Creates an encoded, content-addressed handle.
    pub fn from_bytes(bytes: impl Into<Bytes>) -> Handle {
        let bytes = bytes.into();
        Self::Bytes(Id::bytes(&bytes), bytes)
    }

    /// Creates a decoded, content-addressed RGBA handle.
    pub fn from_rgba(width: u32, height: u32, pixels: impl Into<Bytes>) -> Handle {
        let pixels = pixels.into();

        Self::Rgba {
            id: Id::rgba(width, height, &pixels),
            width,
            height,
            pixels,
        }
    }

    /// Returns the stable identifier of this handle.
    pub fn id(&self) -> Id {
        match self {
            Handle::Path(id, _) | Handle::Bytes(id, _) | Handle::Rgba { id, .. } => *id,
        }
    }
}

impl<T> From<T> for Handle
where
    T: Into<PathBuf>,
{
    fn from(path: T) -> Handle {
        Handle::from_path(path)
    }
}

impl From<&Handle> for Handle {
    fn from(value: &Handle) -> Self {
        value.clone()
    }
}

impl std::fmt::Debug for Handle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(id, path) => write!(formatter, "Path({id:?}, {path:?})"),
            Self::Bytes(id, _) => write!(formatter, "Bytes({id:?}, ...)"),
            Self::Rgba {
                id, width, height, ..
            } => write!(formatter, "Pixels({id:?}, {width} * {height})"),
        }
    }
}

/// A stable image identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(ContentId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum ContentId {
    Hash(u64),
}

impl Id {
    fn path(path: impl AsRef<Path>) -> Self {
        let mut hasher = FxHasher::default();
        hasher.write_u8(0);
        path.as_ref().hash(&mut hasher);
        Self(ContentId::Hash(hasher.finish()))
    }

    fn bytes(bytes: &Bytes) -> Self {
        let mut hasher = FxHasher::default();
        hasher.write_u8(1);
        bytes.hash(&mut hasher);
        Self(ContentId::Hash(hasher.finish()))
    }

    fn rgba(width: u32, height: u32, pixels: &Bytes) -> Self {
        let mut hasher = FxHasher::default();
        hasher.write_u8(2);
        width.hash(&mut hasher);
        height.hash(&mut hasher);
        pixels.hash(&mut hasher);
        Self(ContentId::Hash(hasher.finish()))
    }
}

/// A retained renderer allocation for an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allocation(Arc<Memory>);

/// The handle and dimensions retained by an [`Allocation`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Memory {
    handle: Handle,
    size: Size<u32>,
}

impl Allocation {
    /// Returns a weak reference to the retained memory.
    pub fn downgrade(&self) -> Weak<Memory> {
        Arc::downgrade(&self.0)
    }

    /// Attempts to retain a weak memory reference.
    pub fn upgrade(weak: &Weak<Memory>) -> Option<Allocation> {
        Weak::upgrade(weak).map(Allocation)
    }

    /// Returns the allocated image handle.
    pub fn handle(&self) -> &Handle {
        &self.0.handle
    }

    /// Returns the allocated dimensions.
    pub fn size(&self) -> Size<u32> {
        self.0.size
    }
}

/// Creates a retained allocation after a renderer has uploaded the handle.
///
/// # Safety
/// The handle must already be allocated in renderer memory.
#[allow(unsafe_code)]
pub unsafe fn allocate(handle: &Handle, size: Size<u32>) -> Allocation {
    Allocation(Arc::new(Memory {
        handle: handle.clone(),
        size,
    }))
}

/// Creates metadata-only storage for the debug null renderer.
#[cfg(debug_assertions)]
pub(crate) fn placeholder(handle: &Handle, size: Size<u32>) -> Allocation {
    Allocation(Arc::new(Memory {
        handle: handle.clone(),
        size,
    }))
}
