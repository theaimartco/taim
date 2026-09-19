//! Pick lists display a dropdown list of selectable options.
//!
//! # Example
//! ```no_run
//! # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
//! # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
//! #
//! use taim::widget::pick_list;
//!
//! struct State {
//!    favorite: Option<Fruit>,
//! }
//!
//! #[derive(Debug, Clone, Copy, PartialEq, Eq)]
//! enum Fruit {
//!     Apple,
//!     Orange,
//!     Strawberry,
//!     Tomato,
//! }
//!
//! #[derive(Debug, Clone)]
//! enum Message {
//!     FruitSelected(Fruit),
//! }
//!
//! fn view(state: &State) -> Element<'_, Message> {
//!     let fruits = [
//!         Fruit::Apple,
//!         Fruit::Orange,
//!         Fruit::Strawberry,
//!         Fruit::Tomato,
//!     ];
//!
//!     pick_list(
//!         state.favorite,
//!         fruits,
//!         Fruit::to_string,
//!     )
//!     .on_select(Message::FruitSelected)
//!     .placeholder("Select your favorite fruit...")
//!     .into()
//! }
//!
//! fn update(state: &mut State, message: Message) {
//!     match message {
//!         Message::FruitSelected(fruit) => {
//!             state.favorite = Some(fruit);
//!         }
//!     }
//! }
//!
//! impl std::fmt::Display for Fruit {
//!     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//!         f.write_str(match self {
//!             Self::Apple => "Apple",
//!             Self::Orange => "Orange",
//!             Self::Strawberry => "Strawberry",
//!             Self::Tomato => "Tomato",
//!         })
//!     }
//! }
//! ```
mod behavior;
mod handle;
mod state;
mod style;

pub use handle::{Handle, Icon};
pub use style::{Catalog, Status, Style, StyleFn, default};

use state::State;

use crate::core::alignment;
use crate::core::keyboard;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::text::{self, Text};
use crate::core::touch;
use crate::core::widget::tree::{self, Tree};
use crate::core::{
    Element, Event, Layout, Length, Padding, Pixels, Point, Rectangle, Shell, Size, Vector, Widget,
};
use crate::overlay::menu::{self, Menu};

use std::borrow::Borrow;
use std::f32;

struct Events<'a, T, Message> {
    select: Option<Box<dyn Fn(T) -> Message + 'a>>,
    lifecycle: Option<Box<Lifecycle<Message>>>,
}

impl<T, Message> Default for Events<'_, T, Message> {
    fn default() -> Self {
        Self {
            select: None,
            lifecycle: None,
        }
    }
}

impl<T, Message> Events<'_, T, Message> {
    fn lifecycle_mut(&mut self) -> &mut Lifecycle<Message> {
        self.lifecycle
            .get_or_insert_with(|| Box::new(Lifecycle::default()))
    }
}

struct Lifecycle<Message> {
    open: Option<Message>,
    close: Option<Message>,
}

struct TextFormat<Font> {
    size: Option<Pixels>,
    font: Option<Font>,
    line_height: text::LineHeight,
    shaping: text::Shaping,
    ellipsis: text::Ellipsis,
}

impl<Font> Default for TextFormat<Font> {
    fn default() -> Self {
        Self {
            size: None,
            font: None,
            line_height: text::LineHeight::default(),
            shaping: text::Shaping::default(),
            ellipsis: text::Ellipsis::End,
        }
    }
}

impl<Message> Default for Lifecycle<Message> {
    fn default() -> Self {
        Self {
            open: None,
            close: None,
        }
    }
}

/// A widget for selecting a single value from a list of options.
///
/// # Example
/// ```no_run
/// # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
/// # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
/// #
/// use taim::widget::pick_list;
///
/// struct State {
///    favorite: Option<Fruit>,
/// }
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// enum Fruit {
///     Apple,
///     Orange,
///     Strawberry,
///     Tomato,
/// }
///
/// #[derive(Debug, Clone)]
/// enum Message {
///     FruitSelected(Fruit),
/// }
///
/// fn view(state: &State) -> Element<'_, Message> {
///     let fruits = [
///         Fruit::Apple,
///         Fruit::Orange,
///         Fruit::Strawberry,
///         Fruit::Tomato,
///     ];
///
///     pick_list(
///         state.favorite,
///         fruits,
///         Fruit::to_string,
///     )
///     .on_select(Message::FruitSelected)
///     .placeholder("Select your favorite fruit...")
///     .into()
/// }
///
/// fn update(state: &mut State, message: Message) {
///     match message {
///         Message::FruitSelected(fruit) => {
///             state.favorite = Some(fruit);
///         }
///     }
/// }
///
/// impl std::fmt::Display for Fruit {
///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
///         f.write_str(match self {
///             Self::Apple => "Apple",
///             Self::Orange => "Orange",
///             Self::Strawberry => "Strawberry",
///             Self::Tomato => "Tomato",
///         })
///     }
/// }
/// ```
pub struct PickList<'a, T, L, V, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    T: PartialEq + Clone,
    L: Borrow<[T]> + 'a,
    V: Borrow<T> + 'a,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    options: L,
    to_string: Box<dyn Fn(&T) -> String + 'a>,
    events: Events<'a, T, Message>,
    placeholder: Option<String>,
    selected: Option<V>,
    width: Length,
    padding: Padding,
    text: TextFormat<Renderer::Font>,
    handle: Handle<Renderer::Font>,
    class: <Theme as Catalog>::Class<'a>,
    menu_class: <Theme as menu::Catalog>::Class<'a>,
    menu_height: Length,
}

impl<'a, T, L, V, Message, Theme, Renderer> PickList<'a, T, L, V, Message, Theme, Renderer>
where
    T: PartialEq + Clone,
    L: Borrow<[T]> + 'a,
    V: Borrow<T> + 'a,
    Message: Clone,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    /// Creates a new [`PickList`] with the given list of options, the current
    /// selected value, and the message to produce when an option is selected.
    pub fn new(selected: Option<V>, options: L, to_string: impl Fn(&T) -> String + 'a) -> Self {
        Self {
            to_string: Box::new(to_string),
            events: Events::default(),
            options,
            placeholder: None,
            selected,
            width: Length::Shrink,
            padding: crate::button::DEFAULT_PADDING,
            text: TextFormat::default(),
            handle: Handle::default(),
            class: <Theme as Catalog>::default(),
            menu_class: <Theme as Catalog>::default_menu(),
            menu_height: Length::Shrink,
        }
    }

    /// Sets the placeholder of the [`PickList`].
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Sets the width of the [`PickList`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Menu`].
    pub fn menu_height(mut self, menu_height: impl Into<Length>) -> Self {
        self.menu_height = menu_height.into();
        self
    }

    /// Sets the [`Padding`] of the [`PickList`].
    pub fn padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the text size of the [`PickList`].
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text.size = Some(size.into());
        self
    }

    /// Sets the text [`text::LineHeight`] of the [`PickList`].
    pub fn line_height(mut self, line_height: impl Into<text::LineHeight>) -> Self {
        self.text.line_height = line_height.into();
        self
    }

    /// Sets the [`text::Shaping`] strategy of the [`PickList`].
    pub fn shaping(mut self, shaping: text::Shaping) -> Self {
        self.text.shaping = shaping;
        self
    }

    /// Sets the [`text::Ellipsis`] strategy of the [`PickList`].
    pub fn ellipsis(mut self, ellipsis: text::Ellipsis) -> Self {
        self.text.ellipsis = ellipsis;
        self
    }

    /// Sets the font of the [`PickList`].
    pub fn font(mut self, font: impl Into<Renderer::Font>) -> Self {
        self.text.font = Some(font.into());
        self
    }

    /// Sets the [`Handle`] of the [`PickList`].
    pub fn handle(mut self, handle: Handle<Renderer::Font>) -> Self {
        self.handle = handle;
        self
    }

    /// Sets the message that will be produced when the [`PickList`] selected value changes.
    pub fn on_select(mut self, on_select: impl Fn(T) -> Message + 'a) -> Self {
        self.events.select = Some(Box::new(on_select));
        self
    }

    /// Sets the message that will be produced when the [`PickList`] is opened.
    pub fn on_open(mut self, on_open: Message) -> Self {
        self.events.lifecycle_mut().open = Some(on_open);
        self
    }

    /// Sets the message that will be produced when the [`PickList`] is closed.
    pub fn on_close(mut self, on_close: Message) -> Self {
        self.events.lifecycle_mut().close = Some(on_close);
        self
    }

    /// Returns the number of selectable options without allocating labels.
    pub fn option_count(&self) -> usize {
        self.options.borrow().len()
    }

    /// Returns whether selection input is currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.events.select.is_some()
    }

    /// Sets the style of the [`PickList`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style of the [`Menu`].
    #[must_use]
    pub fn menu_style(mut self, style: impl Fn(&Theme) -> menu::Style + 'a) -> Self
    where
        <Theme as menu::Catalog>::Class<'a>: From<menu::StyleFn<'a, Theme>>,
    {
        self.menu_class = (Box::new(style) as menu::StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the [`PickList`].
    #[cfg(feature = "advanced")]
    #[must_use]
    pub fn class(mut self, class: impl Into<<Theme as Catalog>::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    /// Sets the style class of the [`Menu`].
    #[cfg(feature = "advanced")]
    #[must_use]
    pub fn menu_class(mut self, class: impl Into<<Theme as menu::Catalog>::Class<'a>>) -> Self {
        self.menu_class = class.into();
        self
    }
}

impl<'a, T, L, V, Message, Theme, Renderer> From<PickList<'a, T, L, V, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    T: Clone + PartialEq + 'a,
    L: Borrow<[T]> + 'a,
    V: Borrow<T> + 'a,
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: text::Renderer + 'a,
{
    fn from(pick_list: PickList<'a, T, L, V, Message, Theme, Renderer>) -> Self {
        Self::new(pick_list)
    }
}
