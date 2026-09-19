use crate::core::Bytes;
use crate::core::image;

const MAX_DIMENSION: u32 = 16_384;
const MAX_RGBA_BYTES: u64 = 256 * 1024 * 1024;

/// Decoded RGBA8 pixels backed by TAIM's shared byte storage.
pub type Buffer = ::image::ImageBuffer<::image::Rgba<u8>, Bytes>;

#[derive(Debug, Clone, Copy, Default)]
enum Orientation {
    #[default]
    Identity,
    FlipHorizontal,
    Rotate180,
    FlipVertical,
    Rotate90FlipHorizontal,
    Rotate90,
    Rotate90FlipVertical,
    Rotate270,
}

impl Orientation {
    fn read(source: &mut (impl std::io::BufRead + std::io::Seek)) -> Self {
        exif::Reader::new()
            .read_from_container(source)
            .ok()
            .and_then(|metadata| {
                metadata
                    .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                    .and_then(|field| field.value.get_uint(0))
            })
            .map_or(Self::Identity, |value| match value {
                2 => Self::FlipHorizontal,
                3 => Self::Rotate180,
                4 => Self::FlipVertical,
                5 => Self::Rotate90FlipHorizontal,
                6 => Self::Rotate90,
                7 => Self::Rotate90FlipVertical,
                8 => Self::Rotate270,
                _ => Self::Identity,
            })
    }

    fn apply(self, decoded: ::image::DynamicImage) -> ::image::DynamicImage {
        use ::image::imageops;

        match self {
            Self::Identity => decoded,
            Self::FlipHorizontal => imageops::flip_horizontal(&decoded).into(),
            Self::Rotate180 => imageops::rotate180(&decoded).into(),
            Self::FlipVertical => imageops::flip_vertical(&decoded).into(),
            Self::Rotate90FlipHorizontal => {
                imageops::flip_horizontal(&imageops::rotate90(&decoded)).into()
            }
            Self::Rotate90 => imageops::rotate90(&decoded).into(),
            Self::Rotate90FlipVertical => {
                imageops::flip_vertical(&imageops::rotate90(&decoded)).into()
            }
            Self::Rotate270 => imageops::rotate270(&decoded).into(),
        }
    }
}

/// Decodes or validates an image handle under TAIM's storage limits.
pub fn load(handle: &image::Handle) -> Result<Buffer, image::Error> {
    let (width, height, pixels) = match handle {
        image::Handle::Path(_, path) => {
            let file = std::fs::File::open(path)?;
            decode(std::io::BufReader::new(file))?
        }
        image::Handle::Bytes(_, bytes) => decode(std::io::Cursor::new(bytes.as_ref()))?,
        image::Handle::Rgba {
            width,
            height,
            pixels,
            ..
        } => {
            validate_rgba(*width, *height, pixels.len())?;
            (*width, *height, pixels.clone())
        }
    };

    validate_rgba(width, height, pixels.len())?;
    Buffer::from_raw(width, height, pixels).ok_or_else(limit_error)
}

fn decode(
    mut source: impl std::io::BufRead + std::io::Seek,
) -> Result<(u32, u32, Bytes), image::Error> {
    let orientation = Orientation::read(&mut source);
    source.rewind()?;

    let mut reader = ::image::ImageReader::new(source).with_guessed_format()?;
    let mut limits = ::image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_RGBA_BYTES);
    reader.limits(limits);

    let rgba = orientation
        .apply(reader.decode().map_err(map_error)?)
        .into_rgba8();
    validate_rgba(rgba.width(), rgba.height(), rgba.as_raw().len())?;

    Ok((rgba.width(), rgba.height(), Bytes::from(rgba.into_raw())))
}

fn validate_rgba(width: u32, height: u32, actual: usize) -> Result<(), image::Error> {
    let expected = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .filter(|bytes| {
            width <= MAX_DIMENSION && height <= MAX_DIMENSION && *bytes <= MAX_RGBA_BYTES
        })
        .and_then(|bytes| usize::try_from(bytes).ok());

    if expected == Some(actual) {
        Ok(())
    } else {
        Err(limit_error())
    }
}

fn limit_error() -> image::Error {
    map_error(::image::error::ImageError::Limits(
        ::image::error::LimitError::from_kind(::image::error::LimitErrorKind::DimensionError),
    ))
}

fn map_error(error: ::image::ImageError) -> image::Error {
    use std::sync::Arc;

    match error {
        ::image::ImageError::IoError(error) => image::Error::Inaccessible(Arc::new(error)),
        error => image::Error::Invalid(Arc::new(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_rgba_validation_is_exact_and_bounded() {
        assert!(validate_rgba(1, 1, 4).is_ok());
        assert!(validate_rgba(2, 2, 15).is_err());
        assert!(validate_rgba(MAX_DIMENSION + 1, 1, 0).is_err());
        assert!(validate_rgba(MAX_DIMENSION, MAX_DIMENSION, 0).is_err());
    }
}
