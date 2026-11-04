//! Configure your windows.
#[cfg(target_os = "windows")]
#[path = "settings/windows.rs"]
pub mod platform;

#[cfg(target_os = "macos")]
#[path = "settings/macos.rs"]
mod platform;

#[cfg(target_os = "linux")]
#[path = "settings/linux.rs"]
mod platform;

#[cfg(target_arch = "wasm32")]
#[path = "settings/wasm.rs"]
mod platform;

#[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "linux",
    target_arch = "wasm32"
)))]
#[path = "settings/other.rs"]
mod platform;

use crate::Size;
use crate::window::{Icon, Level, Position};

pub use platform::PlatformSpecific;

/// The window settings of an application.
#[derive(Debug, Clone)]
pub struct Settings {
    /// The initial logical dimensions of the window.
    pub size: Size,

    /// Whether the window should start maximized.
    pub maximized: bool,

    /// Whether the window should start fullscreen.
    pub fullscreen: bool,

    /// The initial position of the window.
    pub position: Position,

    /// The minimum size of the window.
    pub min_size: Option<Size>,

    /// The maximum size of the window.
    pub max_size: Option<Size>,

    /// Whether the window should be visible or not.
    pub visible: bool,

    /// Whether the window should be resizable or not.
    pub resizable: bool,

    /// Whether the title bar has Close button or not
    pub closeable: bool,

    /// Whether the title bar has Minimize button or not
    pub minimizable: bool,

    /// Whether the window should have a border, a title bar, etc. or not.
    pub decorations: bool,

    /// Whether the window should be transparent.
    pub transparent: bool,

    /// Whether the window should have blurry background.
    ///
    /// Note that the blurry effect is applied to the transparent window. You need to enable
    /// [`Settings::transparent`] and set a proper opacity value to the background color with
    /// `Application::style`.
    ///
    /// This option is only supported on macOS and Linux. Please read the [winit document][winit]
    /// for more details.
    ///
    /// [winit]: https://docs.rs/winit/0.30/winit/window/struct.Window.html#method.set_blur
    pub blur: bool,

    /// The window [`Level`].
    pub level: Level,

    /// The icon of the window.
    pub icon: Option<Icon>,

    /// Platform specific settings.
    pub platform_specific: PlatformSpecific,

    /// Whether the window will close when the user requests it, e.g. when a user presses the
    /// close button.
    ///
    /// This can be useful if you want to have some behavior that executes before the window is
    /// actually destroyed. If you disable this, you must manually close the window with the
    /// `window::close` command.
    ///
    /// By default this is enabled.
    pub exit_on_close_request: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            size: Size::new(1024.0, 768.0),
            maximized: false,
            fullscreen: false,
            position: Position::default(),
            min_size: None,
            max_size: None,
            visible: true,
            resizable: true,
            minimizable: true,
            closeable: true,
            decorations: true,
            transparent: false,
            blur: false,
            level: Level::default(),
            icon: None,
            exit_on_close_request: true,
            // Other platforms use a unit struct; desktop adapters have fields.
            #[allow(clippy::default_constructed_unit_structs)]
            platform_specific: PlatformSpecific::default(),
        }
    }
}

impl Settings {
    /// Sets the requested initial logical size.
    pub fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Sets the minimum logical size constraint.
    pub fn with_min_size(mut self, size: Option<Size>) -> Self {
        self.min_size = size;
        self
    }

    /// Sets the maximum logical size constraint.
    pub fn with_max_size(mut self, size: Option<Size>) -> Self {
        self.max_size = size;
        self
    }

    /// Sets both logical size constraints in one pass.
    pub fn with_size_limits(mut self, minimum: Option<Size>, maximum: Option<Size>) -> Self {
        self.min_size = minimum;
        self.max_size = maximum;
        self
    }

    /// Sets the initial window position.
    pub fn with_position(mut self, position: Position) -> Self {
        self.position = position;
        self
    }

    /// Sets initial visibility.
    pub fn with_visibility(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Returns the effective initial size after normalization.
    pub fn effective_size(&self) -> Size {
        normalized_geometry(self.size, self.min_size, self.max_size).0
    }

    /// Normalizes geometry before it crosses into a platform window API.
    ///
    /// Non-finite/negative constraints are discarded, the maximum is raised
    /// to the minimum when they conflict, and the initial size is clamped to
    /// the resulting range. This keeps backend-specific APIs from receiving
    /// NaN or impossible window geometry.
    pub fn normalized(mut self) -> Self {
        (self.size, self.min_size, self.max_size) =
            normalized_geometry(self.size, self.min_size, self.max_size);
        self
    }
}

fn normalized_geometry(
    requested: Size,
    minimum: Option<Size>,
    maximum: Option<Size>,
) -> (Size, Option<Size>, Option<Size>) {
    const DEFAULT: Size = Size::new(1024.0, 768.0);

    let minimum = minimum.filter(|size| valid_constraint(*size, false));
    let mut maximum = maximum.filter(|size| valid_constraint(*size, true));

    if let (Some(minimum), Some(maximum)) = (minimum, &mut maximum) {
        maximum.width = maximum.width.max(minimum.width);
        maximum.height = maximum.height.max(minimum.height);
    }

    let mut size = Size::new(
        positive_or(requested.width, DEFAULT.width),
        positive_or(requested.height, DEFAULT.height),
    );
    if let Some(minimum) = minimum {
        size.width = size.width.max(minimum.width);
        size.height = size.height.max(minimum.height);
    }
    if let Some(maximum) = maximum {
        size.width = size.width.min(maximum.width);
        size.height = size.height.min(maximum.height);
    }

    (size, minimum, maximum)
}

fn valid_constraint(size: Size, strict: bool) -> bool {
    let lower_bound = if strict { f32::MIN_POSITIVE } else { 0.0 };
    size.width.is_finite()
        && size.height.is_finite()
        && size.width >= lower_bound
        && size.height >= lower_bound
}

fn positive_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;
    use crate::Size;

    #[test]
    fn normalization_rejects_invalid_geometry_and_resolves_conflicts() {
        let settings = Settings {
            size: Size::new(f32::NAN, -1.0),
            min_size: Some(Size::new(800.0, 600.0)),
            max_size: Some(Size::new(640.0, 480.0)),
            ..Settings::default()
        }
        .normalized();

        assert_eq!(settings.size, Size::new(800.0, 600.0));
        assert_eq!(settings.max_size, Some(Size::new(800.0, 600.0)));
    }

    #[test]
    fn effective_size_does_not_require_copying_the_icon() {
        let settings = Settings {
            size: Size::new(640.0, 480.0),
            max_size: Some(Size::new(320.0, 240.0)),
            ..Settings::default()
        };

        assert_eq!(settings.effective_size(), Size::new(320.0, 240.0));
        assert_eq!(settings.normalized().size, Size::new(320.0, 240.0));
    }

    #[test]
    fn zero_maximum_cannot_collapse_the_window() {
        let settings = Settings {
            max_size: Some(Size::new(0.0, 100.0)),
            ..Settings::default()
        }
        .normalized();

        assert_eq!(settings.max_size, None);
        assert_eq!(settings.size, Size::new(1024.0, 768.0));
    }
}
