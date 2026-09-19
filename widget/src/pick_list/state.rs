//! Persistent widget-tree state for pick lists.

use crate::core::keyboard;
use crate::core::text::{self, paragraph};
use crate::overlay::menu;

#[derive(Debug)]
pub(super) struct State<P: text::Paragraph> {
    pub(super) menu: menu::State,
    pub(super) keyboard_modifiers: keyboard::Modifiers,
    pub(super) is_open: bool,
    pub(super) hovered_option: Option<usize>,
    pub(super) options: Vec<paragraph::Plain<P>>,
    pub(super) placeholder: paragraph::Plain<P>,
    pub(super) label: paragraph::Plain<P>,
    pub(super) closed_handle: paragraph::Plain<P>,
    pub(super) open_handle: paragraph::Plain<P>,
    pub(super) label_is_placeholder: bool,
    pub(super) is_hovered: bool,
}

impl<P: text::Paragraph> State<P> {
    /// Creates a new [`State`] for a [`PickList`].
    pub(super) fn new() -> Self {
        Self {
            menu: menu::State::default(),
            keyboard_modifiers: keyboard::Modifiers::default(),
            is_open: bool::default(),
            hovered_option: Option::default(),
            options: Vec::new(),
            placeholder: paragraph::Plain::default(),
            label: paragraph::Plain::default(),
            closed_handle: paragraph::Plain::default(),
            open_handle: paragraph::Plain::default(),
            label_is_placeholder: true,
            is_hovered: false,
        }
    }
}

impl<P: text::Paragraph> Default for State<P> {
    fn default() -> Self {
        Self::new()
    }
}
