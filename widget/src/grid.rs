//! Distribute content on a grid.
mod engine;
mod implementation;

use engine::Constraint;

use crate::core::{Element, Length, Pixels};

/// A container that distributes its contents on a responsive grid.
pub struct Grid<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer> {
    spacing: f32,
    columns: Constraint,
    width: Option<Pixels>,
    height: Sizing,
    children: Vec<Element<'a, Message, Theme, Renderer>>,
}

impl<'a, Message, Theme, Renderer> Grid<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    /// Creates an empty [`Grid`].
    pub fn new() -> Self {
        Self::from_vec(Vec::new())
    }

    /// Creates a [`Grid`] with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_vec(Vec::with_capacity(capacity))
    }

    /// Creates a [`Grid`] with the given elements.
    pub fn with_children(
        children: impl IntoIterator<Item = Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let iterator = children.into_iter();
        let mut grid = Self::with_capacity(iterator.size_hint().0);
        grid.append(iterator);
        grid
    }

    /// Creates a [`Grid`] from an already allocated [`Vec`].
    pub fn from_vec(mut children: Vec<Element<'a, Message, Theme, Renderer>>) -> Self {
        children.retain(|child| !child.as_widget().is_void());

        Self {
            spacing: 0.0,
            columns: Constraint::Amount(3),
            width: None,
            height: Sizing::AspectRatio(1.0),
            children,
        }
    }

    /// Sets the spacing _between_ cells in the [`Grid`].
    pub fn spacing(mut self, amount: impl Into<Pixels>) -> Self {
        self.spacing = finite_nonnegative(amount.into().0);
        self
    }

    /// Sets the width of the [`Grid`] in [`Pixels`].
    ///
    /// By default, a [`Grid`] will [`Fill`] its parent.
    ///
    /// [`Fill`]: Length::Fill
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = Some(Pixels(finite_nonnegative(width.into().0)));
        self
    }

    /// Sets the height of the [`Grid`].
    ///
    /// By default, a [`Grid`] uses a cell aspect ratio of `1.0` (i.e. squares).
    pub fn height(mut self, height: impl Into<Sizing>) -> Self {
        self.height = height.into().normalized();
        self
    }

    /// Sets the amount of columns in the [`Grid`].
    pub fn columns(mut self, column: usize) -> Self {
        self.columns = Constraint::Amount(column.max(1));
        self
    }

    /// Makes the amount of columns dynamic in the [`Grid`], never
    /// exceeding the provided `max_width`.
    pub fn fluid(mut self, max_width: impl Into<Pixels>) -> Self {
        let max_width = max_width.into().0;

        self.columns = Constraint::MaxWidth(Pixels(
            if max_width.is_finite() && max_width > 0.0 { max_width } else { 1.0 },
        ));
        self
    }

    /// Adds an [`Element`] to the [`Grid`].
    pub fn push(mut self, child: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        let child = child.into();

        if !child.as_widget().is_void() {
            self.children.push(child);
        }

        self
    }

    /// Adds an element to the [`Grid`], if `Some`.
    pub fn push_maybe(
        self,
        child: Option<impl Into<Element<'a, Message, Theme, Renderer>>>,
    ) -> Self {
        if let Some(child) = child {
            self.push(child)
        } else {
            self
        }
    }

    /// Returns the number of drawable children currently retained.
    pub fn len(&self) -> usize {
        self.children.len()
    }

    /// Returns whether the grid has no drawable children.
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    /// Reserves storage for at least `additional` more children.
    pub fn reserve(&mut self, additional: usize) {
        self.children.reserve(additional);
    }

    /// Returns the number of child slots available before reallocation.
    pub fn capacity(&self) -> usize {
        self.children.capacity()
    }

    /// Removes every child while retaining the existing allocation.
    pub fn clear(&mut self) {
        self.children.clear();
    }

    /// Inserts a drawable child at `index`, clamping the index to the end.
    /// Returns `false` when the child is void and therefore ignored.
    pub fn insert(
        &mut self,
        index: usize,
        child: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> bool {
        let child = child.into();
        if child.as_widget().is_void() {
            return false;
        }

        self.children.insert(index.min(self.children.len()), child);
        true
    }

    /// Removes and returns a child when `index` is in bounds.
    pub fn remove(&mut self, index: usize) -> Option<Element<'a, Message, Theme, Renderer>> {
        (index < self.children.len()).then(|| self.children.remove(index))
    }

    /// Appends drawable children without consuming the grid builder.
    pub fn append(
        &mut self,
        children: impl IntoIterator<Item = Element<'a, Message, Theme, Renderer>>,
    ) {
        let iterator = children.into_iter();
        self.children.reserve(iterator.size_hint().0);
        self.children.extend(
            iterator.filter(|child| !child.as_widget().is_void()),
        );
    }

    /// Extends the [`Grid`] with the given children.
    pub fn extend(
        mut self,
        children: impl IntoIterator<Item = Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.append(children);
        self
    }
}

impl<Message, Theme, Renderer> Default for Grid<'_, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, Message, Theme, Renderer> Extend<Element<'a, Message, Theme, Renderer>>
    for Grid<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn extend<T: IntoIterator<Item = Element<'a, Message, Theme, Renderer>>>(
        &mut self,
        iter: T,
    ) {
        self.append(iter);
    }
}

impl<'a, Message, Theme, Renderer: crate::core::Renderer>
    FromIterator<Element<'a, Message, Theme, Renderer>> for Grid<'a, Message, Theme, Renderer>
{
    fn from_iter<T: IntoIterator<Item = Element<'a, Message, Theme, Renderer>>>(iter: T) -> Self {
        Self::with_children(iter)
    }
}

/// The sizing strategy of a [`Grid`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sizing {
    /// The [`Grid`] will ensure each cell follows the given aspect ratio and the
    /// total size will be the sum of the cells and the spacing between them.
    ///
    /// The ratio is the amount of horizontal pixels per each vertical pixel of a cell
    /// in the [`Grid`].
    AspectRatio(f32),

    /// The [`Grid`] will evenly distribute the space available in the given [`Length`]
    /// for each cell.
    EvenlyDistribute(Length),
}

impl From<f32> for Sizing {
    fn from(height: f32) -> Self {
        Self::EvenlyDistribute(Length::from(finite_nonnegative(height)))
    }
}

impl From<Length> for Sizing {
    fn from(height: Length) -> Self {
        Self::EvenlyDistribute(height)
    }
}

/// Creates a new [`Sizing`] strategy that maintains the given aspect ratio.
pub fn aspect_ratio(width: impl Into<Pixels>, height: impl Into<Pixels>) -> Sizing {
    let width = width.into().0;
    let height = height.into().0;
    let ratio = width / height;

    Sizing::AspectRatio(
        if ratio.is_finite() && ratio > 0.0 { ratio } else { 1.0 },
    )
}

impl Sizing {
    #[inline]
    fn normalized(self) -> Self {
        match self {
            Self::AspectRatio(ratio) => Self::AspectRatio(
                if ratio.is_finite() && ratio > 0.0 { ratio } else { 1.0 },
            ),
            Self::EvenlyDistribute(Length::Fixed(height)) => {
                Self::EvenlyDistribute(Length::Fixed(finite_nonnegative(height)))
            }
            other @ Self::EvenlyDistribute(_) => other,
        }
    }
}

#[inline]
fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}
