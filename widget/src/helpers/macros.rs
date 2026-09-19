//! Declarative collection and text helpers.

/// Creates a [`Column`] with the given children.
///
/// Columns distribute their children vertically.
///
/// # Example
/// ```no_run
/// # mod taim { pub mod widget { pub use taim_widget::*; } }
/// # pub type State = ();
/// # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
/// use taim::widget::{button, column};
///
/// #[derive(Debug, Clone)]
/// enum Message {
///     // ...
/// }
///
/// fn view(state: &State) -> Element<'_, Message> {
///     column![
///         "I am on top!",
///         button("I am in the center!"),
///         "I am below.",
///     ].into()
/// }
/// ```
#[macro_export]
macro_rules! column {
    () => (
        $crate::Column::new()
    );
    ($($x:expr),+ $(,)?) => (
        $crate::Column::with_children([$($crate::core::Element::from($x)),+])
    );
}

/// Creates a [`Row`] with the given children.
///
/// Rows distribute their children horizontally.
///
/// # Example
/// ```no_run
/// # mod taim { pub mod widget { pub use taim_widget::*; } }
/// # pub type State = ();
/// # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
/// use taim::widget::{button, row};
///
/// #[derive(Debug, Clone)]
/// enum Message {
///     // ...
/// }
///
/// fn view(state: &State) -> Element<'_, Message> {
///     row![
///         "I am to the left!",
///         button("I am in the middle!"),
///         "I am to the right!",
///     ].into()
/// }
/// ```
#[macro_export]
macro_rules! row {
    () => (
        $crate::Row::new()
    );
    ($($x:expr),+ $(,)?) => (
        $crate::Row::with_children([$($crate::core::Element::from($x)),+])
    );
}

/// Creates a [`Stack`] with the given children.
///
/// [`Stack`]: crate::Stack
#[macro_export]
macro_rules! stack {
    () => (
        $crate::Stack::new()
    );
    ($($x:expr),+ $(,)?) => (
        $crate::Stack::with_children([$($crate::core::Element::from($x)),+])
    );
}

/// Creates a [`Grid`] with the given children.
///
/// [`Grid`]: crate::Grid
#[macro_export]
macro_rules! grid {
    () => (
        $crate::Grid::new()
    );
    ($($x:expr),+ $(,)?) => (
        $crate::Grid::with_children([$($crate::core::Element::from($x)),+])
    );
}

/// Creates a new [`Text`] widget with the provided content.
///
/// [`Text`]: core::widget::Text
///
/// This macro uses the same syntax as [`format!`], but creates a new [`Text`] widget instead.
///
/// See [the formatting documentation in `std::fmt`](std::fmt)
/// for details of the macro argument syntax.
///
/// # Examples
///
/// ```no_run
/// # mod taim {
/// #     pub mod widget {
/// #         macro_rules! text {
/// #           ($($arg:tt)*) => {unimplemented!()}
/// #         }
/// #         pub(crate) use text;
/// #     }
/// # }
/// # pub type State = ();
/// # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::core::Theme, ()>;
/// use taim::widget::text;
///
/// enum Message {
///     // ...
/// }
///
/// fn view(_state: &State) -> Element<Message> {
///     let simple = text!("Hello, world!");
///
///     let keyword = text!("Hello, {}", "world!");
///
///     let planet = "Earth";
///     let local_variable = text!("Hello, {planet}!");
///     // ...
///     # unimplemented!()
/// }
/// ```
#[macro_export]
macro_rules! text {
    ($($arg:tt)*) => {
        $crate::Text::new(format!($($arg)*))
    };
}

/// Creates some [`Rich`] text with the given spans.
///
/// [`Rich`]: text::Rich
///
/// # Example
/// ```no_run
/// # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::core::*; }
/// # pub type State = ();
/// # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
/// use taim::font;
/// use taim::widget::{rich_text, span};
/// use taim::{color, never, Font};
///
/// #[derive(Debug, Clone)]
/// enum Message {
///     // ...
/// }
///
/// fn view(state: &State) -> Element<'_, Message> {
///     rich_text![
///         span("I am red!").color(color!(0xff0000)),
///         span(" "),
///         span("And I am bold!").font(Font { weight: font::Weight::Bold, ..Font::default() }),
///     ]
///     .on_link_click(never)
///     .size(20)
///     .into()
/// }
/// ```
#[macro_export]
macro_rules! rich_text {
    () => (
        $crate::text::Rich::new()
    );
    ($($x:expr),+ $(,)?) => (
        $crate::text::Rich::from_iter([$($crate::text::Span::from($x)),+])
    );
}
