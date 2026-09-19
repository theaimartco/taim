use crate::core::{self, Element, Length, Size};
use crate::space;

mod implementation;

/// A widget that is aware of its dimensions.
///
/// A [`Responsive`] widget will always try to fill all the available space of
/// its parent.
pub struct Responsive<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer> {
    pub(super) view: Box<dyn Fn(Size) -> Element<'a, Message, Theme, Renderer> + 'a>,
    pub(super) width: Length,
    pub(super) height: Length,
    pub(super) content: Element<'a, Message, Theme, Renderer>,
    /// Last dimensions used to materialize `content` on this widget instance.
    /// A repeated layout pass can reuse the already-diffed subtree.
    pub(super) built_for: Option<DimensionKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DimensionKey(u32, u32);

impl DimensionKey {
    #[inline]
    pub(super) fn from_size(size: Size) -> Self {
        Self(size.width.to_bits(), size.height.to_bits())
    }
}

impl<'a, Message, Theme, Renderer> Responsive<'a, Message, Theme, Renderer>
where
    Renderer: core::Renderer,
{
    /// Creates a new [`Responsive`] widget with a closure that produces its
    /// contents.
    ///
    /// The `view` closure will receive the maximum available space for
    /// the [`Responsive`] during layout. You can use this [`Size`] to
    /// conditionally build the contents.
    pub fn new<E>(view: impl Fn(Size) -> E + 'a) -> Self
    where
        E: Into<Element<'a, Message, Theme, Renderer>>,
    {
        Self {
            view: Box::new(move |size| view(size).into()),
            width: Length::Fill,
            height: Length::Fill,
            content: Element::new(space()),
            built_for: None,
        }
    }

    /// Sets the width of the [`Responsive`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Responsive`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }
}
