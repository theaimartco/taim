//! Immutable Aura theme values and revision tracking.
use crate::platform::Platform;
use crate::tokens::{ColorTokens, MotionTokens, Scheme};
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

impl aura_core::theme::Base for Theme {
    fn default(preference: aura_core::theme::Mode) -> Self {
        match preference {
            aura_core::theme::Mode::None | aura_core::theme::Mode::Light => {
                Theme::new(Scheme::Light)
            }
            aura_core::theme::Mode::Dark => Theme::new(Scheme::Dark),
        }
    }

    fn mode(&self) -> aura_core::theme::Mode {
        match self.scheme() {
            Scheme::Light => aura_core::theme::Mode::Light,
            Scheme::Dark | Scheme::HighContrast => aura_core::theme::Mode::Dark,
        }
    }

    fn base(&self) -> aura_core::theme::Style {
        let colors = self.colors();
        aura_core::theme::Style {
            background_color: colors.background,
            text_color: colors.on_surface,
        }
    }

    fn seed(&self) -> Option<aura_core::theme::palette::Seed> {
        // Aura themes use semantic tokens, not palette seeds.
        None
    }

    fn name(&self) -> &str {
        match self.scheme() {
            Scheme::Light => "Aura Light",
            Scheme::Dark => "Aura Dark",
            Scheme::HighContrast => "Aura High Contrast",
        }
    }
}

// -----------------------------------------------------------------
// text::Catalog — must live here (orphan rule: Theme is local).
// -----------------------------------------------------------------

impl aura_core::widget::text::Catalog for Theme {
    type Class<'a> = aura_core::widget::text::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme: &Theme| {
            let colors = theme.colors();
            aura_core::widget::text::Style {
                color: Some(colors.on_surface),
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>) -> aura_core::widget::text::Style {
        class(self)
    }
}
