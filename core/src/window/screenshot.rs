//! Take screenshots of a window.
use crate::{Bytes, Rectangle, Size};

use std::fmt::{Debug, Formatter};

/// Data of a screenshot, captured with `window::screenshot()`.
///
/// The `bytes` of this screenshot will always be ordered as `RGBA` in the `sRGB` color space.
#[derive(Clone)]
pub struct Screenshot {
    /// The RGBA bytes of the [`Screenshot`].
    pub rgba: Bytes,
    /// The size of the [`Screenshot`] in physical pixels.
    pub size: Size<u32>,
    /// The scale factor of the [`Screenshot`]. This can be useful when converting between widget
    /// bounds (which are in logical pixels) to crop screenshots.
    pub scale_factor: f32,
}

impl Debug for Screenshot {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Screenshot: {{ \n bytes: {}\n scale: {}\n size: {:?} }}",
            self.rgba.len(),
            self.scale_factor,
            self.size
        )
    }
}

impl Screenshot {
    /// Creates a new [`Screenshot`].
    pub fn new(rgba: impl Into<Bytes>, size: Size<u32>, scale_factor: f32) -> Self {
        Self {
            rgba: rgba.into(),
            size,
            scale_factor,
        }
    }

    /// Crops a [`Screenshot`] to the provided `region`. This will always be relative to the
    /// top-left corner of the [`Screenshot`].
    pub fn crop(&self, region: Rectangle<u32>) -> Result<Self, CropError> {
        if region.width == 0 || region.height == 0 {
            return Err(CropError::Zero);
        }

        let right = region
            .x
            .checked_add(region.width)
            .ok_or(CropError::OutOfBounds)?;
        let bottom = region
            .y
            .checked_add(region.height)
            .ok_or(CropError::OutOfBounds)?;

        if right > self.size.width || bottom > self.size.height {
            return Err(CropError::OutOfBounds);
        }

        const PIXEL_SIZE: usize = 4;
        let source_width = usize::try_from(self.size.width).map_err(|_| CropError::OutOfBounds)?;
        let source_height =
            usize::try_from(self.size.height).map_err(|_| CropError::OutOfBounds)?;
        let stride = source_width
            .checked_mul(PIXEL_SIZE)
            .ok_or(CropError::OutOfBounds)?;
        let required = stride
            .checked_mul(source_height)
            .ok_or(CropError::OutOfBounds)?;

        if self.rgba.len() < required {
            return Err(CropError::OutOfBounds);
        }

        if region.x == 0
            && region.y == 0
            && region.width == self.size.width
            && region.height == self.size.height
        {
            return Ok(self.clone());
        }

        let left = usize::try_from(region.x)
            .map_err(|_| CropError::OutOfBounds)?
            .checked_mul(PIXEL_SIZE)
            .ok_or(CropError::OutOfBounds)?;
        let row_bytes = usize::try_from(region.width)
            .map_err(|_| CropError::OutOfBounds)?
            .checked_mul(PIXEL_SIZE)
            .ok_or(CropError::OutOfBounds)?;
        let top = usize::try_from(region.y).map_err(|_| CropError::OutOfBounds)?;
        let rows = usize::try_from(region.height).map_err(|_| CropError::OutOfBounds)?;
        let capacity = row_bytes.checked_mul(rows).ok_or(CropError::OutOfBounds)?;
        let mut rgba = Vec::with_capacity(capacity);

        for row in top..top + rows {
            let start = row * stride + left;
            rgba.extend_from_slice(&self.rgba[start..start + row_bytes]);
        }

        Ok(Self {
            rgba: Bytes::from(rgba),
            size: Size::new(region.width, region.height),
            scale_factor: self.scale_factor,
        })
    }
}

impl AsRef<[u8]> for Screenshot {
    fn as_ref(&self) -> &[u8] {
        &self.rgba
    }
}

impl From<Screenshot> for Bytes {
    fn from(screenshot: Screenshot) -> Self {
        screenshot.rgba
    }
}

#[derive(Debug, thiserror::Error)]
/// Errors that can occur when cropping a [`Screenshot`].
pub enum CropError {
    #[error("The cropped region is out of bounds.")]
    /// The cropped region's size is out of bounds.
    OutOfBounds,
    #[error("The cropped region is not visible.")]
    /// The cropped region's size is zero.
    Zero,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screenshot() -> Screenshot {
        Screenshot::new((0..48).collect::<Vec<u8>>(), Size::new(4, 3), 2.0)
    }

    fn region(x: u32, y: u32, width: u32, height: u32) -> Rectangle<u32> {
        Rectangle {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn crop_copies_only_requested_rows_and_columns() {
        let cropped = screenshot().crop(region(1, 1, 2, 2)).unwrap();

        assert_eq!(cropped.size, Size::new(2, 2));
        assert_eq!(
            cropped.as_ref(),
            &[
                20, 21, 22, 23, 24, 25, 26, 27, 36, 37, 38, 39, 40, 41, 42, 43
            ]
        );
    }

    #[test]
    fn overflowing_region_and_short_storage_are_rejected() {
        assert!(matches!(
            screenshot().crop(region(u32::MAX, 0, 2, 1)),
            Err(CropError::OutOfBounds)
        ));

        let short = Screenshot::new(vec![0; 4], Size::new(4, 3), 1.0);
        assert!(matches!(
            short.crop(region(0, 0, 1, 1)),
            Err(CropError::OutOfBounds)
        ));
    }

    #[test]
    fn full_frame_crop_reuses_shared_bytes() {
        let source = screenshot();
        let cropped = source
            .crop(region(0, 0, source.size.width, source.size.height))
            .unwrap();

        assert_eq!(cropped.as_ref(), source.as_ref());
    }
}
