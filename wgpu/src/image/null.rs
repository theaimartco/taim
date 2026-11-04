//! TAIM no-image renderer with observable suppressed submission accounting.
//!
//! This module is compiled when both raster and SVG support are disabled.
pub use crate::graphics::Image;

/// Workloads remain non-renderable, but lost primitives are measurable.
#[derive(Debug, Default)]
pub struct Batch {
    suppressed: usize,
}

impl Batch {
    /// Discard unsupported image data without retaining its allocation.
    pub fn push(&mut self, _image: Image) {
        self.suppressed = self.suppressed.saturating_add(1);
    }

    /// Reset the diagnostic counter after flushing this render layer.
    pub fn clear(&mut self) {
        self.suppressed = 0;
    }

    /// No raster/SVG primitives are ever sent to the GPU in this build.
    pub fn is_empty(&self) -> bool {
        true
    }

    /// Merge suppressed-submission counts and reset the source.
    pub fn append(&mut self, batch: &mut Self) {
        self.suppressed = self.suppressed.saturating_add(batch.suppressed);
        batch.clear();
    }

    /// Number of requested images that this backend discarded.
    pub fn suppressed_count(&self) -> usize {
        self.suppressed
    }
}

#[cfg(test)]
mod taim_no_image_tests {
    use super::*;

    #[test]
    fn suppressed_assets_count_without_fake_gpu_batches() {
        let mut first = Batch::default();
        let mut second = Batch { suppressed: 3 };
        first.append(&mut second);
        assert_eq!(first.suppressed_count(), 3);
        assert_eq!(second.suppressed_count(), 0);
        assert!(first.is_empty());
        first.clear();
        assert_eq!(first.suppressed_count(), 0);
    }
}
