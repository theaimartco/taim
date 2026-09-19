use super::*;

/// Converts some [`window::Settings`] into some `WindowAttributes` from `winit`.
pub fn window_attributes(
    settings: window::Settings,
    title: &str,
    scale_factor: f32,
    primary_monitor: Option<winit::monitor::MonitorHandle>,
    _id: Option<String>,
) -> winit::window::WindowAttributes {
    let settings = settings.normalized();
    let scale_factor = valid_application_scale(scale_factor);
    let requested_size = scaled_size(settings.size, scale_factor);
    let mut attributes = winit::window::WindowAttributes::default();

    let mut buttons = winit::window::WindowButtons::empty();

    if settings.resizable {
        buttons |= winit::window::WindowButtons::MAXIMIZE;
    }

    if settings.closeable {
        buttons |= winit::window::WindowButtons::CLOSE;
    }

    if settings.minimizable {
        buttons |= winit::window::WindowButtons::MINIMIZE;
    }

    attributes = attributes
        .with_title(title)
        .with_inner_size(winit::dpi::LogicalSize {
            width: requested_size.width,
            height: requested_size.height,
        })
        .with_maximized(settings.maximized)
        .with_fullscreen(
            settings
                .fullscreen
                .then_some(winit::window::Fullscreen::Borderless(None)),
        )
        .with_resizable(settings.resizable)
        .with_enabled_buttons(buttons)
        .with_decorations(settings.decorations)
        .with_transparent(settings.transparent)
        .with_blur(settings.blur)
        .with_window_icon(settings.icon.and_then(icon))
        .with_window_level(window_level(settings.level))
        .with_visible(settings.visible);

    if let Some(position) = position(primary_monitor.as_ref(), requested_size, settings.position) {
        attributes = attributes.with_position(position);
    }

    if let Some(min_size) = settings.min_size {
        let min_size = scaled_size(min_size, scale_factor);
        attributes = attributes.with_min_inner_size(winit::dpi::LogicalSize {
            width: min_size.width,
            height: min_size.height,
        });
    }

    if let Some(max_size) = settings.max_size {
        let max_size = scaled_size(max_size, scale_factor);
        attributes = attributes.with_max_inner_size(winit::dpi::LogicalSize {
            width: max_size.width,
            height: max_size.height,
        });
    }

    #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        use ::winit::platform::wayland::WindowAttributesExtWayland;

        if let Some(id) = _id {
            attributes = attributes.with_name(id.clone(), id);
        }
    }

    #[cfg(target_os = "windows")]
    {
        use window::settings::platform;
        use winit::platform::windows::{CornerPreference, WindowAttributesExtWindows};

        attributes = attributes.with_drag_and_drop(settings.platform_specific.drag_and_drop);

        attributes = attributes.with_skip_taskbar(settings.platform_specific.skip_taskbar);

        attributes =
            attributes.with_undecorated_shadow(settings.platform_specific.undecorated_shadow);

        attributes =
            attributes.with_corner_preference(match settings.platform_specific.corner_preference {
                platform::CornerPreference::Default => CornerPreference::Default,
                platform::CornerPreference::DoNotRound => CornerPreference::DoNotRound,
                platform::CornerPreference::Round => CornerPreference::Round,
                platform::CornerPreference::RoundSmall => CornerPreference::RoundSmall,
            });
    }

    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS;

        attributes = attributes
            .with_title_hidden(settings.platform_specific.title_hidden)
            .with_titlebar_transparent(settings.platform_specific.titlebar_transparent)
            .with_fullsize_content_view(settings.platform_specific.fullsize_content_view);
    }

    #[cfg(target_os = "linux")]
    {
        #[cfg(feature = "x11")]
        {
            use winit::platform::x11::WindowAttributesExtX11;

            attributes = attributes
                .with_override_redirect(settings.platform_specific.override_redirect)
                .with_name(
                    &settings.platform_specific.application_id,
                    &settings.platform_specific.application_id,
                );
        }
        #[cfg(feature = "wayland")]
        {
            use winit::platform::wayland::WindowAttributesExtWayland;

            attributes = attributes.with_name(
                &settings.platform_specific.application_id,
                &settings.platform_specific.application_id,
            );
        }
    }

    attributes
}

/// Converts a [`window::Level`] into a [`winit`] window level.
///
/// [`winit`]: https://github.com/rust-windowing/winit
pub fn window_level(level: window::Level) -> winit::window::WindowLevel {
    match level {
        window::Level::Normal => winit::window::WindowLevel::Normal,
        window::Level::AlwaysOnBottom => winit::window::WindowLevel::AlwaysOnBottom,
        window::Level::AlwaysOnTop => winit::window::WindowLevel::AlwaysOnTop,
    }
}

/// Converts a [`window::Position`] into a [`winit`] logical position for a given monitor.
///
/// [`winit`]: https://github.com/rust-windowing/winit
pub fn position(
    monitor: Option<&winit::monitor::MonitorHandle>,
    size: Size,
    position: window::Position,
) -> Option<winit::dpi::Position> {
    match position {
        window::Position::Default => None,
        window::Position::Specific(position) => {
            Some(winit::dpi::Position::Logical(winit::dpi::LogicalPosition {
                x: f64::from(position.x),
                y: f64::from(position.y),
            }))
        }
        window::Position::SpecificWith(to_position) => {
            if let Some(monitor) = monitor {
                let start = monitor.position();

                let resolution: winit::dpi::LogicalSize<f32> =
                    monitor.size().to_logical(monitor.scale_factor());

                let position = to_position(size, Size::new(resolution.width, resolution.height));

                let centered: winit::dpi::PhysicalPosition<i32> = winit::dpi::LogicalPosition {
                    x: position.x,
                    y: position.y,
                }
                .to_physical(monitor.scale_factor());

                Some(winit::dpi::Position::Physical(
                    winit::dpi::PhysicalPosition {
                        x: start.x.saturating_add(centered.x),
                        y: start.y.saturating_add(centered.y),
                    },
                ))
            } else {
                None
            }
        }
        window::Position::Centered => {
            if let Some(monitor) = monitor {
                let start = monitor.position();

                let resolution: winit::dpi::LogicalSize<f64> =
                    monitor.size().to_logical(monitor.scale_factor());

                let centered: winit::dpi::PhysicalPosition<i32> = winit::dpi::LogicalPosition {
                    x: (resolution.width - f64::from(size.width)) / 2.0,
                    y: (resolution.height - f64::from(size.height)) / 2.0,
                }
                .to_physical(monitor.scale_factor());

                Some(winit::dpi::Position::Physical(
                    winit::dpi::PhysicalPosition {
                        x: start.x.saturating_add(centered.x),
                        y: start.y.saturating_add(centered.y),
                    },
                ))
            } else {
                None
            }
        }
    }
}

/// Converts a [`window::Mode`] into a [`winit`] fullscreen mode.
///
/// [`winit`]: https://github.com/rust-windowing/winit
pub fn fullscreen(
    monitor: Option<winit::monitor::MonitorHandle>,
    mode: window::Mode,
) -> Option<winit::window::Fullscreen> {
    match mode {
        window::Mode::Windowed | window::Mode::Hidden => None,
        window::Mode::Fullscreen => Some(winit::window::Fullscreen::Borderless(monitor)),
    }
}

/// Converts a [`window::Mode`] into a visibility flag.
pub fn visible(mode: window::Mode) -> bool {
    match mode {
        window::Mode::Windowed | window::Mode::Fullscreen => true,
        window::Mode::Hidden => false,
    }
}

/// Converts a [`winit`] fullscreen mode into a [`window::Mode`].
///
/// [`winit`]: https://github.com/rust-windowing/winit
pub fn mode(mode: Option<winit::window::Fullscreen>) -> window::Mode {
    match mode {
        None => window::Mode::Windowed,
        Some(_) => window::Mode::Fullscreen,
    }
}

/// Converts a [`winit`] window theme into a [`theme::Mode`].
///
/// [`winit`]: https://github.com/rust-windowing/winit
pub fn theme_mode(theme: winit::window::Theme) -> theme::Mode {
    match theme {
        winit::window::Theme::Light => theme::Mode::Light,
        winit::window::Theme::Dark => theme::Mode::Dark,
    }
}

/// Converts a [`theme::Mode`] into a window theme.
///
/// [`winit`]: https://github.com/rust-windowing/winit
pub fn window_theme(mode: theme::Mode) -> Option<winit::window::Theme> {
    match mode {
        theme::Mode::None => None,
        theme::Mode::Light => Some(winit::window::Theme::Light),
        theme::Mode::Dark => Some(winit::window::Theme::Dark),
    }
}

#[inline]
fn valid_application_scale(scale_factor: f32) -> f32 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

#[inline]
fn scaled_size(size: Size, scale_factor: f32) -> Size {
    Size::new(
        finite_dimension(size.width * scale_factor),
        finite_dimension(size.height * scale_factor),
    )
}

#[inline]
fn finite_dimension(dimension: f32) -> f32 {
    if dimension.is_finite() {
        dimension.max(0.0)
    } else {
        0.0
    }
}
