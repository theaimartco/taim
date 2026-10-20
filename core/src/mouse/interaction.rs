/// The cursor interaction requested by a TAIM UI layer.
///
/// Legacy cursor variants are retained for applications using the established
/// widget API. TAIM's layer compositor resolves overlapping requests using
/// **paint order**, not the enum's numeric ordering.
#[derive(Debug, Eq, PartialEq, Clone, Copy, PartialOrd, Ord, Default)]
#[allow(missing_docs)]
pub enum Interaction {
    #[default]
    None,
    Hidden,
    Idle,
    ContextMenu,
    Help,
    Pointer,
    Progress,
    Wait,
    Cell,
    Crosshair,
    Text,
    Alias,
    Copy,
    Move,
    NoDrop,
    NotAllowed,
    Grab,
    Grabbing,
    ResizingHorizontally,
    ResizingVertically,
    ResizingDiagonallyUp,
    ResizingDiagonallyDown,
    ResizingColumn,
    ResizingRow,
    AllScroll,
    ZoomIn,
    ZoomOut,
}

impl Interaction {
    /// Resolve a set of overlay cursor requests in painter's order.
    ///
    /// The last painted, non-`None` layer takes precedence. `Hidden` and
    /// `Idle` are *intentional* cursor decisions and must not be skipped.
    /// The double-ended iterator allows checking the highest layer first,
    /// short-circuiting without evaluating covered overlays.
    pub fn topmost(layers: impl DoubleEndedIterator<Item = Self>) -> Self {
        layers
            .rev()
            .find(|interaction| *interaction != Self::None)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::Interaction;

    #[test]
    fn foreground_pointer_overrides_background_resize() {
        let cursor = Interaction::topmost(
            [Interaction::ResizingHorizontally, Interaction::Pointer].into_iter(),
        );
        assert_eq!(cursor, Interaction::Pointer);
    }

    #[test]
    fn noninteractive_front_layer_reveals_underlying_cursor() {
        let cursor = Interaction::topmost(
            [Interaction::Text, Interaction::None].into_iter(),
        );
        assert_eq!(cursor, Interaction::Text);
    }

    #[test]
    fn hiding_cursor_is_a_real_foreground_decision() {
        let cursor = Interaction::topmost(
            [Interaction::Pointer, Interaction::Hidden].into_iter(),
        );
        assert_eq!(cursor, Interaction::Hidden);
        assert_eq!(Interaction::topmost(std::iter::empty()), Interaction::None);
    }
}
