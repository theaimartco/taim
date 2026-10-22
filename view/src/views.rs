//! The built-in views.
mod button;
mod container;
mod dynamic;
mod layers;
mod look;
mod range;
mod scroll;
mod space;
mod stack;
mod text;
mod text_field;
mod toggle;

pub use button::{Button, button};
pub use container::{Container, container};
pub use dynamic::{Each, Rebuild, each, rebuild, when};
pub use layers::{Layers, layers};
pub use range::{Progress, Rule, Slider, progress, rule, slider};
pub use scroll::{Scroll, scroll};
pub use space::{Space, space};
pub use stack::{Align, Stack, column, row};
pub use text::{Text, text};
pub use text_field::{TextField, text_field};
pub use toggle::{Toggle, checkbox, switch};

/// A list of children of mixed view types: `column(views![a, b, c])`.
#[macro_export]
macro_rules! views {
    ($($child:expr),* $(,)?) => {
        [$($crate::AnyView::from($child)),*]
    };
}
