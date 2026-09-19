use super::Mesh;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Immutable, shareable mesh storage with a stable GPU identity.
#[derive(Debug, Clone)]
pub struct Cache {
    id: Id,
    batch: Arc<[Mesh]>,
    version: usize,
}

/// The unique id of a [`Cache`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(u64);

impl Cache {
    /// Creates a cache around the provided shared mesh batch.
    pub fn new(batch: Arc<[Mesh]>) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);

        Self {
            id: Id(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
            batch,
            version: 0,
        }
    }

    /// Returns the stable identity consumed by renderer-side storage.
    pub fn id(&self) -> Id {
        self.id
    }

    /// Returns the content generation consumed by renderer-side storage.
    pub fn version(&self) -> usize {
        self.version
    }

    /// Borrows the shared mesh batch without touching its reference count.
    pub fn batch(&self) -> &Arc<[Mesh]> {
        &self.batch
    }

    /// Returns whether the batch contains no mesh.
    pub fn is_empty(&self) -> bool {
        self.batch.is_empty()
    }

    /// Replaces the shared batch and advances the content generation.
    ///
    /// Reusing the same allocation is a steady-state no-op, which prevents a
    /// redundant GPU upload when a retained geometry cache is passed through
    /// an unchanged frame.
    pub fn update(&mut self, batch: Arc<[Mesh]>) {
        if Arc::ptr_eq(&self.batch, &batch) {
            return;
        }

        self.batch = batch;
        self.version = self.version.wrapping_add(1);
    }

    /// Builds an alpha-adjusted cache when CPU-side scaling is explicitly
    /// requested. Fully opaque scaling keeps the original identity and shared
    /// storage; the WGPU path applies dynamic opacity in its uniform instead.
    pub fn scale_alpha(&self, factor: f32) -> Self {
        let factor = normalise_opacity(factor);
        if factor == 1.0 {
            return self.clone();
        }

        let batch = self
            .batch
            .iter()
            .cloned()
            .map(|mesh| mesh.scale_alpha(factor))
            .collect::<Vec<_>>();

        Self::new(batch.into())
    }
}

#[inline]
fn normalise_opacity(opacity: f32) -> f32 {
    if opacity.is_finite() {
        opacity.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_scaling_keeps_identity_and_storage() {
        let meshes: Arc<[Mesh]> = Arc::from([]);
        let cache = Cache::new(Arc::clone(&meshes));
        let scaled = cache.scale_alpha(1.0);

        assert_eq!(scaled.id(), cache.id());
        assert_eq!(scaled.version(), cache.version());
        assert!(Arc::ptr_eq(scaled.batch(), cache.batch()));
    }

    #[test]
    fn same_storage_does_not_invalidate_gpu_generation() {
        let meshes: Arc<[Mesh]> = Arc::from([]);
        let mut cache = Cache::new(Arc::clone(&meshes));
        cache.update(meshes);

        assert_eq!(cache.version(), 0);
    }

    #[test]
    fn invalid_opacity_is_made_transparent() {
        assert_eq!(normalise_opacity(f32::NAN), 0.0);
        assert_eq!(normalise_opacity(f32::INFINITY), 0.0);
        assert_eq!(normalise_opacity(-1.0), 0.0);
        assert_eq!(normalise_opacity(2.0), 1.0);
    }
}
