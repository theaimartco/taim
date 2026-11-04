//! Default-method compatibility and public API checks for the contracts
//! introduced in `phase.md` Phase 2.
//!
//! These tests exist to catch the specific regression Phase 2's design
//! constraints exist to prevent: a widget that does not override any of
//! the four new `Widget` methods (`paint_bounds`, `hit_test`,
//! `semantics`, `cache_policy`) must keep compiling and behaving exactly
//! as it did before those methods existed ("all existing widgets compile
//! unchanged" and "no mandatory `Widget` method is introduced").

type Element<'a, M, T = taim_core::Theme, R = taim_renderer::Renderer> =
    taim_core::Element<'a, M, T, R>;
use taim_core::widget::{self, Widget};
use taim_core::{Layout, mouse, renderer};
use taim_core::{Length, Point, Rectangle, Size, Theme};
use taim_renderer::Renderer;

/// A widget that overrides nothing beyond the mandatory methods, to
/// prove the four new defaulted methods are reachable and behave as
/// documented without any widget author having to opt in.
struct Bare;

impl<Message> Widget<Message, Theme, Renderer> for Bare {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(40.0), Length::Fixed(20.0))
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        _limits: &taim_core::layout::Limits,
    ) -> taim_core::layout::Node {
        taim_core::layout::Node::new(Size::new(40.0, 20.0))
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

fn layout_at(bounds: Rectangle) -> taim_core::layout::Node {
    taim_core::layout::Node::new(bounds.size()).move_to(bounds.position())
}

#[test]
fn a_bare_widget_compiles_without_overriding_any_phase_2_method() {
    // The real assertion is that this file compiles at all: `Bare` only
    // implements `size`, `layout`, and `draw` (the pre-Phase-2 minimum),
    // yet satisfies the full `Widget` trait.
    fn assert_is_a_widget<T: Widget<(), Theme, Renderer>>() {}
    assert_is_a_widget::<Bare>();
}

#[test]
fn default_paint_bounds_is_the_layout_bounds() {
    let widget = Bare;
    let tree = widget::Tree::empty();

    let bounds = Rectangle::new(Point::new(10.0, 20.0), Size::new(40.0, 20.0));
    let node = layout_at(bounds);
    let layout = Layout::new(&node);

    let widget_ref: &dyn Widget<(), Theme, Renderer> = &widget;
    assert_eq!(widget_ref.paint_bounds(&tree, layout), bounds);
}

#[test]
fn default_hit_test_matches_a_rectangular_bounds_check() {
    let widget = Bare;
    let tree = widget::Tree::empty();

    let bounds = Rectangle::new(Point::new(10.0, 20.0), Size::new(40.0, 20.0));
    let node = layout_at(bounds);
    let layout = Layout::new(&node);

    let widget_ref: &dyn Widget<(), Theme, Renderer> = &widget;

    assert!(widget_ref.hit_test(&tree, layout, Point::new(15.0, 25.0)));
    assert!(!widget_ref.hit_test(&tree, layout, Point::new(0.0, 0.0)));

    // `Rectangle::contains` excludes the far edge; the default `hit_test`
    // must agree exactly, since it is documented to defer to it.
    let far_edge = Point::new(bounds.x + bounds.width, bounds.y + bounds.height);
    assert_eq!(
        bounds.contains(far_edge),
        widget_ref.hit_test(&tree, layout, far_edge)
    );
}

#[test]
fn default_semantics_is_none() {
    let widget = Bare;
    let tree = widget::Tree::empty();

    let node = layout_at(Rectangle::new(Point::ORIGIN, Size::new(40.0, 20.0)));
    let layout = Layout::new(&node);

    let widget_ref: &dyn Widget<(), Theme, Renderer> = &widget;
    assert!(widget_ref.semantics(&tree, layout).is_none());
}

#[test]
fn default_cache_policy_is_none() {
    let widget = Bare;

    let widget_ref: &dyn Widget<(), Theme, Renderer> = &widget;
    assert!(widget_ref.cache_policy().is_none());
}

#[test]
fn default_is_void_is_false_and_void_overrides_it_to_true() {
    let widget = Bare;
    let widget_ref: &dyn Widget<(), Theme, Renderer> = &widget;
    assert!(!widget_ref.is_void());

    let void = widget::Void;
    let void_ref: &dyn Widget<(), Theme, Renderer> = &void;
    assert!(void_ref.is_void());
}

#[test]
fn element_construction_from_a_bare_widget_still_works() {
    // A minimal end-to-end check that the public `Element` API is
    // unaffected: constructing an `Element` from a widget that only
    // implements the pre-Phase-2 minimum must still work.
    let element: Element<'static, ()> = Element::new(Bare);
    drop(element);
}

/// Public API surface checks: every type `phase.md` names for Phase 2
/// must be reachable at the documented path with the documented shape.
/// These assertions are intentionally shallow (constructing a value and
/// checking a basic invariant); a full behavioral test for each type
/// lives alongside its definition in `core/src/`.
mod public_surface {
    use taim_core::widget::Id;
    use taim_core::widget::tree::{DirtyFlags, Revision};
    use taim_core::{cache, damage, frame, semantics};

    #[test]
    fn widget_id_keyed_is_part_of_the_public_api() {
        let a = Id::keyed(1_u64);
        let b = Id::keyed(1_u64);

        assert_eq!(a, b);
    }

    #[test]
    fn tree_revision_and_dirty_flags_are_part_of_the_public_api() {
        assert_eq!(Revision::INITIAL, Revision::INITIAL);
        assert_eq!(DirtyFlags::all().bits(), DirtyFlags::all().bits());
    }

    #[test]
    fn core_damage_is_part_of_the_public_api() {
        let request = damage::Request::full(damage::Cause::Content);
        assert_eq!(request.region, damage::Region::Full);
    }

    #[test]
    fn core_cache_is_part_of_the_public_api() {
        let policy = cache::Policy::new(cache::Budget::Entries(10), cache::Class::Retained);
        assert_eq!(policy.class, cache::Class::Retained);
    }

    #[test]
    fn core_frame_is_part_of_the_public_api() {
        assert_eq!(frame::Id::FIRST.next(), frame::Id::FIRST.next());
        assert!(frame::Quality::Recovery < frame::Quality::Ultra);
    }

    #[test]
    fn core_semantics_is_part_of_the_public_api() {
        let node = semantics::Node::new(
            Id::unique(),
            taim::Rectangle::new(taim::Point::ORIGIN, taim::Size::new(1.0, 1.0)),
            semantics::Role::Button,
        );

        assert_eq!(node.role, semantics::Role::Button);
    }
}
