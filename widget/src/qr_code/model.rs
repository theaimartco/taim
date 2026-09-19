use super::*;

/// Encoded QR modules and their bounded rendered-geometry caches.
#[derive(Debug)]
pub struct Data {
    pub(super) contents: Vec<qrcode::Color>,
    pub(super) width: usize,
    pub(super) caches: RefCell<Vec<StyledCache>>,
}

impl Data {
    /// Encodes data with medium error correction and the smallest valid size.
    pub fn new(data: impl AsRef<[u8]>) -> Result<Self, Error> {
        Ok(Self::build(qrcode::QrCode::new(data)?))
    }

    /// Encodes data with a chosen error-correction level.
    pub fn with_error_correction(
        data: impl AsRef<[u8]>,
        error_correction: ErrorCorrection,
    ) -> Result<Self, Error> {
        Ok(Self::build(qrcode::QrCode::with_error_correction_level(
            data,
            error_correction.into(),
        )?))
    }

    /// Encodes data with a fixed version and error-correction level.
    pub fn with_version(
        data: impl AsRef<[u8]>,
        version: Version,
        error_correction: ErrorCorrection,
    ) -> Result<Self, Error> {
        Ok(Self::build(qrcode::QrCode::with_version(
            data,
            version.into(),
            error_correction.into(),
        )?))
    }

    fn build(encoded: qrcode::QrCode) -> Self {
        let width = encoded.width();

        Self {
            contents: encoded.into_colors(),
            width,
            caches: RefCell::new(Vec::with_capacity(2)),
        }
    }
}

#[derive(Debug)]
pub(super) struct StyledCache {
    pub(super) style: Style,
    pub(super) geometry: canvas::Cache<Renderer>,
}

/// QR-code symbol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// Standard version from 1 through 40.
    Normal(u8),
    /// Micro version from 1 through 4.
    Micro(u8),
}

impl From<Version> for qrcode::Version {
    fn from(version: Version) -> Self {
        match version {
            Version::Normal(version) => Self::Normal(i16::from(version)),
            Version::Micro(version) => Self::Micro(i16::from(version)),
        }
    }
}

/// QR error-correction level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCorrection {
    /// Roughly 7% recovery.
    Low,
    /// Roughly 15% recovery.
    Medium,
    /// Roughly 25% recovery.
    Quartile,
    /// Roughly 30% recovery.
    High,
}

impl From<ErrorCorrection> for qrcode::EcLevel {
    fn from(level: ErrorCorrection) -> Self {
        match level {
            ErrorCorrection::Low => Self::L,
            ErrorCorrection::Medium => Self::M,
            ErrorCorrection::Quartile => Self::Q,
            ErrorCorrection::High => Self::H,
        }
    }
}

/// QR encoding failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ThisError)]
pub enum Error {
    /// Input exceeds the selected symbol capacity.
    #[error("The data is too long to encode in a QR code for the chosen version")]
    DataTooLong,
    /// Version and correction level are incompatible.
    #[error("The chosen version and error correction level combination is invalid.")]
    InvalidVersion,
    /// Input contains unsupported characters.
    #[error("One or more characters are unsupported by the chosen version")]
    UnsupportedCharacterSet,
    /// The ECI designator is outside its valid range.
    #[error("The chosen ECI designator is invalid")]
    InvalidEciDesignator,
    /// A character is outside the selected character set.
    #[error("A character does not belong to the selected character set")]
    InvalidCharacter,
}

impl From<qrcode::types::QrError> for Error {
    fn from(error: qrcode::types::QrError) -> Self {
        match error {
            qrcode::types::QrError::DataTooLong => Self::DataTooLong,
            qrcode::types::QrError::InvalidVersion => Self::InvalidVersion,
            qrcode::types::QrError::UnsupportedCharacterSet => Self::UnsupportedCharacterSet,
            qrcode::types::QrError::InvalidEciDesignator => Self::InvalidEciDesignator,
            qrcode::types::QrError::InvalidCharacter => Self::InvalidCharacter,
        }
    }
}
