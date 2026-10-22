//! Window creation attributes and window-state vocabulary.
//!
//! [`window_attributes`] assembles a native window description in layers:
//! appearance and behavior shared by every platform, placement on a monitor,
//! size limits, then whatever the target platform adds on top.
use super::*;

use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::window::{Fullscreen, WindowAttributes, WindowButtons};

/// Describes the native window for `settings`.
///
/// Sizes are in TAIM logical units multiplied by the application's
/// `scale_factor`; `application_id` names the app to BSD Wayland compositors.
pub fn window_attributes(
    settings: window::Settings,
    title: &str,
    scale_factor: f32,
    primary_monitor: Option<winit::monitor::MonitorHandle>,
    application_id: Option<String>,
) -> WindowAttributes {
    let mut settings = settings.normalized();
    let window_icon = settings.icon.take().and_then(icon);
    let scale = sanitized_scale(scale_factor);
    let inner_size = logical(scaled(settings.size, scale));

    let mut attributes =
        shared_attributes(&settings, title, inner_size).with_window_icon(window_icon);

    if let Some(origin) = position(
        primary_monitor.as_ref(),
        scaled(settings.size, scale),
        settings.position,
    ) {
        attributes = attributes.with_position(origin);
    }

    if let Some(min) = settings.min_size {
        attributes = attributes.with_min_inner_size(logical(scaled(min, scale)));
    }

    if let Some(max) = settings.max_size {
        attributes = attributes.with_max_inner_size(logical(scaled(max, scale)));
    }

    platform_attributes(attributes, &settings, application_id)
}

/// Appearance and behavior every platform understands.
fn shared_attributes(
    settings: &window::Settings,
    title: &str,
    inner_size: LogicalSize<f32>,
) -> WindowAttributes {
    let buttons = [
        (WindowButtons::MAXIMIZE, settings.resizable),
        (WindowButtons::CLOSE, settings.closeable),
        (WindowButtons::MINIMIZE, settings.minimizable),
    ]
    .into_iter()
    .filter(|(_, enabled)| *enabled)
    .fold(WindowButtons::empty(), |all, (button, _)| all | button);

    WindowAttributes::default()
        .with_title(title)
        .with_inner_size(inner_size)
        .with_maximized(settings.maximized)
        .with_fullscreen(settings.fullscreen.then_some(Fullscreen::Borderless(None)))
        .with_resizable(settings.resizable)
        .with_enabled_buttons(buttons)
        .with_decorations(settings.decorations)
        .with_transparent(settings.transparent)
        .with_blur(settings.blur)
        .with_window_level(window_level(settings.level))
        .with_visible(settings.visible)
}

#[cfg(target_os = "windows")]
fn platform_attributes(
    attributes: WindowAttributes,
    settings: &window::Settings,
    _application_id: Option<String>,
) -> WindowAttributes {
    use window::settings::platform::CornerPreference as Corners;
    use winit::platform::windows::{CornerPreference, WindowAttributesExtWindows};

    let windows = &settings.platform_specific;
    let corners = match windows.corner_preference {
        Corners::Default => CornerPreference::Default,
        Corners::DoNotRound => CornerPreference::DoNotRound,
        Corners::Round => CornerPreference::Round,
        Corners::RoundSmall => CornerPreference::RoundSmall,
    };

    attributes
        .with_drag_and_drop(windows.drag_and_drop)
        .with_skip_taskbar(windows.skip_taskbar)
        .with_undecorated_shadow(windows.effective_undecorated_shadow(settings.decorations))
        .with_corner_preference(corners)
}

#[cfg(target_os = "macos")]
fn platform_attributes(
    attributes: WindowAttributes,
    settings: &window::Settings,
    _application_id: Option<String>,
) -> WindowAttributes {
    use winit::platform::macos::WindowAttributesExtMacOS;

    let titlebar = settings.platform_specific.normalized(settings.decorations);

    attributes
        .with_title_hidden(titlebar.title_hidden)
        .with_titlebar_transparent(titlebar.titlebar_transparent)
        .with_fullsize_content_view(titlebar.fullsize_content_view)
}

#[cfg(target_os = "linux")]
fn platform_attributes(
    attributes: WindowAttributes,
    settings: &window::Settings,
    _application_id: Option<String>,
) -> WindowAttributes {
    let linux = &settings.platform_specific;
    let _class = linux.valid_application_id();

    #[cfg(feature = "x11")]
    let attributes = {
        use winit::platform::x11::WindowAttributesExtX11;

        let attributes = attributes.with_override_redirect(linux.override_redirect);
        match _class {
            Some(class) => attributes.with_name(class, class),
            None => attributes,
        }
    };

    #[cfg(feature = "wayland")]
    let attributes = {
        use winit::platform::wayland::WindowAttributesExtWayland;

        match _class {
            Some(app_id) => attributes.with_name(app_id, app_id),
            None => attributes,
        }
    };

    attributes
}

#[cfg(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
fn platform_attributes(
    attributes: WindowAttributes,
    _settings: &window::Settings,
    application_id: Option<String>,
) -> WindowAttributes {
    use winit::platform::wayland::WindowAttributesExtWayland;

    match application_id {
        Some(app_id) => attributes.with_name(app_id.clone(), app_id),
        None => attributes,
    }
}

#[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
)))]
fn platform_attributes(
    attributes: WindowAttributes,
    _settings: &window::Settings,
    _application_id: Option<String>,
) -> WindowAttributes {
    attributes
}

/// Translates a stacking level.
pub fn window_level(level: window::Level) -> winit::window::WindowLevel {
    use winit::window::WindowLevel as Stack;

    match level {
        window::Level::Normal => Stack::Normal,
        window::Level::AlwaysOnBottom => Stack::AlwaysOnBottom,
        window::Level::AlwaysOnTop => Stack::AlwaysOnTop,
    }
}

/// Where a window of logical `size` should appear, given `monitor`.
///
/// Positions TAIM resolves globally stay logical; positions relative to the
/// monitor are converted with that monitor's own scale and offset from its
/// physical origin, so they land on the right screen in mixed-DPI setups.
pub fn position(
    monitor: Option<&winit::monitor::MonitorHandle>,
    size: Size,
    position: window::Position,
) -> Option<winit::dpi::Position> {
    let monitor_size = monitor.map(|monitor| {
        let extent: LogicalSize<f32> = monitor.size().to_logical(monitor.scale_factor());
        Size::new(extent.width, extent.height)
    });

    let point = |x: f32, y: f32| LogicalPosition::new(f64::from(x), f64::from(y));

    match position.resolve(size, monitor_size)? {
        window::ResolvedPosition::Global(at) => Some(point(at.x, at.y).into()),
        window::ResolvedPosition::MonitorRelative(offset) => {
            let monitor = monitor?;
            let origin = monitor.position();
            let offset: PhysicalPosition<i32> =
                point(offset.x, offset.y).to_physical(monitor.scale_factor());

            Some(
                PhysicalPosition::new(
                    origin.x.saturating_add(offset.x),
                    origin.y.saturating_add(offset.y),
                )
                .into(),
            )
        }
    }
}

/// The borderless fullscreen request for `mode`, if it asks for fullscreen.
pub fn fullscreen(
    monitor: Option<winit::monitor::MonitorHandle>,
    mode: window::Mode,
) -> Option<Fullscreen> {
    mode.requests_fullscreen()
        .then_some(Fullscreen::Borderless(monitor))
}

/// Whether a window in `mode` is mapped on screen.
pub fn visible(mode: window::Mode) -> bool {
    mode.is_visible()
}

/// The presentation mode of a visible window from its fullscreen state.
pub fn mode(fullscreen: Option<Fullscreen>) -> window::Mode {
    if fullscreen.is_some() {
        window::Mode::Fullscreen
    } else {
        window::Mode::Windowed
    }
}

/// Translates a platform color scheme.
pub fn theme_mode(theme: winit::window::Theme) -> theme::Mode {
    match theme {
        winit::window::Theme::Dark => theme::Mode::Dark,
        winit::window::Theme::Light => theme::Mode::Light,
    }
}

/// The platform color scheme to request for `mode`; `None` follows the system.
pub fn window_theme(mode: theme::Mode) -> Option<winit::window::Theme> {
    Some(match mode {
        theme::Mode::Dark => winit::window::Theme::Dark,
        theme::Mode::Light => winit::window::Theme::Light,
        theme::Mode::None => return None,
    })
}

#[inline]
fn sanitized_scale(scale_factor: f32) -> f32 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

/// `size * scale`, with non-finite or negative dimensions collapsed to zero.
#[inline]
fn scaled(size: Size, scale: f32) -> Size {
    let dimension = |value: f32| {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    };

    Size::new(
        dimension(size.width * scale),
        dimension(size.height * scale),
    )
}

#[inline]
fn logical(size: Size) -> LogicalSize<f32> {
    LogicalSize::new(size.width, size.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaling_never_produces_negative_or_infinite_sizes() {
        assert_eq!(scaled(Size::new(10.0, 20.0), 2.0), Size::new(20.0, 40.0));
        assert_eq!(scaled(Size::new(-5.0, f32::NAN), 1.0), Size::new(0.0, 0.0));
        assert_eq!(scaled(Size::new(f32::MAX, 1.0), 4.0), Size::new(0.0, 4.0));
        assert_eq!(sanitized_scale(0.0), 1.0);
        assert_eq!(sanitized_scale(1.5), 1.5);
    }

    #[test]
    fn theme_vocabulary_round_trips() {
        for theme in [winit::window::Theme::Light, winit::window::Theme::Dark] {
            assert_eq!(window_theme(theme_mode(theme)), Some(theme));
        }
        assert_eq!(window_theme(theme::Mode::None), None);
    }
}
