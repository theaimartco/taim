//! Starting an application.
use crate::{Font, Result, Settings, window};

use taim_view::{AnyView, Scope};

use std::borrow::Cow;

/// Opens a window titled `title` and shows the view `build` returns.
///
/// `build` runs once. It creates the window's state with
/// [`Scope::signal`](taim_view::Scope::signal) and returns the root view;
/// from then on the view tree updates itself as that state changes.
///
/// ```no_run
/// use taim::prelude::*;
///
/// fn main() -> taim::Result {
///     taim::launch("Counter", |cx| {
///         let count = cx.signal(0);
///
///         column(views![
///             text(move || format!("Count: {}", count.get())),
///             button("+1").on_press(move || count.update(|n| *n += 1)),
///         ])
///     })
/// }
/// ```
pub fn launch<V>(title: &str, build: impl FnOnce(&mut Scope) -> V + 'static) -> Result
where
    V: Into<AnyView>,
{
    app(title, build).run()
}

/// Describes an application before running it; see [`App::run`].
///
/// ```no_run
/// use taim::prelude::*;
///
/// fn main() -> taim::Result {
///     taim::app("Notes", |_| text("Hello"))
///         .size(480.0, 640.0)
///         .run()
/// }
/// ```
pub fn app<V>(title: &str, build: impl FnOnce(&mut Scope) -> V + 'static) -> App
where
    V: Into<AnyView>,
{
    App {
        title: title.to_owned(),
        settings: Settings::default(),
        window: window::Settings::default(),
        build: Box::new(move |cx| build(cx).into()),
    }
}

/// An application ready to run. Made by [`app`].
pub struct App {
    title: String,
    settings: Settings,
    window: window::Settings,
    build: Box<dyn FnOnce(&mut Scope) -> AnyView>,
}

impl App {
    /// The window's initial size in logical pixels.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.window.size = crate::Size::new(width, height);
        self
    }

    /// Replaces every window setting.
    pub fn window(mut self, settings: window::Settings) -> Self {
        self.window = settings;
        self
    }

    /// Replaces the application settings (fonts, default text, backend).
    pub fn settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    /// Makes a font available to every view.
    pub fn font(mut self, bytes: impl Into<Cow<'static, [u8]>>) -> Self {
        self.settings.fonts.push(bytes.into());
        self
    }

    /// The font text uses when a view does not choose one.
    pub fn default_font(mut self, font: Font) -> Self {
        self.settings.default_font = font;
        self
    }

    /// Opens the window and runs until it closes.
    pub fn run(self) -> Result {
        taim_winit::run(taim_winit::App {
            title: self.title,
            settings: self.settings,
            window: self.window,
            build: self.build,
        })
        .map_err(Into::into)
    }
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("title", &self.title)
            .finish_non_exhaustive()
    }
}

/// Everything an application written against the view API needs.
pub mod prelude {
    pub use taim_view::prelude::*;
}
