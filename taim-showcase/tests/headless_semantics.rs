//! Pillar X's gate, run for real: **drive the full `taim-showcase` end to
//! end with the screen off, through the semantic tree alone, with no
//! visual renderer instantiated.**
//!
//! The renderer instantiated below is `()` -- `taim_core::renderer`'s
//! null renderer, built in automatically under `debug_assertions` (see
//! `core/src/renderer/null.rs`). It is not a headless CPU rasteriser
//! standing in for a screen the way `renderer::Headless` for tiny-skia is
//! used elsewhere in this workspace's own test suite
//! (`tests/golden/harness.rs`, `tests/accessibility/user_interface.rs`):
//! `()` never produces a pixel, under any circumstance -- its
//! `image::Renderer`/`svg::Renderer`/`core::Renderer` impls are all
//! no-ops, and its `text::Renderer` impl reports trivial zero-size
//! metrics rather than shaping anything. `taim_showcase::Renderer` (see
//! `src/lib.rs`) exists precisely so `Showcase`'s view methods can be
//! instantiated against it instead of the real backend `main.rs` uses.
//!
//! No `winit` window, no wgpu device or surface, no tiny-skia canvas, no
//! X11/Wayland connection is created anywhere in this file or anything
//! it calls.
//!
//! The driver is deliberately ignorant of `Showcase`'s internals: it
//! never reads a field and never names a `Message` variant to decide what
//! to do. It walks `Tree::in_order()` for role, name, actions and bounds
//! -- exactly what an AT-SPI client such as Orca, or an agent reading the
//! tree as a tool schema (future.md, Pillar X's "agent presentation"),
//! would see -- picks a target by that alone, and synthesizes the
//! pointer event at the bounds the tree itself reported.
//!
//! The second test below is not decoration: driving the gate for real
//! surfaced a genuine defect in the framework's incremental semantic
//! engine, undocumented before this file. See its doc comment.

use taim::Theme;
use taim_accessibility::tree::Tree;
use taim_core::semantics::{Actions, Role, State};
use taim_core::{Event, Point, Rectangle, Size, mouse, shell, window};
use taim_runtime::user_interface::{Cache, UserInterface};
use taim_showcase::{Message, Showcase};

/// The null renderer: no wgpu, no tiny-skia, nothing that draws.
type Renderer = ();

const WINDOW: Size = Size {
    width: 1024.0,
    height: 1400.0,
};

/// One pass: build the `UserInterface` from the current view, refresh its
/// retained semantic tree, and hand back both -- exactly the two things
/// an accessibility bridge and an application frame loop each need, and
/// nothing a renderer would need. This is `winit/src/lib.rs`'s own
/// `UserInterface::build` / `semantics_changes` pair (see its line 1345
/// and 946), reached the same way, with the renderer swapped for `()`.
fn frame<'a>(
    showcase: &'a Showcase,
    cache: Cache,
    renderer: &mut Renderer,
) -> (UserInterface<'a, Message, Theme, Renderer>, Tree) {
    let mut ui = UserInterface::build(showcase.view::<Renderer>(), WINDOW, cache, renderer);
    let _ = ui.semantics_changes(renderer);
    let tree = ui.semantic_tree().clone();

    (ui, tree)
}

/// Synthesizes a pointer click at `point` -- a mouse move, a press, and a
/// release, the same three events a real pointer produces -- and returns
/// whatever messages the widgets under it produced.
fn click(
    ui: &mut UserInterface<'_, Message, Theme, Renderer>,
    renderer: &mut Renderer,
    point: Point,
) -> Vec<Message> {
    let events = [
        Event::Mouse(mouse::Event::CursorMoved { position: point }),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    ];

    let mut messages = Vec::new();

    let _ = ui.update(
        &window::Headless,
        &shell::Waker::noop(),
        &events,
        mouse::Cursor::Available(point),
        renderer,
        &mut messages,
    );

    messages
}

/// The center of an entry's reported bounds -- the point a coordinate-
/// based assistive input (touch-through, a switch device, an agent
/// clicking through a tool call) would target, using nothing but what
/// the tree already told it.
fn center(bounds: Rectangle) -> Point {
    Point::new(
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    )
}

/// Pillar X's gate, as one test: the showcase's full widget tree is
/// reachable, walkable, and actionable through nothing but its semantic
/// tree, with `()` -- no visual renderer -- doing the layout
/// text-measurement `UserInterface::build` needs.
#[test]
fn taim_showcase_is_driven_end_to_end_through_the_semantic_tree_with_no_visual_renderer() {
    let (mut showcase, _) = Showcase::new();
    let mut renderer: Renderer = ();
    let mut cache = Cache::new();

    // ── Read the tree cold, the way a screen reader attaching mid-session
    // does. ─────────────────────────────────────────────────────────────
    let (ui, tree) = frame(&showcase, cache, &mut renderer);
    cache = ui.into_cache();

    assert!(
        tree.len() > 20,
        "the showcase's opening screen (Controls) must publish a real \
         tree, not a handful of placeholder nodes: got {} entries",
        tree.len()
    );

    let roles: Vec<Role> = tree.in_order().iter().map(|entry| entry.role).collect();

    for role in [Role::Button, Role::Checkbox, Role::Slider, Role::TextField] {
        assert!(
            roles.contains(&role),
            "the Controls section must expose a {role:?} in the tree; saw {roles:?}"
        );
    }

    // ── Find the "Default" button by role and name alone, and press it
    // purely by its reported bounds -- no widget code, no coordinates
    // chosen by looking at a screen, nothing but what the tree said. ────
    let default_button = tree
        .in_order()
        .into_iter()
        .find(|entry| entry.role == Role::Button && entry.name.as_deref() == Some("Default"))
        .expect("the showcase must publish a button named 'Default'")
        .clone();

    assert!(
        default_button.actions.contains(Actions::ACTIVATE),
        "a button that does not offer ACTIVATE could never be pressed by \
         a screen reader or an agent, only by a sighted mouse user"
    );

    let (mut ui, _) = frame(&showcase, cache, &mut renderer);
    let messages = click(&mut ui, &mut renderer, center(default_button.bounds));
    cache = ui.into_cache();

    assert_eq!(
        messages,
        vec![Message::NoticeShown],
        "clicking exactly the point the tree reported for the button \
         must produce the button's message, and nothing else -- the \
         click reached the real widget, not empty space"
    );

    for message in messages {
        let _ = showcase.update(message);
    }

    // ── Do it again against a completely different control -- a
    // checkbox rather than a button, ACTIVATE toggling state rather than
    // firing a one-shot message -- to show the first click was not a
    // fluke of one widget's implementation. ─────────────────────────────
    let (ui, tree) = frame(&showcase, cache, &mut renderer);
    cache = ui.into_cache();

    let checkbox = tree
        .in_order()
        .into_iter()
        .find(|entry| entry.role == Role::Checkbox && entry.name.as_deref() == Some("Checkbox"))
        .expect("the showcase must publish a checkbox named 'Checkbox'")
        .clone();

    assert!(
        !checkbox.state.contains(State::CHECKED),
        "the showcase starts with its checkbox unchecked"
    );

    let (mut ui, _) = frame(&showcase, cache, &mut renderer);
    let messages = click(&mut ui, &mut renderer, center(checkbox.bounds));
    let _ = ui.into_cache();

    assert_eq!(
        messages,
        vec![Message::Checked(true)],
        "clicking the checkbox at the point its own tree entry reported \
         must produce its toggle message"
    );

    // Deliberately not verified here: that the checkbox's `State::CHECKED`
    // bit flips in the *next* tree once `Showcase::update` applies this
    // message. It does not -- see
    // `a_widgets_semantic_state_can_go_stale_after_a_value_only_prop_change`
    // below, which pins down why, and is the actual finding this file
    // exists to report.
}

/// A gap this file's own gate exercise found, not one it set out to
/// look for: **a widget whose semantic state is driven only by a prop
/// that changes between `view()` calls -- the ordinary Elm-style pattern
/// this entire framework, and this very showcase, is written in -- does
/// not reliably get a fresh semantic node.**
///
/// `runtime/src/user_interface.rs::build` calls `state.diff(root)` then
/// unconditionally marks the tree `DirtyFlags::PAINT` -- but never
/// `DirtyFlags::SEMANTICS` (`runtime/src/user_interface.rs:127-129`).
/// `taim_runtime::semantics::build`'s incremental engine (its own doc
/// comment, `runtime/src/semantics.rs:10`) treats a clean `SEMANTICS`
/// flag as "reuse the cached node" -- and nothing clears it again after
/// the first frame for an ordinary widget. Grepping the whole `widget`
/// crate for `mark_dirty` turns up exactly four call sites --
/// `virtual_list.rs`, `lazy.rs`, `navigator.rs`, `reactive.rs` -- and
/// `Checkbox`, `Toggler`, `Slider`, and every other value-only widget is
/// not one of them; `Widget::diff`'s default
/// (`core/src/widget.rs:85-87`) does not touch dirty flags at all.
/// `tests/widgets/dirty_flags.rs`'s own doc comment already named this:
/// *"a pattern Phase 3's real reconciliation is expected to follow once
/// it starts calling `Tree::mark_dirty` from widget code. Phase 2
/// introduces no such caller itself."* That caller still does not exist.
///
/// This is not specific to the null renderer, to this test's harness, or
/// to `Checkbox`: `winit/src/lib.rs:1345` and `:946` call the exact same
/// `UserInterface::build` / `semantics_changes` pair this file calls, so
/// a real, on-screen Taim application driven by mouse and keyboard has
/// the same stale tree a screen reader would read from. It is not
/// currently exercised anywhere in this workspace's test suite, because
/// nothing before this file drove a click all the way from the semantic
/// tree back through to reading the semantic tree again.
///
/// This test pins down *today's* (2026-09-06) actual, incorrect
/// behavior -- it asserts the bug, not the fix -- so a future fix to
/// widget diffing turns it red instead of silently leaving the defect in
/// place. `Message::Checked(true)` is genuinely produced (proved above);
/// what does not happen is the tree updating to say so.
#[test]
fn a_widgets_semantic_state_can_go_stale_after_a_value_only_prop_change() {
    let (mut showcase, _) = Showcase::new();
    let mut renderer: Renderer = ();
    let mut cache = Cache::new();

    let (ui, tree) = frame(&showcase, cache, &mut renderer);
    cache = ui.into_cache();

    let checkbox = tree
        .in_order()
        .into_iter()
        .find(|entry| entry.role == Role::Checkbox && entry.name.as_deref() == Some("Checkbox"))
        .expect("the showcase must publish a checkbox named 'Checkbox'")
        .clone();

    let (mut ui, _) = frame(&showcase, cache, &mut renderer);
    let messages = click(&mut ui, &mut renderer, center(checkbox.bounds));
    cache = ui.into_cache();

    assert_eq!(messages, vec![Message::Checked(true)]);

    for message in messages {
        let _ = showcase.update(message);
    }

    let (ui, tree) = frame(&showcase, cache, &mut renderer);
    let _ = ui.into_cache();

    let checkbox_after = tree
        .in_order()
        .into_iter()
        .find(|entry| entry.role == Role::Checkbox && entry.name.as_deref() == Some("Checkbox"))
        .expect("the checkbox must still be published");

    assert!(
        !checkbox_after.state.contains(State::CHECKED),
        "this assertion documents the bug: `Showcase::checked` is now \
         `true` and a fresh oracle rebuild would report CHECKED, but the \
         incremental engine's cached node is not invalidated, so the \
         tree a screen reader or an agent would read here is stale. If \
         this assertion starts failing, `DirtyFlags::SEMANTICS` \
         invalidation on plain prop changes has been fixed -- flip this \
         test (and Pillar X's gate status in future.md) accordingly."
    );
}
