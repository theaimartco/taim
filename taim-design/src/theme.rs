//! Immutable Taim theme values and revision tracking.
use crate::platform::Platform;
use crate::tokens::{ColorTokens, MaterialTokens, MotionTokens, Scheme};
use std::sync::Arc;

/// An immutable theme snapshot. Cloning shares token storage cheaply.
#[derive(Debug, Clone)]
pub struct Theme(Arc<Inner>);
#[derive(Debug)]
struct Inner {
    scheme: Scheme,
    platform: Platform,
    colors: ColorTokens,
    motion: MotionTokens,
    materials: MaterialTokens,
    revision: u64,
}
impl Theme {
    /// Creates a theme with revision zero and the default platform.
    pub fn new(scheme: Scheme) -> Self {
        Self::with_platform(scheme, Platform::default())
    }
    /// Creates a theme with an explicit platform.
    pub fn with_platform(scheme: Scheme, platform: Platform) -> Self {
        Self(Arc::new(Inner {
            scheme,
            platform,
            colors: ColorTokens::for_scheme(scheme),
            motion: platform.motion_tokens(),
            materials: MaterialTokens::default(),
            revision: 0,
        }))
    }
    /// Returns the palette scheme.
    pub fn scheme(&self) -> Scheme {
        self.0.scheme
    }
    /// Returns the platform.
    pub fn platform(&self) -> Platform {
        self.0.platform
    }
    /// Returns semantic colors.
    pub fn colors(&self) -> ColorTokens {
        self.0.colors
    }
    /// Returns semantic motion tokens.
    pub fn motion(&self) -> MotionTokens {
        self.0.motion
    }
    /// Returns semantic material tokens.
    ///
    /// [`MaterialTokens::FLAT`] by default -- opt in with
    /// [`Theme::with_materials`] to shade surfaces physically.
    pub fn materials(&self) -> MaterialTokens {
        self.0.materials
    }
    /// Returns monotonic theme revision.
    pub fn revision(&self) -> u64 {
        self.0.revision
    }
    /// Returns a new theme with a different scheme and incremented revision.
    pub fn with_scheme(&self, scheme: Scheme) -> Self {
        if scheme == self.0.scheme {
            return self.clone();
        }
        Self(Arc::new(Inner {
            scheme,
            platform: self.0.platform,
            colors: ColorTokens::for_scheme(scheme),
            motion: self.0.platform.motion_tokens(),
            materials: self.0.materials,
            revision: self.0.revision.wrapping_add(1),
        }))
    }
    /// Returns a new theme with different material tokens and incremented
    /// revision.
    pub fn with_materials(&self, materials: MaterialTokens) -> Self {
        if materials == self.0.materials {
            return self.clone();
        }
        Self(Arc::new(Inner {
            scheme: self.0.scheme,
            platform: self.0.platform,
            colors: self.0.colors,
            motion: self.0.motion,
            materials,
            revision: self.0.revision.wrapping_add(1),
        }))
    }
}
impl Default for Theme {
    fn default() -> Self {
        Self::new(Scheme::Dark)
    }
}

// -----------------------------------------------------------------
// theme::Base — must live here (orphan rule: Theme is local).
// -----------------------------------------------------------------

impl taim_core::theme::Base for Theme {
    fn default(preference: taim_core::theme::Mode) -> Self {
        match preference {
            taim_core::theme::Mode::None | taim_core::theme::Mode::Light => {
                Theme::new(Scheme::Light)
            }
            taim_core::theme::Mode::Dark => Theme::new(Scheme::Dark),
        }
    }

    fn mode(&self) -> taim_core::theme::Mode {
        match self.scheme() {
            Scheme::Light => taim_core::theme::Mode::Light,
            Scheme::Dark | Scheme::HighContrast => taim_core::theme::Mode::Dark,
        }
    }

    fn base(&self) -> taim_core::theme::Style {
        let colors = self.colors();
        taim_core::theme::Style {
            background_color: colors.background,
            text_color: colors.on_surface,
        }
    }

    fn seed(&self) -> Option<taim_core::theme::palette::Seed> {
        // Taim themes use semantic tokens, not palette seeds.
        None
    }

    fn name(&self) -> &str {
        match self.scheme() {
            Scheme::Light => "Taim Light",
            Scheme::Dark => "Taim Dark",
            Scheme::HighContrast => "Taim High Contrast",
        }
    }

    fn light(&self) -> taim_core::material::SceneLight {
        self.materials().light
    }
}

// -----------------------------------------------------------------
// text::Catalog — must live here (orphan rule: Theme is local).
// -----------------------------------------------------------------

impl taim_core::widget::text::Catalog for Theme {
    type Class<'a> = taim_core::widget::text::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme| {
            let colors = theme.colors();
            taim_core::widget::text::Style {
                color: Some(colors.on_surface),
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>) -> taim_core::widget::text::Style {
        class(self)
    }
}
