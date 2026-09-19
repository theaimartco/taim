use crate::{Rectangle, Transformation, layout, semantics};

/// Cold, optional data kept away from the traversal-critical [`Tree`] fields.
///
/// Most widget nodes never opt in to retained layout metadata. Keeping these
/// values behind one lazy pointer makes ordinary tree walking touch fewer cache
/// lines while preserving every public cache contract.
#[derive(Debug, Default)]
pub(super) struct Cache {
    pub(super) bounds: Option<Rectangle>,
    pub(super) transform: Option<Transformation>,
    pub(super) clip: Option<Rectangle>,
    pub(super) layout: Option<(layout::LimitsKey, layout::Node)>,
    pub(super) semantics: Option<Option<semantics::Node>>,
}
