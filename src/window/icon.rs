//! Bounded icon decoding for native TAIM windows.
//!
//! Icon files and embedded payloads may come from untrusted sources. Decode
//! only after validating the encoded byte budget and header dimensions.
pub use crate::core::window::icon::*;

use crate::core::window::icon;
use std::io;
#[cfg(feature = "image")]
use std::io::Read;
#[cfg(feature = "image")]
use std::path::Path;

/// Maximum size of any decoded window-icon edge.
pub const MAX_ICON_EDGE: u32 = 2048;
/// Maximum number of decoded RGBA pixels.
pub const MAX_ICON_PIXELS: u64 = 2048 * 2048;
/// Upper bound for icon input file bytes.
pub const MAX_ICON_ENCODED_BYTES: usize = 8 * 1024 * 1024;

/// Reject an excessive encoded payload without passing it to a decoder.
#[inline]
pub fn check_icon_encoded_len(bytes: usize) -> Result<(), Error> {
    if bytes > MAX_ICON_ENCODED_BYTES {
        Err(Error::EncodedDataTooLarge {
            bytes,
            limit: MAX_ICON_ENCODED_BYTES,
        })
    } else {
        Ok(())
    }
}

/// Check untrusted image-header dimensions before decoded-image allocation.
#[inline]
pub fn check_icon_dimensions(width: u32, height: u32) -> Result<(), Error> {
    if width == 0
        || height == 0
        || width > MAX_ICON_EDGE
        || height > MAX_ICON_EDGE
        || u64::from(width) * u64::from(height) > MAX_ICON_PIXELS
    {
        return Err(Error::ImageDimensionsExceeded { width, height });
    }
    Ok(())
}

/// Read a native window icon from a single file handle using bounded I/O.
///
/// Only `MAX_ICON_ENCODED_BYTES + 1` bytes are read. An over-budget file
/// is rejected before decoding; file extension does not determine format.
#[cfg(feature = "image")]
pub fn from_file<P: AsRef<Path>>(icon_path: P) -> Result<Icon, Error> {
    let file = std::fs::File::open(icon_path)?;
    let mut encoded = Vec::new();
    // The workspace denies unused_results; record the bounded read
    // explicitly, then reject over-budget data before decoder allocation.
    let _bytes_read = file
        .take(MAX_ICON_ENCODED_BYTES as u64 + 1)
        .read_to_end(&mut encoded)?;

    from_file_data(&encoded, None)
}

/// Decode an icon from embedded encoded image bytes.
///
/// The image header is parsed once for a strict pixel budget; the decoder
/// also receives explicit dimension and best-effort allocation limits.
/// Codec allocation limits may be less strict than dimension limits.
#[cfg(feature = "image")]
pub fn from_file_data(
    data: &[u8],
    explicit_format: Option<image::ImageFormat>,
) -> Result<Icon, Error> {
    check_icon_encoded_len(data.len())?;

    let mut probe = image::ImageReader::new(std::io::Cursor::new(data));
    if let Some(format) = explicit_format {
        probe.set_format(format);
    } else {
        probe = probe.with_guessed_format()?;
    }
    let format = probe.format().ok_or(Error::UnknownImageFormat)?;
    let (width, height) = probe.into_dimensions()?;
    check_icon_dimensions(width, height)?;

    let mut decoder = image::ImageReader::new(std::io::Cursor::new(data));
    decoder.set_format(format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_ICON_EDGE);
    limits.max_image_height = Some(MAX_ICON_EDGE);
    limits.max_alloc = Some(64 * 1024 * 1024);
    decoder.limits(limits);

    let rgba = decoder.decode()?.to_rgba8();
    check_icon_dimensions(rgba.width(), rgba.height())?;
    Ok(icon::from_rgba(rgba.into_raw(), width, height)?)
}

/// An error produced while validating or decoding a native window icon.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The decoded RGBA icon is invalid.
    #[error("The icon is invalid: {0}")]
    InvalidError(#[from] icon::Error),

    /// File I/O failed.
    #[error("Unable to read window icon: {0}")]
    OsError(#[from] io::Error),

    /// Image header or payload decoding failed.
    #[cfg(feature = "image")]
    #[error("Unable to decode window icon: {0}")]
    ImageError(#[from] image::ImageError),

    /// Embedded payload has no recognized format.
    #[error("Unable to detect the window icon image format")]
    UnknownImageFormat,

    /// Encoded input exceeds the configured budget.
    #[error("Icon input is {bytes} bytes, exceeding limit {limit}")]
    EncodedDataTooLarge {
        /// Source data length.
        bytes: usize,
        /// Accepted data limit.
        limit: usize,
    },

    /// Header declared invalid or excessive dimensions.
    #[error("Icon dimensions {width}x{height} exceed the TAIM icon budget")]
    ImageDimensionsExceeded {
        /// Source pixel width.
        width: u32,
        /// Source pixel height.
        height: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_input_limits_are_rejected_without_decoder_allocation() {
        assert!(check_icon_encoded_len(MAX_ICON_ENCODED_BYTES).is_ok());
        assert!(matches!(
            check_icon_encoded_len(MAX_ICON_ENCODED_BYTES + 1),
            Err(Error::EncodedDataTooLarge { .. })
        ));
        assert!(check_icon_dimensions(1, 1).is_ok());
        assert!(check_icon_dimensions(2048, 2048).is_ok());
        assert!(matches!(
            check_icon_dimensions(2049, 1),
            Err(Error::ImageDimensionsExceeded { .. })
        ));
        assert!(check_icon_dimensions(0, 2).is_err());
        assert!(check_icon_dimensions(u32::MAX, u32::MAX).is_err());
    }

    #[cfg(feature = "image")]
    #[test]
    fn png_roundtrip_and_oversized_header_rejection() {
        let mut valid = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::new(1, 1))
            .write_to(&mut valid, image::ImageFormat::Png)
            .unwrap();

        let icon = from_file_data(valid.get_ref(), None).unwrap();
        assert_eq!(icon.size().width, 1);
        assert_eq!(icon.size().height, 1);

        let mut oversized = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::new(MAX_ICON_EDGE + 1, 1))
            .write_to(&mut oversized, image::ImageFormat::Png)
            .unwrap();
        assert!(matches!(
            from_file_data(oversized.get_ref(), None),
            Err(Error::ImageDimensionsExceeded { .. })
        ));
    }
}
