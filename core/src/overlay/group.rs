use crate::overlay;

mod implementation;

/// An [`Overlay`] container that displays multiple overlay [`overlay::Element`]
/// children.
pub struct Group<'a, Message, Theme, Renderer> {
    pub(super) children: Vec<overlay::Element<'a, Message, Theme, Renderer>>,
}

impl<'a, Message, Theme, Renderer> Group<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a + crate::Renderer,
{
    /// Creates an empty [`Group`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a [`Group`] with the given elements.
    pub fn with_children(
        mut children: Vec<overlay::Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        // Overlay groups are commonly assembled in render order already.
        // Detect that linear-time fast path and only pay O(n log n) when the
        // caller actually supplied an inversion. `total_cmp` also gives NaN
        // indices a stable, deterministic side of the order.
        let inverted = children.windows(2).any(|pair| {
            pair[0]
                .as_overlay()
                .index()
                .total_cmp(&pair[1].as_overlay().index())
                .is_gt()
        });

        if inverted {
            children.sort_unstable_by(|left, right| {
                left.as_overlay()
                    .index()
                    .total_cmp(&right.as_overlay().index())
            });
        }

        Group { children }
    }

    /// Turns the [`Group`] into an overlay [`overlay::Element`].
    pub fn overlay(self) -> overlay::Element<'a, Message, Theme, Renderer> {
        overlay::Element::new(Box::new(self))
    }
}

impl<'a, Message, Theme, Renderer> Default for Group<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a + crate::Renderer,
{
    fn default() -> Self {
        Self::with_children(Vec::new())
    }
}

impl<'a, Message, Theme, Renderer> From<Group<'a, Message, Theme, Renderer>>
    for overlay::Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: 'a + crate::Renderer,
{
    fn from(group: Group<'a, Message, Theme, Renderer>) -> Self {
        group.overlay()
    }
}
