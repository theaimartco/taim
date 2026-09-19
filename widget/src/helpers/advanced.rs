//! Optional renderer and advanced widget helpers.

use super::*;
use crate::row;

/// Creates an [`Element`] that displays the taim logo with the given `text_size`.
///
/// Useful for showing some love to your favorite GUI library in your "About" screen,
/// for instance.
pub fn taim<'a, Message, Theme, Renderer>(
    text_size: impl Into<core::Pixels>,
) -> Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Renderer: core::Renderer + core::text::Renderer<Font = core::Font> + 'a,
    Theme: text::Catalog + container::Catalog + 'a,
    <Theme as container::Catalog>::Class<'a>: From<container::StyleFn<'a, Theme>>,
    <Theme as text::Catalog>::Class<'a>: From<text::StyleFn<'a, Theme>>,
{
    use crate::core::border;
    use crate::core::color;
    use crate::core::gradient;
    use crate::core::{Alignment, Color, Font, Radians};

    let text_size = text_size.into();

    row![
        container(
            text(Renderer::TAIM_LOGO)
                .line_height(1.0)
                .size(text_size)
                .font(Renderer::ICON_FONT)
                .color(Color::WHITE)
        )
        .padding(text_size * 0.15)
        .style(move |_| container::Style {
            background: Some(
                gradient::Linear::new(Radians::PI / 4.0)
                    .add_stop(0.0, color!(0x0033ff))
                    .add_stop(1.0, color!(0x1177ff))
                    .into()
            ),
            border: border::rounded(border::radius(text_size * 0.4)),
            ..container::Style::default()
        }),
        text("taim").size(text_size).font(Font::MONOSPACE)
    ]
    .spacing(text_size.0 / 3.0)
    .align_y(Alignment::Center)
    .into()
}

/// Creates a new [`Canvas`].
///
/// Canvases can be leveraged to draw interactive 2D graphics.
///
/// [`Canvas`]: crate::Canvas
///
/// # Example: Drawing a Simple Circle
/// ```no_run
/// # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
/// # pub type State = ();
/// # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
/// #
/// use taim::mouse;
/// use taim::widget::canvas;
/// use taim::{Color, Rectangle, Renderer, Theme};
///
/// // First, we define the data we need for drawing
/// #[derive(Debug)]
/// struct Circle {
///     radius: f32,
/// }
///
/// // Then, we implement the `Program` trait
/// impl<Message> canvas::Program<Message> for Circle {
///     // No internal state
///     type State = ();
///
///     fn draw(
///         &self,
///         _state: &(),
///         renderer: &Renderer,
///         _theme: &Theme,
///         bounds: Rectangle,
///         _cursor: mouse::Cursor
///     ) -> Vec<canvas::Geometry> {
///         // We prepare a new `Frame`
///         let mut frame = canvas::Frame::new(renderer, bounds.size());
///
///         // We create a `Path` representing a simple circle
///         let circle = canvas::Path::circle(frame.center(), self.radius);
///
///         // And fill it with some color
///         frame.fill(&circle, Color::BLACK);
///
///         // Then, we produce the geometry
///         vec![frame.into_geometry()]
///     }
/// }
///
/// // Finally, we simply use our `Circle` to create the `Canvas`!
/// fn view<'a, Message: 'a>(_state: &'a State) -> Element<'a, Message> {
///     canvas(Circle { radius: 50.0 }).into()
/// }
/// ```
#[cfg(feature = "canvas")]
pub fn canvas<P, Message, Theme, Renderer>(program: P) -> crate::Canvas<P, Message, Theme, Renderer>
where
    Renderer: crate::graphics::geometry::Renderer,
    P: crate::canvas::Program<Message, Theme, Renderer>,
{
    crate::Canvas::new(program)
}

/// Creates a new [`QRCode`] widget from the given [`Data`].
///
/// QR codes display information in a type of two-dimensional matrix barcode.
///
/// [`QRCode`]: crate::QRCode
/// [`Data`]: crate::qr_code::Data
///
/// # Example
/// ```no_run
/// # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
/// # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
/// #
/// use taim::widget::qr_code;
///
/// struct State {
///    data: qr_code::Data,
/// }
///
/// #[derive(Debug, Clone)]
/// enum Message {
///     // ...
/// }
///
/// fn view(state: &State) -> Element<'_, Message> {
///     qr_code(&state.data).into()
/// }
/// ```
#[cfg(feature = "qr_code")]
pub fn qr_code<'a, Theme>(data: &'a crate::qr_code::Data) -> crate::QRCode<'a, Theme>
where
    Theme: crate::qr_code::Catalog + 'a,
{
    crate::QRCode::new(data)
}

/// Creates a new [`Shader`].
///
/// [`Shader`]: crate::Shader
#[cfg(feature = "wgpu")]
pub fn shader<Message, P>(program: P) -> crate::Shader<Message, P>
where
    P: crate::shader::Program<Message>,
{
    crate::Shader::new(program)
}

/// Creates a new [`MouseArea`].
pub fn mouse_area<'a, Message, Theme, Renderer>(
    widget: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> MouseArea<'a, Message, Theme, Renderer>
where
    Renderer: core::Renderer,
{
    MouseArea::new(widget)
}

/// A widget that applies any `Theme` to its contents.
pub fn themer<'a, Message, Theme, Renderer>(
    theme: Option<Theme>,
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Themer<'a, Message, Theme, Renderer>
where
    Theme: theme::Base,
    Renderer: core::Renderer,
{
    Themer::new(theme, content)
}

/// Creates a [`PaneGrid`] with the given [`pane_grid::State`] and view function.
///
/// Pane grids let your users split regions of your application and organize layout dynamically.
///
/// # Example
/// ```no_run
/// # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
/// # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
/// #
/// use taim::widget::{pane_grid, text};
///
/// struct State {
///     panes: pane_grid::State<Pane>,
/// }
///
/// enum Pane {
///     SomePane,
///     AnotherKindOfPane,
/// }
///
/// enum Message {
///     PaneDragged(pane_grid::DragEvent),
///     PaneResized(pane_grid::ResizeEvent),
/// }
///
/// fn view(state: &State) -> Element<'_, Message> {
///     pane_grid(&state.panes, |pane, state, is_maximized| {
///         pane_grid::Content::new(match state {
///             Pane::SomePane => text("This is some pane"),
///             Pane::AnotherKindOfPane => text("This is another kind of pane"),
///         })
///     })
///     .on_drag(Message::PaneDragged)
///     .on_resize(10, Message::PaneResized)
///     .into()
/// }
/// ```
pub fn pane_grid<'a, T, Message, Theme, Renderer>(
    state: &'a pane_grid::State<T>,
    view: impl Fn(pane_grid::Pane, &'a T, bool) -> pane_grid::Content<'a, Message, Theme, Renderer>,
) -> PaneGrid<'a, Message, Theme, Renderer>
where
    Theme: pane_grid::Catalog,
    Renderer: core::Renderer,
{
    PaneGrid::new(state, view)
}

/// Creates a new [`Float`] widget with the given content.
pub fn float<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Float<'a, Message, Theme, Renderer>
where
    Theme: float::Catalog,
    Renderer: core::Renderer,
{
    Float::new(content)
}

/// Creates a new [`Responsive`] widget with a closure that produces its
/// contents.
///
/// The `view` closure will receive the maximum available space for
/// the [`Responsive`] during layout. You can use this [`Size`] to
/// conditionally build the contents.
pub fn responsive<'a, Message, Theme, Renderer, E>(
    f: impl Fn(Size) -> E + 'a,
) -> Responsive<'a, Message, Theme, Renderer>
where
    Renderer: core::Renderer,
    E: Into<Element<'a, Message, Theme, Renderer>>,
{
    Responsive::new(f)
}

/// Creates a new [`Transition`].
///
/// The `init` closure will be used to initialize an implementor of [`Program`]. This is normally
/// an [`Animation`](crate::core::Animation), but you can implement [`Program`] on your own types
/// as well.
///
/// The `view` closure will receive the [`Program`] and the current [`Instant`], which can be used for interpolating values.
/// When the `value` changes, this will be called every frame, until the [`Program`] stops animating.
///
/// [`Program`]: transition::Program
///
/// # Example
///
/// Here is how you could implement a smooth progress bar:
///
/// ```
/// # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::core::Animation; }
/// # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
/// use taim::widget::{transition, progress_bar};
/// use taim::Animation;
///
/// fn smooth_progress_bar<'a, Message: 'a>(progress: f32) -> Element<'a, Message> {
///     transition(progress, || Animation::new(0.).quick(), |animation, now| {
///         progress_bar(0.0..=1.0, animation.interpolate_with(std::convert::identity, now))
///     }).into()
/// }
/// ```
pub fn transition<'a, Message, Theme, Renderer, P, E>(
    value: P::Value,
    init: impl Fn() -> P + 'a,
    view: impl Fn(&P, Instant) -> E + 'a,
) -> Transition<'a, Message, Theme, Renderer, P>
where
    Renderer: core::Renderer,
    P: transition::Program,
    E: Into<Element<'a, Message, Theme, Renderer>>,
{
    Transition::new(init, value, view)
}

/// Creates a zero-sized [`Widget`] that does nothing and will be filtered out by
/// containers.
pub fn void() -> core::widget::Void {
    core::widget::Void
}
