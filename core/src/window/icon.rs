//! Change the icon of a window.
use crate::Size;

/// Builds an  [`Icon`] from its RGBA pixels in the `sRGB` color space.
pub fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Result<Icon, Error> {
    const PIXEL_SIZE: usize = 4;

    if width == 0 || height == 0 {
        return Err(Error::EmptyDimensions { width, height });
    }

    if !rgba.len().is_multiple_of(PIXEL_SIZE) {
        return Err(Error::ByteCountNotDivisibleBy4 {
            byte_count: rgba.len(),
        });
    }

    let pixel_count = rgba.len() / PIXEL_SIZE;

    let expected = usize::try_from(width)
        .ok()
        .and_then(|width| usize::try_from(height).ok().and_then(|height| width.checked_mul(height)))
        .ok_or(Error::DimensionsOverflow { width, height })?;

    if pixel_count != expected {
        return Err(Error::DimensionsVsPixelCount {
            width,
            height,
            width_x_height: expected,
            pixel_count,
        });
    }

    Ok(Icon {
        rgba,
        size: Size::new(width, height),
    })
}

/// An window icon normally used for the titlebar or taskbar.
#[derive(Clone)]
pub struct Icon {
    rgba: Vec<u8>,
    size: Size<u32>,
}

impl Icon {
    /// Returns the dimensions of the icon in physical pixels.
    pub const fn size(&self) -> Size<u32> {
        self.size
    }

    /// Borrows the RGBA data without transferring ownership.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// Returns the number of RGBA pixels.
    pub fn pixel_count(&self) -> usize {
        self.rgba.len() / 4
    }

    /// Returns the raw data of the [`Icon`].
    pub fn into_raw(self) -> (Vec<u8>, Size<u32>) {
        (self.rgba, self.size)
    }
}

impl std::fmt::Debug for Icon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Icon")
            .field("rgba", &format!("{} pixels", self.rgba.len() / 4))
            .field("size", &self.size)
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
/// An error produced when using [`from_rgba`] with invalid arguments.
pub enum Error {
    /// Width and height must both be positive.
    #[error("window icon dimensions must be nonzero, received {width}x{height}")]
    EmptyDimensions {
        /// Supplied width.
        width: u32,
        /// Supplied height.
        height: u32,
    },
    /// The pixel count cannot be represented on this platform.
    #[error("window icon dimensions overflow the platform address space: {width}x{height}")]
    DimensionsOverflow {
        /// Supplied width.
        width: u32,
        /// Supplied height.
        height: u32,
    },
    /// Produced when the length of the `rgba` argument isn't divisible by 4, thus `rgba` can't be
    /// safely interpreted as 32bpp RGBA pixels.
    #[error(
        "The provided RGBA data (with length {byte_count}) isn't divisible \
        by 4. Therefore, it cannot be safely interpreted as 32bpp RGBA pixels"
    )]
    ByteCountNotDivisibleBy4 {
        /// The length of the provided RGBA data.
        byte_count: usize,
    },
    /// Produced when the number of pixels (`rgba.len() / 4`) isn't equal to `width * height`.
    /// At least one of your arguments is incorrect.
    #[error(
        "The number of RGBA pixels ({pixel_count}) does not match the \
        provided dimensions ({width}x{height})."
    )]
    DimensionsVsPixelCount {
        /// The provided width.
        width: u32,
        /// The provided height.
        height: u32,
        /// The product of `width` and `height`.
        width_x_height: usize,
        /// The amount of pixels of the provided RGBA data.
        pixel_count: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_rgba_dimensions_are_checked_before_icon_creation() {
        assert!(matches!(from_rgba(Vec::new(), 0, 0), Err(Error::EmptyDimensions { .. })));
        assert!(matches!(from_rgba(vec![0; 4], 2, 1), Err(Error::DimensionsVsPixelCount { .. })));

        let icon = from_rgba(vec![255, 0, 0, 255], 1, 1).unwrap();
        assert_eq!(icon.size(), Size::new(1, 1));
        assert_eq!(icon.pixel_count(), 1);
        assert_eq!(icon.rgba(), &[255, 0, 0, 255]);
    }
}
