//! The built-in views.
mod button;
mod container;
mod space;
mod stack;
mod text;

pub use button::{Button, button};
pub use container::{Container, container};
pub use space::{Space, space};
pub use stack::{Align, Stack, column, row};
pub use text::{Text, text};

/// A list of children of mixed view types: `column(views![a, b, c])`.
#[macro_export]
macro_rules! views {
    ($($child:expr),* $(,)?) => {
        [$($crate::AnyView::from($child)),*]
    };
}
