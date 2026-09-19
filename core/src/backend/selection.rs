//! Allocation-conscious backend and adapter selection.

use std::env;
use std::fmt;

/// A graphical backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Backend {
    /// Auto-detect the best available backend.
    Best,
    /// Hardware acceleration through a selected API.
    Hardware(Api),
    /// CPU software rasterization.
    Software,
    /// Named backend supplied by an integration.
    Custom(String),
}

impl Backend {
    /// All built-in backend selections.
    pub const ALL: &[Self] = &[
        Self::Best,
        Self::Hardware(Api::Best),
        Self::Hardware(Api::Vulkan),
        Self::Hardware(Api::Metal),
        Self::Hardware(Api::DirectX12),
        Self::Hardware(Api::OpenGL),
        Self::Hardware(Api::WebGPU),
        Self::Software,
    ];

    /// Returns the selected hardware API.
    #[inline]
    pub const fn hardware(&self) -> Option<Api> {
        match self {
            Self::Hardware(api) => Some(*api),
            _ => None,
        }
    }

    /// Returns whether software rendering was explicitly requested.
    #[inline]
    pub const fn is_software(&self) -> bool {
        matches!(self, Self::Software)
    }

    /// Tests a custom backend name without allocating normalized copies.
    #[inline]
    pub fn matches(&self, target: &str) -> bool {
        match self {
            Self::Best => true,
            Self::Custom(name) => equivalent_name(name, target),
            Self::Hardware(_) | Self::Software => false,
        }
    }
}

impl Default for Backend {
    fn default() -> Self {
        env::var("TAIM_BACKEND").map_or(Self::Best, Self::Custom)
    }
}

impl From<String> for Backend {
    fn from(name: String) -> Self {
        Self::Custom(name)
    }
}

impl From<&str> for Backend {
    fn from(name: &str) -> Self {
        Self::Custom(name.to_owned())
    }
}

impl fmt::Display for Backend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Best => formatter.write_str("Best Backend"),
            Self::Hardware(api) => write!(formatter, "Hardware Backend ({api})"),
            Self::Software => formatter.write_str("Software Backend"),
            Self::Custom(name) => write!(formatter, "Custom Backend ({name})"),
        }
    }
}

/// A hardware graphics API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Api {
    /// Let the renderer choose.
    #[default]
    Best,
    /// Vulkan.
    Vulkan,
    /// Metal.
    Metal,
    /// Direct3D 12.
    DirectX12,
    /// OpenGL, OpenGL ES, or WebGL 2.
    OpenGL,
    /// Browser WebGPU.
    WebGPU,
}

impl fmt::Display for Api {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Best => "Best",
            Self::Vulkan => "Vulkan",
            Self::Metal => "Metal",
            Self::DirectX12 => "DirectX 12",
            Self::OpenGL => "OpenGL",
            Self::WebGPU => "WebGPU",
        })
    }
}

/// Adapter power policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PowerPreference {
    /// No preference.
    #[default]
    None,
    /// Prefer energy efficiency.
    LowPower,
    /// Prefer maximum rendering performance.
    HighPerformance,
}

#[inline]
fn equivalent_name(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left.bytes().zip(right.bytes()).all(|(left, right)| {
            left.eq_ignore_ascii_case(&right)
                || matches!((left, right), (b'-', b'_') | (b'_', b'-'))
        })
}
