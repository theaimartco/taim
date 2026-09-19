//! Keyed columns distribute content vertically while keeping continuity.
mod implementation;

use crate::core::{Alignment, Element, Length, Padding, Pixels};

/// A container that distributes its contents vertically while keeping continuity.
///
/// # Example
/// ```no_run
/// # mod taim { pub mod widget { pub use taim_widget::*; } }
/// # pub type State = ();
/// # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
/// use taim::widget::{keyed_column, text};
///
/// enum Message {
///     // ...
/// }
///
/// fn view(state: &State) -> Element<'_, Message> {
///     keyed_column((0..=100).map(|i| {
///         (i, text!("Item {i}").into())
///     })).into()
/// }
/// ```
pub struct Column<'a, Key, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Key: Copy + Eq + std::hash::Hash,
{
    spacing: f32,
    padding: Padding,
    width: Length,
    height: Length,
    align_items: Alignment,
    keys: Vec<Key>,
    children: Vec<Element<'a, Message, Theme, Renderer>>,
}

impl<'a, Key, Message, Theme, Renderer> Column<'a, Key, Message, Theme, Renderer>
where
    Key: Copy + Eq + std::hash::Hash,
    Renderer: crate::core::Renderer,
{
    /// Creates an empty [`Column`].
    pub fn new() -> Self {
        Self::from_vecs(Vec::new(), Vec::new())
    }

    /// Creates a [`Column`] from already allocated [`Vec`]s.
    ///
    /// Keep in mind that the [`Column`] will not inspect the [`Vec`]s, which means
    /// it won't automatically adapt to the sizing strategy of its contents.
    ///
    /// If any of the children have a [`Length::Fill`] strategy, you will need to
    /// call [`Column::width`] or [`Column::height`] accordingly.
    pub fn from_vecs(
        mut keys: Vec<Key>,
        mut children: Vec<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let paired_len = keys.len().min(children.len());
        keys.truncate(paired_len);
        children.truncate(paired_len);

        // Compact both parallel buffers in place so void widgets can never
        // desynchronize identity from state.
        let mut read = 0;
        let mut write = 0;
        children.retain(|child| {
            let keep = !child.as_widget().is_void();

            if keep {
                keys[write] = keys[read];
                write += 1;
            }

            read += 1;
            keep
        });
        keys.truncate(write);

        Self {
            spacing: 0.0,
            padding: Padding::ZERO,
            width: Length::Fit,
            height: Length::Fit,
            align_items: Alignment::Start,
            keys,
            children,
        }
    }

    /// Creates a [`Column`] with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_vecs(Vec::with_capacity(capacity), Vec::with_capacity(capacity))
    }

    /// Creates a [`Column`] with the given elements.
    pub fn with_children(
        children: impl IntoIterator<Item = (Key, Element<'a, Message, Theme, Renderer>)>,
    ) -> Self {
        let iterator = children.into_iter();

        Self::with_capacity(iterator.size_hint().0).extend(iterator)
    }

    /// Sets the vertical spacing _between_ elements.
    ///
    /// Custom margins per element do not exist in taim. You should use this
    /// method instead! While less flexible, it helps you keep spacing between
    /// elements consistent.
    pub fn spacing(mut self, amount: impl Into<Pixels>) -> Self {
        let amount = amount.into().0;
        self.spacing = if amount.is_finite() {
            amount.max(0.0)
        } else {
            0.0
        };
        self
    }

    /// Sets the [`Padding`] of the [`Column`].
    pub fn padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the width of the [`Column`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Column`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the horizontal alignment of the contents of the [`Column`] .
    pub fn align_items(mut self, align: Alignment) -> Self {
        self.align_items = align;
        self
    }

    /// Adds an element to the [`Column`].
    pub fn push(
        mut self,
        key: Key,
        child: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let child = child.into();

        if !child.as_widget().is_void() {
            self.keys.push(key);
            self.children.push(child);
        }

        self
    }

    /// Adds an element to the [`Column`], if `Some`.
    pub fn push_maybe(
        self,
        key: Key,
        child: Option<impl Into<Element<'a, Message, Theme, Renderer>>>,
    ) -> Self {
        if let Some(child) = child {
            self.push(key, child)
        } else {
            self
        }
    }

    /// Extends the [`Column`] with the given children.
    pub fn extend(
        mut self,
        children: impl IntoIterator<Item = (Key, Element<'a, Message, Theme, Renderer>)>,
    ) -> Self {
        let iterator = children.into_iter();
        let additional = iterator.size_hint().0;
        self.keys.reserve(additional);
        self.children.reserve(additional);

        for (key, child) in iterator {
            if !child.as_widget().is_void() {
                self.keys.push(key);
                self.children.push(child);
            }
        }

        self
    }
}

impl<Key, Message, Renderer> Default for Column<'_, Key, Message, Renderer>
where
    Key: Copy + Eq + std::hash::Hash,
    Renderer: crate::core::Renderer,
{
    fn default() -> Self {
        Self::new()
    }
}
