//! The seam a real platform accessibility bridge plugs into.
//!
//! Every platform's accessibility API wants something different (AT-SPI
//! on Linux, UIA on Windows, `AXUIElement` on macOS, and whatever VYOM's
//! own shell exposes), and none of them want the whole tree every frame.
//! [`Adapter`] is the narrow contract between this crate's retained,
//! diffed tree and any of them.
//!
//! # Capability negotiation
//!
//! Not every platform supports every feature: some have no live regions,
//! some cannot express a focus trap, some ignore geometry entirely. An
//! adapter declares what it can do via [`Capabilities`], and the engine
//! **withholds** what it cannot -- rather than sending calls the adapter
//! will drop on the floor, or worse, forwarding an announcement to a
//! platform that has nowhere to put it.
//!
//! # No adapter ships here
//!
//! This module deliberately contains the trait and the negotiation, and
//! no platform implementation. A real bridge belongs with the backend
//! that owns the platform connection (Phase 14's native VYOM backend is
//! the first candidate); putting one here would make this crate depend on
//! a platform and stop being the neutral engine everything else can use.
//! `docs/accessibility/contract.md` records that gap explicitly.

use crate::live_region::Announcement;
use crate::node::Entry;
use crate::tree::Change;

use aura_core::widget::Id;

bitflags::bitflags! {
    /// What a platform adapter can actually do.
    ///
    /// Declared by the adapter, honored by [`Session`]. A capability that
    /// is absent means the engine must not attempt that call at all.
    ///
    /// `Default` is deliberately *empty*, not [`Capabilities::ALL`]: an
    /// adapter that forgets to declare anything must receive nothing,
    /// rather than being handed calls it never claimed to handle.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Capabilities: u8 {
        /// The platform can consume incremental tree changes. Without
        /// this, [`Session::publish`] sends nothing and the adapter is
        /// expected to pull the whole tree when it wants it.
        const INCREMENTAL_UPDATES = 1 << 0;
        /// The platform can announce live regions.
        const LIVE_REGIONS = 1 << 1;
        /// The platform tracks focus itself and wants to be told about
        /// focus moves.
        const FOCUS_NOTIFICATIONS = 1 << 2;
        /// The platform uses node geometry (for hit-testing, magnifier
        /// tracking, or braille routing).
        const GEOMETRY = 1 << 3;
    }
}

impl Capabilities {
    /// A fully-featured platform.
    pub const ALL: Self = Self::INCREMENTAL_UPDATES
        .union(Self::LIVE_REGIONS)
        .union(Self::FOCUS_NOTIFICATIONS)
        .union(Self::GEOMETRY);
}

/// A platform accessibility bridge.
///
/// Every method has a default no-op body, so an adapter implements only
/// what it declared in [`Adapter::capabilities`] -- and an adapter that
/// declares a capability but forgets to implement it silently does
/// nothing rather than failing to compile. That asymmetry is deliberate:
/// the alternative is forcing every adapter to write empty bodies for
/// features its platform lacks.
pub trait Adapter {
    /// What this adapter supports. Queried once per [`Session`].
    fn capabilities(&self) -> Capabilities;

    /// Applies a batch of tree changes.
    ///
    /// Only called when [`Capabilities::INCREMENTAL_UPDATES`] is
    /// declared. The batch is the exact output of
    /// [`Tree::diff`](crate::tree::Tree::diff): removals first, then the
    /// rest in traversal order.
    fn update(&mut self, _changes: &[Change]) {}

    /// Announces live-region messages, assertive first.
    ///
    /// Only called when [`Capabilities::LIVE_REGIONS`] is declared. Every
    /// announcement reaching here has already passed
    /// [`LiveRegions::announce`](crate::live_region::LiveRegions::announce)'s
    /// privacy checks.
    fn announce(&mut self, _announcements: &[Announcement]) {}

    /// Reports that focus moved to `focused` (or away from everything).
    ///
    /// Only called when [`Capabilities::FOCUS_NOTIFICATIONS`] is
    /// declared.
    fn focus_changed(&mut self, _focused: Option<&Id>) {}
}

/// Drives one [`Adapter`], honoring its declared [`Capabilities`].
///
/// The negotiation lives here rather than in each adapter so that "this
/// platform has no live regions" is stated once, as data, instead of
/// being re-implemented as an early return in every bridge.
#[derive(Debug)]
pub struct Session<A: Adapter> {
    adapter: A,
    capabilities: Capabilities,
    last_focus: Option<Id>,
}

impl<A: Adapter> Session<A> {
    /// Negotiates capabilities with `adapter` and begins a session.
    pub fn new(adapter: A) -> Self {
        let capabilities = adapter.capabilities();

        Self {
            adapter,
            capabilities,
            last_focus: None,
        }
    }

    /// The capabilities negotiated at construction.
    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    /// Forwards tree changes, if the platform can consume them.
    ///
    /// Changes that are purely geometric are filtered out for a platform
    /// that declared no [`Capabilities::GEOMETRY`]: it has nothing to do
    /// with them, and a scroll would otherwise flood it with updates it
    /// will discard.
    pub fn publish(&mut self, changes: &[Change]) {
        if !self
            .capabilities
            .contains(Capabilities::INCREMENTAL_UPDATES)
        {
            return;
        }

        if self.capabilities.contains(Capabilities::GEOMETRY) {
            if !changes.is_empty() {
                self.adapter.update(changes);
            }
            return;
        }

        let relevant: Vec<Change> = changes
            .iter()
            .filter(|change| !matches!(change, Change::Moved { .. }))
            .cloned()
            .collect();

        if !relevant.is_empty() {
            self.adapter.update(&relevant);
        }
    }

    /// Forwards announcements, if the platform has live regions.
    pub fn announce(&mut self, announcements: &[Announcement]) {
        if !self.capabilities.contains(Capabilities::LIVE_REGIONS) || announcements.is_empty() {
            return;
        }

        self.adapter.announce(announcements);
    }

    /// Forwards a focus change, if the platform wants one and focus
    /// actually moved.
    ///
    /// De-duplicated here so an adapter never sees "focus moved" for a
    /// focus that did not move -- a screen reader would re-announce the
    /// control.
    pub fn focus_changed(&mut self, focused: Option<&Id>) {
        if !self
            .capabilities
            .contains(Capabilities::FOCUS_NOTIFICATIONS)
        {
            return;
        }

        if self.last_focus.as_ref() == focused {
            return;
        }

        self.last_focus = focused.cloned();
        self.adapter.focus_changed(focused);
    }

    /// Borrows the underlying adapter.
    pub fn adapter(&self) -> &A {
        &self.adapter
    }
}

/// Whether `entry` is worth reporting to a platform at all.
///
/// A hidden node is present in the widget tree but must not appear in the
/// accessibility tree: it is mid-transition, or deliberately withheld.
pub fn is_exposed(entry: &Entry) -> bool {
    !entry.state.contains(aura_core::semantics::State::HIDDEN)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::live_region::Urgency;
    use crate::node::Order;
    use aura_core::semantics::{Actions, Role, State};
    use aura_core::{Rectangle, Size};

    #[derive(Debug, Default)]
    struct Recorder {
        capabilities: Capabilities,
        updates: Vec<Vec<Change>>,
        announcements: Vec<Vec<Announcement>>,
        focus_changes: Vec<Option<Id>>,
    }

    impl Adapter for Recorder {
        fn capabilities(&self) -> Capabilities {
            self.capabilities
        }

        fn update(&mut self, changes: &[Change]) {
            self.updates.push(changes.to_vec());
        }

        fn announce(&mut self, announcements: &[Announcement]) {
            self.announcements.push(announcements.to_vec());
        }

        fn focus_changed(&mut self, focused: Option<&Id>) {
            self.focus_changes.push(focused.cloned());
        }
    }

    fn recorder(capabilities: Capabilities) -> Recorder {
        Recorder {
            capabilities,
            ..Recorder::default()
        }
    }

    fn entry(id: Id) -> Entry {
        Entry {
            id,
            parent: None,
            order: Order::root(0),
            bounds: Rectangle::with_size(Size::new(10.0, 10.0)),
            role: Role::Button,
            state: State::empty(),
            name: None,
            description: None,
            value: None,
            actions: Actions::empty(),
            live: None,
        }
    }

    #[test]
    fn a_platform_without_incremental_updates_receives_none() {
        let mut session = Session::new(recorder(Capabilities::LIVE_REGIONS));

        session.publish(&[Change::Added(entry(Id::unique()))]);

        assert!(
            session.adapter().updates.is_empty(),
            "the engine must not call what the adapter said it cannot do"
        );
    }

    #[test]
    fn a_platform_without_geometry_is_not_flooded_by_scrolling() {
        // A scroll produces a Moved for every visible node. A platform
        // that ignores geometry has nothing to do with them.
        let mut session = Session::new(recorder(Capabilities::INCREMENTAL_UPDATES));

        let moved: Vec<Change> = (0..50)
            .map(|_| Change::Moved {
                id: Id::unique(),
                bounds: Rectangle::with_size(Size::new(1.0, 1.0)),
            })
            .collect();

        session.publish(&moved);

        assert!(
            session.adapter().updates.is_empty(),
            "pure geometry churn must not reach a platform that ignores geometry"
        );
    }

    #[test]
    fn a_platform_with_geometry_receives_moves() {
        let mut session = Session::new(recorder(
            Capabilities::INCREMENTAL_UPDATES | Capabilities::GEOMETRY,
        ));

        session.publish(&[Change::Moved {
            id: Id::unique(),
            bounds: Rectangle::with_size(Size::new(1.0, 1.0)),
        }]);

        assert_eq!(session.adapter().updates.len(), 1);
    }

    #[test]
    fn a_platform_without_geometry_still_receives_real_changes() {
        let mut session = Session::new(recorder(Capabilities::INCREMENTAL_UPDATES));
        let id = Id::unique();

        session.publish(&[
            Change::Moved {
                id: Id::unique(),
                bounds: Rectangle::with_size(Size::new(1.0, 1.0)),
            },
            Change::Added(entry(id.clone())),
        ]);

        assert_eq!(session.adapter().updates.len(), 1);
        assert_eq!(
            session.adapter().updates[0].len(),
            1,
            "the Added must survive; only the Moved is filtered"
        );
    }

    #[test]
    fn a_platform_without_live_regions_hears_nothing() {
        let mut session = Session::new(recorder(Capabilities::INCREMENTAL_UPDATES));

        session.announce(&[Announcement {
            source: Id::unique(),
            message: "hello".into(),
            urgency: Urgency::Polite,
        }]);

        assert!(session.adapter().announcements.is_empty());
    }

    #[test]
    fn focus_notifications_are_deduplicated() {
        // A screen reader would re-announce the control every time.
        let mut session = Session::new(recorder(Capabilities::ALL));
        let id = Id::unique();

        session.focus_changed(Some(&id));
        session.focus_changed(Some(&id));
        session.focus_changed(Some(&id));

        assert_eq!(
            session.adapter().focus_changes.len(),
            1,
            "focus that did not move must not be reported"
        );

        session.focus_changed(None);
        assert_eq!(session.adapter().focus_changes.len(), 2);
    }

    #[test]
    fn empty_batches_are_never_forwarded() {
        let mut session = Session::new(recorder(Capabilities::ALL));

        session.publish(&[]);
        session.announce(&[]);

        assert!(session.adapter().updates.is_empty());
        assert!(session.adapter().announcements.is_empty());
    }

    #[test]
    fn a_hidden_node_is_not_exposed() {
        let mut hidden = entry(Id::unique());
        hidden.state = State::HIDDEN;

        assert!(!is_exposed(&hidden));
        assert!(is_exposed(&entry(Id::unique())));
    }
}
