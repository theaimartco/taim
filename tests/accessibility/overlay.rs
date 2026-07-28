//! Overlays report semantics.
//!
//! Before `Overlay::semantics` existed, **every overlay was invisible to
//! the accessibility engine**: `aura_runtime::semantics::build` walks the
//! widget tree, and an overlay is reached through `Widget::overlay`
//! instead, so nothing ever asked it.
//!
//! The practical effect was that an open `pick_list` announced nothing --
//! a screen-reader user could hear the closed control, open it, and find
//! the entire list of choices unreachable. These tests pin that the hook
//! exists, forwards correctly, and is actually implemented where it
//! mattered.

use aura::advanced::widget::{self, Widget};
use aura::advanced::{Layout, layout, mouse, overlay};
use aura::widget::pick_list;
use aura::{Point, Rectangle, Size, Theme, Vector};
use aura_core::semantics::{Node, Role, State};
use aura_core::widget::Id;
use aura_renderer::Renderer;

type Message = &'static str;

const WINDOW: Size = Size {
    width: 400.0,
    height: 400.0,
};

fn headless_renderer() -> Renderer {
    aura_futures::futures::executor::block_on(
        <Renderer as aura::advanced::renderer::Headless>::new(
            aura::advanced::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("tiny-skia renderer must be available")
}

// ---------------------------------------------------------------------
// The trait contract
// ---------------------------------------------------------------------

/// A minimal overlay that reports a node, for testing the plumbing
/// independently of any real widget.
struct Reporting {
    id: Id,
    reports: bool,
}

impl overlay::Overlay<Message, Theme, Renderer> for Reporting {
    fn layout(&mut self, _renderer: &Renderer, bounds: Size) -> layout::Node {
        layout::Node::new(Size::new(bounds.width / 2.0, bounds.height / 2.0))
    }

    fn semantics(&self, layout: Layout<'_>) -> Option<Node> {
        self.reports
            .then(|| Node::new(self.id.clone(), layout.bounds(), Role::Group))
    }

    fn draw(
        &self,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &aura::advanced::renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
    ) {
    }
}

#[test]
fn an_overlay_that_reports_a_node_is_seen_through_nested() {
    let id = Id::unique();
    let element = overlay::Element::new(Box::new(Reporting {
        id: id.clone(),
        reports: true,
    }));

    let mut nested = overlay::Nested::new(element);
    let renderer = headless_renderer();

    let node = nested.layout(&renderer, WINDOW);
    let found = nested.semantics(Layout::new(&node), &renderer);

    assert_eq!(found.len(), 1, "the overlay's node must reach the runtime");
    assert_eq!(found[0].id, id);
    assert_eq!(found[0].role, Role::Group);
}

#[test]
fn an_overlay_with_no_semantics_of_its_own_reports_nothing() {
    // The default. A purely presentational overlay must not invent a node.
    let element = overlay::Element::new(Box::new(Reporting {
        id: Id::unique(),
        reports: false,
    }));

    let mut nested = overlay::Nested::new(element);
    let renderer = headless_renderer();

    let node = nested.layout(&renderer, WINDOW);

    assert!(nested.semantics(Layout::new(&node), &renderer).is_empty());
}

#[test]
fn an_overlays_node_carries_its_real_bounds() {
    // A screen reader points at where the overlay actually is, which is
    // not where its parent widget is -- that is the whole reason an
    // overlay exists.
    let element = overlay::Element::new(Box::new(Reporting {
        id: Id::unique(),
        reports: true,
    }));

    let mut nested = overlay::Nested::new(element);
    let renderer = headless_renderer();

    let node = nested.layout(&renderer, WINDOW);
    let found = nested.semantics(Layout::new(&node), &renderer);

    assert_eq!(
        found[0].bounds,
        Rectangle::new(Point::ORIGIN, Size::new(200.0, 200.0))
    );
}

// ---------------------------------------------------------------------
// The real payoff: pick_list's dropdown
// ---------------------------------------------------------------------

/// Builds a `pick_list`, lays it out, and clicks it open -- the real path
/// a user takes. `pick_list::State` is private, so there is no shortcut,
/// which is just as well: forcing the flag would not prove the dropdown
/// actually opens.
struct OpenPickList<'a> {
    widget: pick_list::PickList<'a, String, Vec<String>, String, Message, Theme, Renderer>,
    tree: widget::Tree,
    node: layout::Node,
    renderer: Renderer,
}

impl OpenPickList<'_> {
    fn new() -> Self {
        let options: Vec<String> = vec!["Alpha".into(), "Beta".into(), "Gamma".into()];

        let mut widget_ = pick_list(None::<String>, options, |option: &String| option.clone())
            .on_select(|_| "picked");

        let renderer = headless_renderer();
        let mut tree = widget::Tree::new(&widget_ as &dyn Widget<Message, Theme, Renderer>);
        tree.diff(&mut widget_ as &mut dyn Widget<Message, Theme, Renderer>);

        let node = Widget::<Message, Theme, Renderer>::layout(
            &mut widget_,
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, WINDOW),
        );

        Self {
            widget: widget_,
            tree,
            node,
            renderer,
        }
    }

    /// Clicks the control, which is what opens the dropdown.
    fn click(&mut self) {
        let mut messages = Vec::new();
        let mut shell = aura::advanced::Shell::new(
            &aura_core::window::Headless,
            aura_core::shell::Waker::noop(),
            &mut messages,
        );

        Widget::<Message, Theme, Renderer>::update(
            &mut self.widget,
            &mut self.tree,
            &aura::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Layout::new(&self.node),
            mouse::Cursor::Available(Layout::new(&self.node).bounds().center()),
            &self.renderer,
            &mut shell,
            &Rectangle::new(Point::ORIGIN, WINDOW),
        );
    }

    /// The semantics the dropdown overlay reports, if it is open.
    fn overlay_semantics(&mut self) -> Vec<Node> {
        let viewport = Rectangle::new(Point::ORIGIN, WINDOW);

        let Some(element) = Widget::<Message, Theme, Renderer>::overlay(
            &mut self.widget,
            &mut self.tree,
            Layout::new(&self.node),
            &self.renderer,
            &viewport,
            Vector::ZERO,
        ) else {
            return Vec::new();
        };

        let mut nested = overlay::Nested::new(element);
        let overlay_node = nested.layout(&self.renderer, WINDOW);

        nested.semantics(Layout::new(&overlay_node), &self.renderer)
    }
}

#[test]
fn an_open_pick_list_dropdown_is_no_longer_invisible() {
    // The regression this whole change exists to prevent. An open dropdown
    // is the only thing the user can interact with; if it reports nothing,
    // the choices are unreachable.
    let mut list = OpenPickList::new();
    list.click();

    let found = list.overlay_semantics();

    assert!(
        !found.is_empty(),
        "an open dropdown must report semantics -- it reported none before this hook existed"
    );
    assert!(
        found[0].state.contains(State::EXPANDED),
        "the dropdown must announce that it is open"
    );
    assert!(
        found[0].bounds.width > 0.0 && found[0].bounds.height > 0.0,
        "the dropdown must carry real bounds a screen reader can point at"
    );
}

#[test]
fn a_closed_pick_list_has_no_overlay_to_announce() {
    // The converse: a closed dropdown has nothing to say, and must not
    // leave a phantom node behind.
    let mut list = OpenPickList::new();

    assert!(
        list.overlay_semantics().is_empty(),
        "a closed pick_list must announce nothing"
    );
}

#[test]
fn an_open_dropdowns_identity_is_stable_across_frames() {
    // A node whose id changed every frame would look new to assistive
    // technology on every repaint. The overlay derives its identity from
    // the widget's own `Tree`, which survives reconciliation.
    let mut list = OpenPickList::new();
    list.click();

    let first = list.overlay_semantics();
    let second = list.overlay_semantics();

    assert!(!first.is_empty() && !second.is_empty());
    assert_eq!(
        first[0].id, second[0].id,
        "an overlay's identity must not be reminted every frame"
    );
}
