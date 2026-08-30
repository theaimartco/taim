//! Decode, upload, resize, and eviction workload.
//!
//! Images are synthetic, deterministic RGBA buffers built in memory via
//! [`taim_core::image::Handle::from_rgba`] rather than files on disk, so
//! this scene has no external asset dependency and produces byte-
//! identical input on every run and platform. This intentionally skips
//! real codec decode cost (JPEG/PNG); Phase 5's
//! `wgpu/src/image/decode.rs` is where decode-specific budgets belong.

use crate::scenes::{Element, Scene};

use taim_core::Length;
use taim_core::image::Handle;
use taim_widget::{column, container, image, row, scrollable};

/// Command-line/report identifier for this scene.
pub const NAME: &str = "image_grid";

/// Total number of distinct synthetic images in the grid's backing pool.
/// A grid larger than the visible window exercises eviction of
/// off-screen image resources once such a cache exists (Phase 5's
/// `wgpu/src/image/cache.rs`); Phase 1 measures the pre-cache baseline.
const IMAGE_COUNT: usize = 64;

/// Side length, in pixels, of each synthetic image.
const IMAGE_SIZE: u32 = 96;

/// Columns per row in the grid.
const COLUMNS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// Scrolls the grid forward, bringing new images into view.
    ScrollForward,
}

/// A scrollable grid of synthetic images.
#[derive(Debug)]
pub struct ImageGrid {
    handles: Vec<Handle>,
    scroll_ticks: u64,
}

impl ImageGrid {
    /// Builds `IMAGE_COUNT` distinct solid-color images, each derived
    /// deterministically from its index so two runs produce identical
    /// pixel buffers.
    pub fn new() -> Self {
        let handles = (0..IMAGE_COUNT)
            .map(|index| Handle::from_rgba(IMAGE_SIZE, IMAGE_SIZE, synthetic_pixels(index)))
            .collect();

        Self {
            handles,
            scroll_ticks: 0,
        }
    }
}

impl Default for ImageGrid {
    fn default() -> Self {
        Self::new()
    }
}

/// Generates a solid RGBA buffer whose color is derived from `index`, so
/// each image is visually distinct and reproducible without any file I/O.
fn synthetic_pixels(index: usize) -> Vec<u8> {
    let r = ((index * 37) % 256) as u8;
    let g = ((index * 61) % 256) as u8;
    let b = ((index * 89) % 256) as u8;

    let pixel_count = (IMAGE_SIZE * IMAGE_SIZE) as usize;
    let mut pixels = Vec::with_capacity(pixel_count * 4);

    for _ in 0..pixel_count {
        pixels.extend_from_slice(&[r, g, b, 255]);
    }

    pixels
}

impl Scene for ImageGrid {
    type Message = Message;

    fn name(&self) -> &'static str {
        NAME
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let rows = self.handles.chunks(COLUMNS).map(|chunk| {
            Element::from(
                row(chunk
                    .iter()
                    .map(|handle| Element::from(image(handle.clone()).width(IMAGE_SIZE as f32))))
                .spacing(8),
            )
        });

        container(scrollable(column(rows).width(Length::Fill).spacing(8)).height(Length::Fill))
            .padding(8)
            .into()
    }

    fn update(&mut self, message: Self::Message) {
        match message {
            Message::ScrollForward => {
                self.scroll_ticks += 1;
            }
        }
    }

    fn advance(&mut self, _frame_index: u64) -> Vec<Self::Message> {
        vec![Message::ScrollForward]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_pixels_are_deterministic() {
        assert_eq!(synthetic_pixels(5), synthetic_pixels(5));
        assert_ne!(synthetic_pixels(5), synthetic_pixels(6));
    }

    #[test]
    fn synthetic_pixels_have_the_expected_length() {
        let pixels = synthetic_pixels(0);
        assert_eq!(pixels.len(), (IMAGE_SIZE * IMAGE_SIZE * 4) as usize);
    }

    #[test]
    fn grid_builds_the_configured_image_count() {
        let grid = ImageGrid::new();
        assert_eq!(grid.handles.len(), IMAGE_COUNT);
    }
}
