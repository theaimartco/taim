//! `UserInterface::semantics` — the accessibility engine, reached the way
//! an application reaches it.
//!
//! Everything else in this suite drives the engine through a harness that
//! adapts widgets to `Semantic` by hand. That proved each stage works; it
//! did not prove any of them were *connected*, and until
//! `UserInterface::semantics` existed, they were not: every call site of
//! `runtime::semantics::build` in the workspace was a test, and
//! `UserInterface` had no semantics method at all. A screen reader
//! attached to a real Taim application would have found nothing.
//!
//! These tests call the same public method an application calls, on a
//! real `UserInterface` built from real widgets.

use taim::widget::{button, checkbox, column, container, float, pick_list, pin, row, space, text};
use taim::{Element, Point, Size, Theme};
use taim_core::mouse;
use taim_core::semantics::{Node, Role, State};
use taim_renderer::Renderer;
use taim_runtime::user_interface::{Cache, UserInterface};

type Message = &'static str;

const WINDOW: Size = Size {
    width: 400.0,
    height: 400.0,
};

fn headless_renderer() -> Renderer {
    taim_futures::futures::executor::block_on(
        <Renderer as taim::advanced::renderer::Headless>::new(
            taim::advanced::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("tiny-skia renderer must be available")
}

/// Builds a real `UserInterface` from `view` and asks it for its
/// semantics — exactly what an application's accessibility adapter does
/// each frame.
fn semantics_of(view: Element<'_, Message, Theme, Renderer>) -> Vec<Node> {
    let mut renderer = headless_renderer();
    let mut ui = UserInterface::build(view, WINDOW, Cache::new(), &mut renderer);

    ui.semantics(&renderer)
}

/// Every name in `forest`, depth-first.
fn names(forest: &[Node]) -> Vec<&str> {
    fn recurse<'a>(nodes: &'a [Node], out: &mut Vec<&'a str>) {
        for node in nodes {
            if let Some(name) = &node.name {
                out.push(name.as_str());
            }

            recurse(&node.children, out);
        }
    }

    let mut out = Vec::new();
    recurse(forest, &mut out);
    out
}

/// Every role in `forest`, depth-first — the order a screen reader reads.
fn roles(forest: &[Node]) -> Vec<Role> {
    fn recurse(nodes: &[Node], out: &mut Vec<Role>) {
        for node in nodes {
            out.push(node.role);
            recurse(&node.children, out);
        }
    }

    let mut out = Vec::new();
    recurse(forest, &mut out);
    out
}

#[test]
fn a_real_application_can_finally_see_its_own_widgets() {
    // The regression this whole change exists to prevent. Before
    // `UserInterface::semantics`, this returned nothing -- there was no
    // method to call.
    let forest = semantics_of(
        column![
            button("Save").on_press("save"),
            checkbox(true).label("Agree").on_toggle(|_| "agree"),
        ]
        .into(),
    );

    assert_eq!(
        roles(&forest),
        // The `StaticText` is the button's own label, reported as its
        // child -- the shape AT-SPI expects. The button carries the same
        // text as its accessible name; see
        // `a_button_takes_its_accessible_name_from_its_label`.
        vec![Role::Button, Role::StaticText, Role::Checkbox],
        "a screen reader must see both controls, in document order"
    );
}

#[test]
fn a_transparent_container_hoists_its_children_rather_than_hiding_them() {
    // `column`, `container` and `row` report nothing of their own. If
    // they did not hoist, everything inside an ordinary layout would be
    // invisible -- which is to say, everything.
    let forest = semantics_of(
        container(column![
            row![button("One").on_press("one")],
            container(checkbox(false).label("Two").on_toggle(|_| "two")),
        ])
        .into(),
    );

    assert_eq!(
        roles(&forest),
        vec![Role::Button, Role::StaticText, Role::Checkbox],
        "controls nested under four transparent containers must still surface"
    );
}

#[test]
fn a_tree_transparent_wrapper_does_not_poison_its_contents_semantics() {
    // `container`, `pin` and `float` are transparent in the *widget tree*
    // as well as the semantic one: their `tag`/`state`/`diff` defer to
    // their content, so wrapper and content share a single `Tree` -- and
    // therefore a single `semantics_with_cache` slot.
    //
    // A wrapper that reported its own (`None`) semantics would cache that
    // `None` into the shared slot and clear the dirty flag, so the
    // content's own lookup would hit the poisoned cache and return
    // nothing. `container(button(..))` -- as ordinary as a view gets --
    // would silently announce nothing at all.
    for (name, forest) in [
        (
            "container",
            semantics_of(container(button("Save").on_press("save")).into()),
        ),
        (
            "pin",
            semantics_of(pin(button("Save").on_press("save")).into()),
        ),
        (
            "float",
            semantics_of(float(button("Save").on_press("save")).into()),
        ),
    ] {
        assert_eq!(
            roles(&forest),
            vec![Role::Button, Role::StaticText],
            "{name} shares a Tree with its content; it must delegate its semantics \
             rather than cache a None over them"
        );
    }
}

#[test]
fn prose_is_not_invisible() {
    // `text` reported nothing until recently, which meant every label and
    // paragraph in an application was invisible: a screen reader heard
    // the controls and none of the writing between them.
    let forest = semantics_of(column![text("just a label"), text("and another")].into());

    assert_eq!(
        roles(&forest),
        vec![Role::StaticText, Role::StaticText],
        "prose must reach a screen reader"
    );
    assert_eq!(names(&forest), vec!["just a label", "and another"]);
}

#[test]
fn a_view_with_nothing_to_say_reports_an_empty_forest() {
    // Transparency must not invent nodes either. Empty text has nothing
    // to announce, so it reports nothing -- announcing "" is noise, and
    // `text("")` is a common spacer.
    let forest = semantics_of(column![text(""), space()].into());

    assert!(
        forest.is_empty(),
        "nothing here has anything to announce -- got {forest:?}"
    );
}

#[test]
fn a_button_takes_its_accessible_name_from_its_label() {
    // `button("Save")` announced as a bare, unlabeled "button" until the
    // name-from-content rule existed. Unlike `checkbox`/`radio`, a button
    // has no `label` field of its own -- its label is a child widget, and
    // `Widget::semantics` takes `&self`, so a button cannot reach its own
    // subtree. The name has to be computed during assembly.
    let forest = semantics_of(column![button("Save").on_press("save")].into());

    assert_eq!(forest[0].role, Role::Button);
    assert_eq!(
        forest[0].name.as_deref(),
        Some("Save"),
        "a button must announce its label, not just \"button\""
    );
}

#[test]
fn a_buttons_name_is_gathered_from_a_composite_label() {
    // Why the rule walks descendants rather than reading one child: a
    // real button's label is often a row of an icon and some text.
    let forest =
        semantics_of(column![button(row![text("Save"), text("all")]).on_press("save")].into());

    assert_eq!(forest[0].name.as_deref(), Some("Save all"));
}

#[test]
fn a_control_that_names_itself_is_not_renamed_by_its_content() {
    // `checkbox` owns its label and reports it directly. The rule is a
    // fallback, and must never override an explicit name.
    let forest = semantics_of(column![checkbox(true).label("Agree").on_toggle(|_| "t")].into());

    assert_eq!(forest[0].name.as_deref(), Some("Agree"));
}

#[test]
fn an_unlabeled_button_stays_unlabeled() {
    // A name of `""` would read as "named, with an empty name" -- worse
    // than an honest absence, which an adapter can at least fall back on.
    let forest = semantics_of(column![button(text("")).on_press("x")].into());

    assert_eq!(forest[0].role, Role::Button);
    assert_eq!(forest[0].name, None);
}

#[test]
fn a_disabled_control_still_appears_but_advertises_no_actions() {
    // Authoring rule 2, verified through the real pipeline rather than
    // against the widget in isolation.
    let forest = semantics_of(column![button("Save")].into());

    let node = forest.first().expect("the button must still be reported");

    assert_eq!(node.role, Role::Button);
    assert!(node.state.contains(State::DISABLED));
    assert!(
        node.actions.is_empty(),
        "a disabled button must offer assistive technology nothing to invoke"
    );
}

#[test]
fn identity_is_stable_across_frames() {
    // A node whose id changed per frame would look new to assistive
    // technology on every repaint, re-announcing itself endlessly.
    let mut renderer = headless_renderer();
    let mut cache = Cache::new();
    let mut ids = Vec::new();

    for _ in 0..2 {
        let view: Element<'_, Message, Theme, Renderer> =
            column![button("Save").on_press("save")].into();

        let mut ui = UserInterface::build(view, WINDOW, cache, &mut renderer);
        ids.push(ui.semantics(&renderer)[0].id.clone());
        cache = ui.into_cache();
    }

    assert_eq!(
        ids[0], ids[1],
        "an id must survive view reconstruction, as widget state does"
    );
}

#[test]
fn live_semantic_diff_only_publishes_real_changes() {
    let mut renderer = headless_renderer();
    let view: Element<'_, Message, Theme, Renderer> =
        column![button("Save").on_press("save")].into();
    let mut ui = UserInterface::build(view, WINDOW, Cache::new(), &mut renderer);

    let initial = ui.semantics_changes(&renderer);
    assert!(
        !initial.is_empty(),
        "the first live semantic frame must publish the initial tree"
    );
    assert!(
        ui.semantics_changes(&renderer).is_empty(),
        "an unchanged frame must not re-announce semantic nodes"
    );
}

#[test]
fn a_clean_tree_reuses_its_cached_nodes_across_frames() {
    // `semantics` is called per frame, so it must cost a walk rather than
    // a rebuild. Equal output across two builds of an untouched tree is
    // the observable half of that; the call-counting proof lives in
    // `tests/semantics/incremental.rs`.
    let mut renderer = headless_renderer();
    let mut cache = Cache::new();
    let mut frames = Vec::new();

    for _ in 0..2 {
        let view: Element<'_, Message, Theme, Renderer> = column![
            button("Save").on_press("save"),
            checkbox(true).label("Agree")
        ]
        .into();

        let mut ui = UserInterface::build(view, WINDOW, cache, &mut renderer);
        frames.push(ui.semantics(&renderer));
        cache = ui.into_cache();
    }

    assert_eq!(
        frames[0], frames[1],
        "an untouched tree must report identically frame to frame"
    );
}

#[test]
fn an_open_dropdown_reaches_the_application_through_its_overlay() {
    // The end of the chain the overlay work started: `pick_list`'s
    // dropdown is not in the widget tree at all, so it can only arrive
    // via `Nested::semantics`. This is the test that proves
    // `UserInterface` actually calls it.
    let mut renderer = headless_renderer();

    let options: Vec<String> = vec!["Alpha".into(), "Beta".into()];
    let view: Element<'_, Message, Theme, Renderer> =
        pick_list(None::<String>, options, |option: &String| option.clone())
            .on_select(|_| "picked")
            .into();

    let mut ui = UserInterface::build(view, WINDOW, Cache::new(), &mut renderer);

    let closed = ui.semantics(&renderer);
    assert!(
        !closed
            .iter()
            .any(|node| node.state.contains(State::EXPANDED)),
        "a closed pick_list must not claim to be expanded"
    );

    // Click it open, the way a user does.
    let mut messages = Vec::new();
    let _ = ui.update(
        &taim_core::window::Headless,
        &taim_core::shell::Waker::noop(),
        &[taim::Event::Mouse(mouse::Event::ButtonPressed(
            mouse::Button::Left,
        ))],
        mouse::Cursor::Available(Point::new(20.0, 10.0)),
        &mut renderer,
        &mut messages,
    );

    let open = ui.semantics(&renderer);

    assert!(
        open.iter().any(|node| node.state.contains(State::EXPANDED)),
        "an open dropdown must reach the application -- it lives outside the widget tree, \
         so only the overlay path can carry it: {open:?}"
    );
}
