use crate::core::{self, Event};
use crate::{backend, clipboard, font, image, system, window};

use std::fmt;

/// An action that the taim runtime can perform.
pub enum Action<T> {
    /// Output some value.
    Output(T),

    /// Run a widget operation.
    Widget(Box<dyn core::widget::Operation>),

    /// Run a clipboard action.
    Clipboard(clipboard::Action),

    /// Run a window action.
    Window(window::Action),

    /// Run a system action.
    System(system::Action),

    /// Run a font action.
    Font(font::Action),

    /// Run an image action.
    Image(image::Action),

    /// Run a backend action.
    Backend(backend::Action),

    /// Produce an event.
    Event {
        /// The [`window::Id`](core::window::Id) of the event.
        window: core::window::Id,
        /// The [`Event`] to be produced.
        event: Event,
    },

    /// Poll any resources that may have pending computations.
    Tick,

    /// Recreate all user interfaces and redraw all windows.
    Reload,

    /// Exits the runtime.
    ///
    /// This will normally close any application windows and
    /// terminate the runtime loop.
    Exit,
}

impl<T> Action<T> {
    /// Creates a new [`Action::Widget`] with the given [`widget::Operation`](core::widget::Operation).
    #[inline]
    pub fn widget(operation: impl core::widget::Operation + 'static) -> Self {
        Self::Widget(Box::new(operation))
    }

    #[inline]
    pub(crate) fn output<O>(self) -> Result<T, Action<O>> {
        match self {
            Action::Output(output) => Ok(output),
            Action::Widget(operation) => Err(Action::Widget(operation)),
            Action::Clipboard(action) => Err(Action::Clipboard(action)),
            Action::Window(action) => Err(Action::Window(action)),
            Action::System(action) => Err(Action::System(action)),
            Action::Font(action) => Err(Action::Font(action)),
            Action::Image(action) => Err(Action::Image(action)),
            Action::Backend(action) => Err(Action::Backend(action)),
            Action::Event { window, event } => Err(Action::Event { window, event }),
            Action::Tick => Err(Action::Tick),
            Action::Reload => Err(Action::Reload),
            Action::Exit => Err(Action::Exit),
        }
    }
}

impl<T> fmt::Debug for Action<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Output(output) => write!(f, "Action::Output({output:?})"),
            Action::Widget { .. } => {
                write!(f, "Action::Widget")
            }
            Action::Clipboard(action) => {
                write!(f, "Action::Clipboard({action:?})")
            }
            Action::Window(_) => write!(f, "Action::Window"),
            Action::System(action) => write!(f, "Action::System({action:?})"),
            Action::Font(action) => {
                write!(f, "Action::Font({action:?})")
            }
            Action::Image(action) => write!(f, "Action::Image({action:?})"),
            Action::Backend(action) => write!(f, "Action::Backend({action:?})"),
            Action::Event { window, event } => write!(
                f,
                "Action::Event {{ window: {window:?}, event: {event:?} }}"
            ),
            Action::Tick => write!(f, "Action::Tick"),
            Action::Reload => write!(f, "Action::Reload"),
            Action::Exit => write!(f, "Action::Exit"),
        }
    }
}
