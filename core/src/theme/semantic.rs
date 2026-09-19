use super::Theme;
use crate::Color;

// -----------------------------------------------------------------
// TAIM Semantic Design Environment
// -----------------------------------------------------------------

/// Semantic color roles for Taim controls and surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ColorRole {
    /// Primary brand accent color.
    #[default]
    Primary,
    /// High-contrast text/icon color on top of Primary.
    OnPrimary,
    /// Secondary accent color for supporting UI elements.
    Secondary,
    /// High-contrast text/icon color on top of Secondary.
    OnSecondary,
    /// Standard surface container color.
    Surface,
    /// Text and icon color on standard surfaces.
    OnSurface,
    /// Subtle, tinted container color.
    SurfaceVariant,
    /// Secondary text color on variant surfaces.
    OnSurfaceVariant,
    /// Application background color.
    Background,
    /// Body text color on background.
    OnBackground,
    /// Hairline and divider boundary color.
    Outline,
    /// Critical error indicator color.
    Error,
    /// Text on error containers.
    OnError,
    /// Cautionary warning color.
    Warning,
    /// Success / confirmation color.
    Success,
}

/// Semantic surface roles establishing depth and material context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SurfaceRole {
    /// Deepest application background canvas.
    #[default]
    Canvas,
    /// Elevated content card.
    Card,
    /// Bottom sheet or modal drawer.
    Sheet,
    /// High-priority modal dialog.
    Dialog,
    /// Transient popup or dropdown menu.
    Popover,
    /// Floating action button or tool palette.
    Floating,
    /// Translucent frosted physical glass surface.
    Glass,
}

/// Typography semantic roles for scalable layout and text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TypographyRole {
    /// Hero title text.
    Display,
    /// Section header text.
    Headline,
    /// Card and dialog titles.
    Title,
    /// Standard reading text.
    #[default]
    Body,
    /// Control labels and badges.
    Label,
    /// Fixed-width numbers and alignment rows.
    Monospace,
    /// Code editor and terminal blocks.
    Code,
}

/// Elevation levels for layered depth and shadows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ElevationRole {
    /// Flat on surface.
    #[default]
    Level0,
    /// Resting card or button.
    Level1,
    /// Hovered card or sheet.
    Level2,
    /// Modal dialog.
    Level3,
    /// Nav drawer or floating sheet.
    Level4,
    /// Topmost overlay.
    Level5,
}

/// Semantic motion roles dictating interaction curve physics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MotionRole {
    /// 0ms instant transition.
    Instant,
    /// 100ms micro-interaction (button press, checkbox toggle).
    Fast,
    /// 250ms standard layout settle.
    #[default]
    Standard,
    /// 400ms expressive page transition or sheet slide.
    Expressive,
    /// Physics-based spring with inertia.
    HeavySpring,
}

/// Interactive states for buttons, inputs, and controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum InteractionRole {
    /// Passive resting state.
    #[default]
    Idle,
    /// Pointer cursor within bounds.
    Hovered,
    /// Active mouse button or finger down.
    Pressed,
    /// Keyboard or accessibility focus ring active.
    Focused,
    /// Non-interactive disabled state.
    Disabled,
    /// Dragging in progress.
    Dragged,
}

/// Visual density scaling for high-density desktop vs spacious touch layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Density {
    /// Compact desktop layout (tight paddings, dense rows).
    Compact,
    /// Balanced standard layout.
    #[default]
    Normal,
    /// Spacious touch-first layout (minimum 48px targets).
    Comfortable,
}

/// Target device class for adaptive styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DeviceClass {
    /// Handheld touch device.
    Mobile,
    /// Medium viewport tablet.
    Tablet,
    /// Desktop mouse + keyboard environment.
    #[default]
    Desktop,
    /// Low-power embedded or automotive display.
    Embedded,
}

/// Accessibility preferences influencing color contrast and animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AccessibilityPreferences {
    /// Enhanced contrast ratio mode.
    pub high_contrast: bool,
    /// Disables decorative motion and spring effects.
    pub reduced_motion: bool,
    /// Scales typography by 1.25x+.
    pub large_text: bool,
}

impl Theme {
    /// Resolves a semantic [`ColorRole`] against this theme's palette.
    pub fn color_role(&self, role: ColorRole) -> Color {
        let palette = self.palette();
        match role {
            ColorRole::Primary => palette.primary.base.color,
            ColorRole::OnPrimary => palette.primary.base.text,
            ColorRole::Secondary => palette.secondary.base.color,
            ColorRole::OnSecondary => palette.secondary.base.text,
            ColorRole::Surface => palette.background.base.color,
            ColorRole::OnSurface => palette.background.base.text,
            ColorRole::SurfaceVariant => palette.background.weakest.color,
            ColorRole::OnSurfaceVariant => palette.background.weakest.text,
            ColorRole::Background => palette.background.base.color,
            ColorRole::OnBackground => palette.background.base.text,
            ColorRole::Outline => palette.background.stronger.color,
            ColorRole::Error => palette.danger.base.color,
            ColorRole::OnError => palette.danger.base.text,
            ColorRole::Warning => palette.warning.base.color,
            ColorRole::Success => palette.success.base.color,
        }
    }

    /// Resolves the shadow blur radius for an [`ElevationRole`].
    pub fn elevation_blur(&self, role: ElevationRole) -> f32 {
        match role {
            ElevationRole::Level0 => 0.0,
            ElevationRole::Level1 => 2.0,
            ElevationRole::Level2 => 6.0,
            ElevationRole::Level3 => 12.0,
            ElevationRole::Level4 => 20.0,
            ElevationRole::Level5 => 32.0,
        }
    }

    /// Resolves standard motion duration in milliseconds for a [`MotionRole`].
    pub fn motion_duration_ms(&self, role: MotionRole) -> u32 {
        match role {
            MotionRole::Instant => 0,
            MotionRole::Fast => 100,
            MotionRole::Standard => 250,
            MotionRole::Expressive => 400,
            MotionRole::HeavySpring => 500,
        }
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    #[test]
    fn semantic_color_roles_resolve_consistently() {
        let theme = Theme::Dark;
        let primary = theme.color_role(ColorRole::Primary);
        let on_primary = theme.color_role(ColorRole::OnPrimary);
        assert_ne!(primary, on_primary);

        let surface = theme.color_role(ColorRole::Surface);
        let on_surface = theme.color_role(ColorRole::OnSurface);
        assert_ne!(surface, on_surface);
    }

    #[test]
    fn elevation_blur_grows_monotonically() {
        let theme = Theme::Light;
        let b0 = theme.elevation_blur(ElevationRole::Level0);
        let b1 = theme.elevation_blur(ElevationRole::Level1);
        let b2 = theme.elevation_blur(ElevationRole::Level2);
        let b3 = theme.elevation_blur(ElevationRole::Level3);
        let b4 = theme.elevation_blur(ElevationRole::Level4);
        let b5 = theme.elevation_blur(ElevationRole::Level5);

        assert!(b0 <= b1 && b1 <= b2 && b2 <= b3 && b3 <= b4 && b4 <= b5);
    }
}
