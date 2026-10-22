//! Type-erased widgets.
use crate::widget;
use crate::{Color, Widget};

use std::borrow::{Borrow, BorrowMut};

mod explain;
mod map;

use explain::Explain;
use map::Map;

/// Any [`Widget`], boxed behind one type.
///
/// Views return `Element`s so a parent never needs to know which concrete
/// widgets its children are. Every built-in widget converts into one with
/// `.into()`.
pub struct Element<'a, Message, Theme, Renderer> {
    widget: Box<dyn Widget<Message, Theme, Renderer> + 'a>,
}

impl<'a, Message, Theme, Renderer> Element<'a, Message, Theme, Renderer> {
    /// Boxes `widget`.
    pub fn new(widget: impl Widget<Message, Theme, Renderer> + 'a) -> Self
    where
        Renderer: crate::Renderer,
    {
        Self {
            widget: Box::new(widget),
        }
    }

    /// The boxed widget.
    pub fn as_widget(&self) -> &dyn Widget<Message, Theme, Renderer> {
        &*self.widget
    }

    /// The boxed widget, mutably.
    pub fn as_widget_mut(&mut self) -> &mut dyn Widget<Message, Theme, Renderer> {
        &mut *self.widget
    }

    /// Converts every message the element produces with `f`.
    ///
    /// This is how a reusable component plugs into a larger application: the
    /// component speaks its own message type, and the parent wraps those
    /// messages into its own.
    ///
    /// ```
    /// # type Element<'a, M> = taim_core::Element<'a, M, taim_core::Theme, ()>;
    /// mod slider {
    ///     #[derive(Debug, Clone)]
    ///     pub enum Message { Moved(f32) }
    /// }
    ///
    /// #[derive(Debug, Clone)]
    /// enum Message {
    ///     Volume(slider::Message),
    /// }
    ///
    /// fn volume<'a>(slider: Element<'a, slider::Message>) -> Element<'a, Message> {
    ///     slider.map(Message::Volume)
    /// }
    /// ```
    pub fn map<B>(self, f: impl Fn(Message) -> B + 'a) -> Element<'a, B, Theme, Renderer>
    where
        Message: 'a,
        Theme: 'a,
        Renderer: crate::Renderer + 'a,
        B: 'a,
    {
        Element::new(Map::new(self.widget, f))
    }

    /// Outlines the element and all of its descendants in `color`, to debug
    /// layout.
    pub fn explain(self, color: impl Into<Color>) -> Element<'a, Message, Theme, Renderer>
    where
        Message: 'a,
        Theme: 'a,
        Renderer: crate::Renderer + 'a,
    {
        let color = color.into();

        Element {
            widget: Box::new(Explain::new(self, color)),
        }
    }
}

/// Lets widget-tree helpers accept elements, element references and plain
/// widget references interchangeably.
macro_rules! borrow_widget {
    ($($holder:ty => $($mutable:ident)?;)+) => {
        $(
            impl<'a, Message, Theme, Renderer> Borrow<dyn Widget<Message, Theme, Renderer> + 'a>
                for $holder
            {
                fn borrow(&self) -> &(dyn Widget<Message, Theme, Renderer> + 'a) {
                    &*self.widget
                }
            }

            $(
                borrow_widget!(@$mutable $holder);
            )?
        )+
    };
    (@mutable $holder:ty) => {
        impl<'a, Message, Theme, Renderer> BorrowMut<dyn Widget<Message, Theme, Renderer> + 'a>
            for $holder
        {
            fn borrow_mut(&mut self) -> &mut (dyn Widget<Message, Theme, Renderer> + 'a) {
                &mut *self.widget
            }
        }
    };
}

borrow_widget! {
    Element<'a, Message, Theme, Renderer> => mutable;
    &Element<'a, Message, Theme, Renderer> => ;
    &mut Element<'a, Message, Theme, Renderer> => mutable;
}

/// `None` becomes an empty, zero-sized element, so optional children can be
/// passed straight into containers.
impl<'a, T, Message, Theme, Renderer> From<Option<T>> for Element<'a, Message, Theme, Renderer>
where
    T: Into<Self>,
    Renderer: crate::Renderer,
{
    fn from(child: Option<T>) -> Self {
        match child {
            Some(child) => child.into(),
            None => Element::new(widget::Void),
        }
    }
}

impl<'a, Message, Theme, Renderer> From<widget::Void> for Element<'a, Message, Theme, Renderer>
where
    Renderer: crate::Renderer,
{
    fn from(void: widget::Void) -> Self {
        Element::new(void)
    }
}
