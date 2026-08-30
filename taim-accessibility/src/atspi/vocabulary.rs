//! Translation between Taim's semantic vocabulary and AT-SPI2's.
//!
//! These are pure functions over a **wire ABI**: the numbers below are
//! fixed by AT-SPI2 itself (`AtspiRole`/`AtspiStateType` in
//! at-spi2-core), and a screen reader on the other end of the bus reads
//! them by value. Getting one wrong does not fail loudly — it announces
//! the wrong thing.
//!
//! Every constant here was read out of the installed at-spi2-core via
//! its GObject introspection data, not recalled. That is not fussiness:
//! a first draft of this file was written from memory and had **11 of 17
//! roles and 7 of 11 states wrong** — `PUSH_BUTTON` as 44 rather than 43,
//! `GROUPING` as 26 rather than 99 — and its tests pinned those wrong
//! numbers and passed. Re-derive them the same way before changing any:
//!
//! ```text
//! python3 -c "import gi; gi.require_version('Atspi','2.0')
//! from gi.repository import Atspi; print(int(Atspi.Role.PUSH_BUTTON))"
//! ```
//!
//! Kept apart from the D-Bus transport deliberately: the mapping is the
//! part that decides what a user *hears*, and it should be verifiable
//! without a bus, a session, or an async runtime.

use taim_core::semantics::{Actions, Live, Role, State};

/// An AT-SPI2 role, as the `u32` sent over the bus.
///
/// Values from at-spi2-core's `AtspiRole` enum. Only the ones Taim can
/// produce are named; the enum itself has ~130 members.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AtspiRole {
    /// `ATSPI_ROLE_INVALID`, the fallback for a [`Role`] this mapping
    /// does not know.
    Invalid = 0,
    /// `ATSPI_ROLE_PUSH_BUTTON`
    PushButton = 43,
    /// `ATSPI_ROLE_CHECK_BOX`
    CheckBox = 7,
    /// `ATSPI_ROLE_RADIO_BUTTON`
    RadioButton = 44,
    /// `ATSPI_ROLE_SLIDER`
    Slider = 51,
    /// `ATSPI_ROLE_ENTRY`
    Entry = 79,
    /// `ATSPI_ROLE_PASSWORD_TEXT`
    PasswordText = 40,
    /// `ATSPI_ROLE_LABEL`
    Label = 29,
    /// `ATSPI_ROLE_IMAGE`
    Image = 27,
    /// `ATSPI_ROLE_SCROLL_PANE`
    ScrollPane = 49,
    /// `ATSPI_ROLE_PANEL`
    Panel = 39,
    /// `ATSPI_ROLE_LINK`
    Link = 88,
    /// `ATSPI_ROLE_PAGE_TAB`
    PageTab = 37,
    /// `ATSPI_ROLE_MENU_ITEM`
    MenuItem = 35,
    /// `ATSPI_ROLE_LIST_ITEM`
    ListItem = 32,
    /// `ATSPI_ROLE_FRAME`
    Frame = 23,
}

/// Maps a [`Role`] to its AT-SPI2 equivalent.
///
/// Two mappings are worth stating outright, because they are choices
/// rather than lookups:
///
/// - [`Role::SecureField`] becomes `PasswordText`, **not** `Entry`. The
///   role is itself a privacy signal: a screen reader that sees
///   `PasswordText` suppresses echo and braille routing of the content.
///   Reporting `Entry` for a password field would defeat every guard on
///   the Taim side, since the platform would treat it as ordinary text.
/// - [`Role::NamedGroup`] becomes `Panel`, same as [`Role::Group`].
///   AT-SPI has no separate "named group" role; what distinguishes one is
///   that it carries a name, which travels on the node regardless.
///   (`ATSPI_ROLE_GROUPING` exists, but means a *form* grouping — a
///   fieldset — not a generic container.)
///
/// [`Role`] is `#[non_exhaustive]`, so a role added upstream lands on
/// `Invalid` rather than being quietly announced as something it is not.
pub fn role(role: Role) -> AtspiRole {
    match role {
        Role::Button => AtspiRole::PushButton,
        Role::Checkbox => AtspiRole::CheckBox,
        Role::RadioButton => AtspiRole::RadioButton,
        Role::Slider => AtspiRole::Slider,
        Role::TextField => AtspiRole::Entry,
        Role::SecureField => AtspiRole::PasswordText,
        Role::StaticText => AtspiRole::Label,
        Role::Image => AtspiRole::Image,
        Role::ScrollView => AtspiRole::ScrollPane,
        Role::Link => AtspiRole::Link,
        Role::Tab => AtspiRole::PageTab,
        Role::MenuItem => AtspiRole::MenuItem,
        Role::ListItem => AtspiRole::ListItem,
        Role::Window => AtspiRole::Frame,
        Role::Group | Role::NamedGroup => AtspiRole::Panel,
        _ => AtspiRole::Invalid,
    }
}

/// The machine name AT-SPI2 gives a role, as `GetRoleName` reports it.
///
/// Read from `Atspi.role_get_name`, so these match what a reader sees
/// from every other toolkit on the bus — a reader keying off `"push
/// button"` where GTK says `"button"` would treat Taim as a stranger.
pub fn role_name(role: Role) -> &'static str {
    match self::role(role) {
        AtspiRole::PushButton => "button",
        AtspiRole::CheckBox => "check box",
        AtspiRole::RadioButton => "radio button",
        AtspiRole::Slider => "slider",
        AtspiRole::Entry => "entry",
        AtspiRole::PasswordText => "password text",
        AtspiRole::Label => "label",
        AtspiRole::Image => "image",
        AtspiRole::ScrollPane => "scroll pane",
        AtspiRole::Panel => "panel",
        AtspiRole::Link => "link",
        AtspiRole::PageTab => "page tab",
        AtspiRole::MenuItem => "menu item",
        AtspiRole::ListItem => "list item",
        AtspiRole::Frame => "frame",
        AtspiRole::Invalid => "invalid",
    }
}

/// AT-SPI2 state bits, as indices into the `StateSet` bitfield.
///
/// Values from at-spi2-core's `AtspiStateType`. A `StateSet` goes over
/// the bus as two `u32`s (low and high words), so these are bit indices,
/// not masks.
pub mod state {
    /// `ATSPI_STATE_BUSY`
    pub const BUSY: u32 = 3;
    /// `ATSPI_STATE_CHECKED`
    pub const CHECKED: u32 = 4;
    /// `ATSPI_STATE_ENABLED`
    pub const ENABLED: u32 = 8;
    /// `ATSPI_STATE_EXPANDED`
    pub const EXPANDED: u32 = 10;
    /// `ATSPI_STATE_FOCUSABLE`
    pub const FOCUSABLE: u32 = 11;
    /// `ATSPI_STATE_FOCUSED`
    pub const FOCUSED: u32 = 12;
    /// `ATSPI_STATE_SELECTED`
    pub const SELECTED: u32 = 23;
    /// `ATSPI_STATE_SENSITIVE`
    pub const SENSITIVE: u32 = 24;
    /// `ATSPI_STATE_SHOWING`
    pub const SHOWING: u32 = 25;
    /// `ATSPI_STATE_VISIBLE`
    pub const VISIBLE: u32 = 30;
    /// `ATSPI_STATE_INDETERMINATE`
    ///
    /// Note the value: this one lives in the **high** word, which is why
    /// [`super::state_words`] is not academic.
    pub const INDETERMINATE: u32 = 32;
}

/// Maps a [`State`] to an AT-SPI2 `StateSet` bitfield.
///
/// The inversions are the substance here. Taim reports what is *wrong*
/// with a node (`DISABLED`, `HIDDEN`) because that is the exceptional
/// case worth carrying; AT-SPI reports what is *right* with it
/// (`ENABLED`, `SENSITIVE`, `SHOWING`, `VISIBLE`) and expects those
/// present by default. A direct bit-for-bit copy would therefore mark
/// **every** node disabled and invisible — a silent, total failure, since
/// each bit is individually plausible.
pub fn state_set(state: State) -> u64 {
    let mut bits = 0u64;
    let mut set = |bit: u32| bits |= 1 << bit;

    let disabled = state.contains(State::DISABLED);
    let hidden = state.contains(State::HIDDEN);

    if !disabled {
        set(state::ENABLED);
        set(state::SENSITIVE);
    }

    if !hidden {
        set(state::SHOWING);
        set(state::VISIBLE);
    }

    if state.contains(State::FOCUSABLE) {
        set(state::FOCUSABLE);
    }

    if state.contains(State::FOCUSED) {
        set(state::FOCUSED);
    }

    if state.contains(State::CHECKED) {
        set(state::CHECKED);
    }

    if state.contains(State::INDETERMINATE) {
        set(state::INDETERMINATE);
    }

    if state.contains(State::SELECTED) {
        set(state::SELECTED);
    }

    if state.contains(State::EXPANDED) {
        set(state::EXPANDED);
    }

    if state.contains(State::BUSY) {
        set(state::BUSY);
    }

    bits
}

/// Splits a `StateSet` into the `(low, high)` `u32` pair AT-SPI2's
/// `GetState` returns.
pub fn state_words(bits: u64) -> (u32, u32) {
    (bits as u32, (bits >> 32) as u32)
}

/// One AT-SPI2 action, as exposed through `org.a11y.atspi.Action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    /// The action's machine name, matched by assistive technology.
    pub name: &'static str,
    /// A short human-readable description.
    pub description: &'static str,
}

/// Lists the AT-SPI2 actions a node advertises, in a stable order.
///
/// Order matters and is not cosmetic: `org.a11y.atspi.Action.DoAction`
/// takes an **index** into this list, so the order is part of the wire
/// contract. It must depend only on which bits are set — never on
/// iteration order — or the action a user invokes would vary run to run.
///
/// An empty [`Actions`] yields an empty list, which is the whole point of
/// the "a disabled control advertises no actions" rule: assistive
/// technology offers the user nothing rather than something inert.
pub fn actions(actions: Actions) -> Vec<Action> {
    let mut list = Vec::new();

    let mut push = |flag: Actions, name, description| {
        if actions.contains(flag) {
            list.push(Action { name, description });
        }
    };

    push(Actions::ACTIVATE, "click", "Activate this control");
    push(Actions::FOCUS, "grabFocus", "Move focus here");
    push(Actions::INCREMENT, "increment", "Increase the value");
    push(Actions::DECREMENT, "decrement", "Decrease the value");
    push(Actions::SET_VALUE, "setValue", "Set the value");
    push(
        Actions::TOGGLE_EXPANDED,
        "expandOrCollapse",
        "Expand or collapse",
    );
    push(
        Actions::SCROLL_INTO_VIEW,
        "scrollIntoView",
        "Scroll into view",
    );
    push(Actions::DISMISS, "dismiss", "Dismiss");

    list
}

/// Maps a [`Live`] politeness to the `live` object attribute AT-SPI2
/// carries (the same vocabulary as ARIA's `aria-live`).
pub fn live(live: Live) -> &'static str {
    match live {
        Live::Polite => "polite",
        Live::Assertive => "assertive",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_match_the_atspi_abi() {
        // Pinned against at-spi2-core's `AtspiRole`. A screen reader reads
        // these by value, so a wrong number announces the wrong thing
        // rather than failing.
        assert_eq!(role(Role::Button) as u32, 43);
        assert_eq!(role(Role::Checkbox) as u32, 7);
        assert_eq!(role(Role::RadioButton) as u32, 44);
        assert_eq!(role(Role::Slider) as u32, 51);
        assert_eq!(role(Role::TextField) as u32, 79);
        assert_eq!(role(Role::SecureField) as u32, 40);
        assert_eq!(role(Role::StaticText) as u32, 29);
        assert_eq!(role(Role::Image) as u32, 27);
        assert_eq!(role(Role::ScrollView) as u32, 49);
        assert_eq!(role(Role::Link) as u32, 88);
        assert_eq!(role(Role::Tab) as u32, 37);
        assert_eq!(role(Role::MenuItem) as u32, 35);
        assert_eq!(role(Role::ListItem) as u32, 32);
        assert_eq!(role(Role::Window) as u32, 23);
        assert_eq!(role(Role::Group) as u32, 39);
        assert_eq!(role(Role::NamedGroup) as u32, 39);
    }

    #[test]
    fn a_secure_field_is_password_text_not_an_entry() {
        // The role is itself a privacy signal: `PasswordText` makes a
        // screen reader suppress echo and braille routing. Reporting
        // `Entry` would hand the platform an ordinary text field and
        // defeat every guard on the Taim side.
        assert_eq!(role(Role::SecureField), AtspiRole::PasswordText);
        assert_ne!(role(Role::SecureField), role(Role::TextField));
    }

    #[test]
    fn an_ordinary_node_is_enabled_and_visible() {
        // The inversion that matters. Taim reports what is *wrong*
        // (`DISABLED`, `HIDDEN`); AT-SPI reports what is *right*
        // (`ENABLED`, `SENSITIVE`, `SHOWING`, `VISIBLE`) and expects them
        // present by default. Copying the bits across would mark every
        // node disabled and invisible.
        let bits = state_set(State::empty());

        for (bit, name) in [
            (state::ENABLED, "ENABLED"),
            (state::SENSITIVE, "SENSITIVE"),
            (state::SHOWING, "SHOWING"),
            (state::VISIBLE, "VISIBLE"),
        ] {
            assert!(
                bits & (1 << bit) != 0,
                "a node with no flags set must still be {name} to AT-SPI"
            );
        }
    }

    #[test]
    fn a_disabled_node_is_neither_enabled_nor_sensitive() {
        let bits = state_set(State::DISABLED);

        assert_eq!(bits & (1 << state::ENABLED), 0);
        assert_eq!(bits & (1 << state::SENSITIVE), 0);
        assert!(
            bits & (1 << state::SHOWING) != 0,
            "disabled is not hidden -- a dimmed control is still on screen"
        );
    }

    #[test]
    fn a_hidden_node_is_neither_showing_nor_visible() {
        let bits = state_set(State::HIDDEN);

        assert_eq!(bits & (1 << state::SHOWING), 0);
        assert_eq!(bits & (1 << state::VISIBLE), 0);
        assert!(
            bits & (1 << state::ENABLED) != 0,
            "hidden is not disabled -- the two are independent"
        );
    }

    #[test]
    fn state_flags_map_across() {
        let bits = state_set(State::CHECKED | State::FOCUSED | State::FOCUSABLE);

        assert!(bits & (1 << state::CHECKED) != 0);
        assert!(bits & (1 << state::FOCUSED) != 0);
        assert!(bits & (1 << state::FOCUSABLE) != 0);
    }

    #[test]
    fn indeterminate_crosses_into_the_high_word() {
        // `ATSPI_STATE_INDETERMINATE` is bit 32, so it is the one state
        // Taim can produce that a 32-bit-only split would drop entirely:
        // a partially-checked checkbox would announce as plain unchecked.
        let (low, high) = state_words(state_set(State::INDETERMINATE));

        assert_eq!(low & (1 << 4), 0, "it must not land in the low word");
        assert!(high & 1 != 0, "bit 32 is bit 0 of the high word");
    }

    #[test]
    fn state_words_split_low_and_high() {
        let (low, high) = state_words((1 << 4) | (1 << 33));

        assert_eq!(low, 1 << 4);
        assert_eq!(high, 1 << 1);
    }

    #[test]
    fn a_disabled_control_advertises_no_actions() {
        // The authoring rule, enforced at the boundary: an action offered
        // to a user that silently does nothing is worse than no action.
        assert!(actions(Actions::empty()).is_empty());
    }

    #[test]
    fn actions_are_listed_in_a_stable_order() {
        // `DoAction` takes an index into this list, so the order is wire
        // contract: it must depend only on which bits are set.
        let names: Vec<_> = actions(Actions::FOCUS | Actions::ACTIVATE)
            .iter()
            .map(|action| action.name)
            .collect();

        assert_eq!(names, vec!["click", "grabFocus"]);

        assert_eq!(
            actions(Actions::ACTIVATE | Actions::FOCUS),
            actions(Actions::FOCUS | Actions::ACTIVATE),
            "the order must not depend on how the flags were combined"
        );
    }

    #[test]
    fn a_sliders_actions_survive_the_crossing() {
        let names: Vec<_> = actions(Actions::INCREMENT | Actions::DECREMENT | Actions::SET_VALUE)
            .iter()
            .map(|action| action.name)
            .collect();

        assert_eq!(names, vec!["increment", "decrement", "setValue"]);
    }

    #[test]
    fn role_names_match_what_every_other_toolkit_reports() {
        // Read from `Atspi.role_get_name`. A reader keying off "push
        // button" where GTK says "button" would treat Taim as a stranger.
        assert_eq!(role_name(Role::Button), "button");
        assert_eq!(role_name(Role::Checkbox), "check box");
        assert_eq!(role_name(Role::RadioButton), "radio button");
        assert_eq!(role_name(Role::SecureField), "password text");
        assert_eq!(role_name(Role::StaticText), "label");
        assert_eq!(role_name(Role::Window), "frame");
        assert_eq!(role_name(Role::Group), "panel");
    }

    #[test]
    fn live_politeness_uses_the_aria_vocabulary() {
        assert_eq!(live(Live::Polite), "polite");
        assert_eq!(live(Live::Assertive), "assertive");
    }
}
