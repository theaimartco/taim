//! The core library of [Taim].
//!
//! This library holds basic types that can be reused and re-exported in
//! different runtime implementations.
//!
//! ![The foundations of the Taim ecosystem](https://github.com/theaimartco/taim/blob/main/docs/graphs/foundations.svg?raw=true)
//!
//! [Taim]: https://github.com/theaimartco/taim
#![doc(html_logo_url = "https://raw.githubusercontent.com/theaimartco/taim/main/docs/logo.svg")]
macro_rules! modules {
    (public: $($public:ident),+; private: $($private:ident),+ $(;)?) => {
        $(pub mod $public;)+
        $(mod $private;)+
    };
}

macro_rules! exports {
    ($($module:ident::$item:ident),+ $(,)?) => { $(pub use $module::$item;)+ };
}

modules!(
    public:
        alignment, animation, backend, border, cache, clipboard, color, damage, event, font,
        frame, gradient, image, input_method, keyboard, layout, layout_solver, length, material,
        mouse, overlay, padding, pointer, renderer, scene, security, semantics, semantics_driver,
        shell, spatial, svg, text, theme, time, touch, widget, window;
    private:
        angle, background, content_fit, element, pixels, point, rectangle, rotation, settings,
        shadow, size, transformation, vector;
);

exports!(
    alignment::Alignment,
    animation::Animation,
    backend::Backend,
    background::Background,
    border::Border,
    clipboard::Clipboard,
    color::Color,
    content_fit::ContentFit,
    element::Element,
    event::Event,
    font::Font,
    gradient::Gradient,
    image::Image,
    input_method::InputMethod,
    layout::Layout,
    length::Length,
    overlay::Overlay,
    padding::Padding,
    pixels::Pixels,
    point::Point,
    rectangle::Rectangle,
    renderer::Renderer,
    rotation::Rotation,
    settings::Settings,
    shadow::Shadow,
    shell::Shell,
    size::Size,
    svg::Svg,
    text::Text,
    theme::Theme,
    transformation::Transformation,
    vector::Vector,
    widget::Widget,
    window::Window,
);

pub use angle::{Degrees, Radians};
pub use scene::{NodeId, SceneArena, SceneNode};
pub use semantics_driver::SemanticsDriver;
pub use spatial::{Mm, PhysicalContext, Transform3D};

pub use bytes::Bytes;
pub use smol_str::SmolStr;
pub use std::convert::Infallible as Never;

/// A function that can _never_ be called.
///
/// This is useful to turn generic types into anything
/// you want by coercing them into a type with no possible
/// values.
pub fn never<T>(never: Never) -> T {
    match never {}
}

/// A trait extension for binary functions (`Fn(A, B) -> O`).
///
/// It enables you to use a bunch of nifty functional programming paradigms
/// that work well with taim.
pub trait Function<A, B, O> {
    /// Applies the given first argument to a binary function and returns
    /// a new function that takes the other argument.
    ///
    /// This lets you partially "apply" a function—equivalent to currying,
    /// but it only works with binary functions. If you want to apply an
    /// arbitrary number of arguments, create a little struct for them.
    ///
    /// # When is this useful?
    /// Sometimes you will want to identify the source or target
    /// of some message in your user interface. This can be achieved through
    /// normal means by defining a closure and moving the identifier
    /// inside:
    ///
    /// ```rust
    /// # let element: Option<()> = Some(());
    /// # enum Message { ButtonPressed(u32, ()) }
    /// let id = 123;
    ///
    /// # let _ = {
    /// element.map(move |result| Message::ButtonPressed(id, result))
    /// # };
    /// ```
    ///
    /// That's quite a mouthful. [`with`](Self::with) lets you write:
    ///
    /// ```rust
    /// # use taim_core::Function;
    /// # let element: Option<()> = Some(());
    /// # enum Message { ButtonPressed(u32, ()) }
    /// let id = 123;
    ///
    /// # let _ = {
    /// element.map(Message::ButtonPressed.with(id))
    /// # };
    /// ```
    ///
    /// Effectively creating the same closure that partially applies
    /// the `id` to the message—but much more concise!
    fn with(self, prefix: A) -> impl Fn(B) -> O;
}

impl<F, A, B, O> Function<A, B, O> for F
where
    F: Fn(A, B) -> O,
    Self: Sized,
    A: Clone,
{
    fn with(self, prefix: A) -> impl Fn(B) -> O {
        move |result| self(prefix.clone(), result)
    }
}
