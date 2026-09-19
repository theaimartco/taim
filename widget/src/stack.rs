//! Display content on top of other content.
use crate::core::layout;
use crate::core::{Element, Length};

#[path = "stack/implementation.rs"]
mod implementation;

/// The internal, per-instance state of a [`Stack`]: a bounded cache of
/// the base layer's own measurement, keyed on the [`layout::Limits`] the
/// whole [`Stack`] is asked to resolve under.
///
/// Every non-base layer is measured against `Limits::new(Size::ZERO,
/// size)`, where `size` is derived entirely from the base layer's
/// measurement -- so an unchanged base layer measured under unchanged
/// [`layout::Limits`] means every layer's resulting geometry is
/// reproducible from the cache without re-running any layer's `layout`.
#[derive(Debug, Default)]
struct State {
    base: layout::Cache,
}

/// A container that displays children on top of each other.
///
/// The first [`Element`] dictates the intrinsic [`crate::core::Size`] of a [`Stack`] and
/// will be displayed as the base layer. Every consecutive [`Element`] will be
/// rendered on top; on its own layer.
///
/// You can use [`push_under`](Self::push_under) to push an [`Element`] under
/// the current [`Stack`] without affecting its intrinsic [`Size`].
///
/// Keep in mind that too much layering will normally produce bad UX as well as
/// introduce certain rendering overhead. Use this widget sparingly!
pub struct Stack<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer> {
    width: Length,
    height: Length,
    children: Vec<Element<'a, Message, Theme, Renderer>>,
    clip: bool,
    base_layer: usize,
}

impl<'a, Message, Theme, Renderer> Stack<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    /// Creates an empty [`Stack`].
    pub fn new() -> Self {
        Self::from_vec(Vec::new())
    }

    /// Creates a [`Stack`] with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_vec(Vec::with_capacity(capacity))
    }

    /// Creates a [`Stack`] with the given elements.
    pub fn with_children(
        children: impl IntoIterator<Item = Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let iterator = children.into_iter();

        Self::with_capacity(iterator.size_hint().0).extend(iterator)
    }

    /// Creates a [`Stack`] from an already allocated [`Vec`].
    pub fn from_vec(mut children: Vec<Element<'a, Message, Theme, Renderer>>) -> Self {
        children.retain(|child| !child.as_widget().is_void());

        Self {
            width: Length::Fit,
            height: Length::Fit,
            children,
            clip: false,
            base_layer: 0,
        }
    }

    /// Sets the width of the [`Stack`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Stack`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Adds an element on top of the [`Stack`].
    pub fn push(mut self, child: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        let child = child.into();

        if !child.as_widget().is_void() {
            self.children.push(child);
        }

        self
    }

    /// Adds an element under the [`Stack`].
    pub fn push_under(mut self, child: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        let child = child.into();

        if child.as_widget().is_void() {
            return self;
        }

        if self.children.is_empty() {
            // A sole layer is necessarily the intrinsic base. Advancing the
            // index here would make the only child unreachable during layout.
            self.children.push(child);
        } else {
            self.children.insert(0, child);
            self.base_layer += 1;
        }

        self
    }

    /// Extends the [`Stack`] with the given children.
    pub fn extend(
        mut self,
        children: impl IntoIterator<Item = Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let iterator = children.into_iter();
        self.children.reserve(iterator.size_hint().0);

        for child in iterator {
            if !child.as_widget().is_void() {
                self.children.push(child);
            }
        }

        self
    }

    /// Sets whether the [`Stack`] should clip overflowing content.
    ///
    /// It has a slight performance overhead during presentation.
    ///
    /// By default, it is set to `false`.
    pub fn clip(mut self, clip: bool) -> Self {
        self.clip = clip;
        self
    }
}

impl<Message, Renderer> Default for Stack<'_, Message, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, Message, Theme, Renderer> From<Stack<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: crate::core::Renderer + 'a,
{
    fn from(stack: Stack<'a, Message, Theme, Renderer>) -> Self {
        Self::new(stack)
    }
}
