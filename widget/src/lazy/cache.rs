#![allow(dead_code)]
use crate::core::Element;
use crate::core::overlay;

use ouroboros::self_referencing;

#[self_referencing(pub_extras)]
pub struct Cache<'a, Message: 'a, Theme: 'a, Renderer: 'a> {
    pub element: Element<'a, Message, Theme, Renderer>,

    #[borrows(mut element)]
    #[covariant]
    overlay: Option<overlay::Element<'this, Message, Theme, Renderer>>,
}

/// Construct a retained lazy-element cache without an attached overlay.
///
/// This follows the same self-referential ownership contract as the
/// generated ouroboros cache builder while avoiding a stale borrowed
/// overlay when a virtualized item is freshly attached to a widget tree.
impl<'a, Message: 'a, Theme: 'a, Renderer: 'a> Cache<'a, Message, Theme, Renderer> {
    pub fn without_overlay(element: Element<'a, Message, Theme, Renderer>) -> Self {
        CacheBuilder {
            element,
            overlay_builder: |_| None,
        }
        .build()
    }
}
