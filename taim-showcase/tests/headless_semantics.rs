//! Drives the whole showcase with no window: the view tree runs inside the
//! runtime's `UserInterface` with the CPU (tiny-skia) renderer created
//! headless, and the test finds controls only through the semantic tree —
//! role, name, actions, bounds — the way a screen reader or an agent would,
//! then clicks at the bounds the tree reported.
use std::cell::RefCell;

use taim::Theme;
use taim::view::Host;
use taim::view::host::Root;
use taim_accessibility::tree::Tree;
use taim_core::renderer::Headless as _;
use taim_core::semantics::{Actions, Role, State};
use taim_core::{Element, Event, Point, Rectangle, Size, mouse, shell, window};
use taim_runtime::user_interface::{Cache, UserInterface};

type Renderer = taim::Renderer;

const WINDOW: Size = Size {
    width: 1024.0,
    height: 1400.0,
};

fn renderer() -> Renderer {
    futures::executor::block_on(Renderer::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("the CPU renderer starts without a GPU or a window")
}

/// Lays the tree out, refreshes its semantics, and returns both.
fn frame<'a>(
    root: &'a RefCell<Root>,
    cache: Cache,
    renderer: &mut Renderer,
) -> (UserInterface<'a, (), Theme, Renderer>, Tree) {
    let element: Element<'a, (), Theme, Renderer> = Host::new(root).into();
    let mut ui = UserInterface::build(element, WINDOW, cache, renderer);
    let _ = ui.semantics_changes(renderer);
    let tree = ui.semantic_tree().clone();
    (ui, tree)
}

/// A move, press and release at `point`.
fn click(ui: &mut UserInterface<'_, (), Theme, Renderer>, renderer: &mut Renderer, point: Point) {
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
}

fn center(bounds: Rectangle) -> Point {
    Point::new(
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    )
}

fn find(tree: &Tree, role: Role, name: &str) -> taim_accessibility::Entry {
    tree.in_order()
        .into_iter()
        .find(|entry| entry.role == role && entry.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("the showcase must publish a {role:?} named {name:?}"))
        .clone()
}

#[test]
fn the_showcase_is_driven_end_to_end_through_its_semantic_tree() {
    let root = RefCell::new(Root::new(taim_showcase::build));
    let mut renderer = renderer();

    let (ui, tree) = frame(&root, Cache::new(), &mut renderer);
    let mut cache = ui.into_cache();

    assert!(
        tree.len() > 20,
        "the Controls page publishes a real tree, got {} entries",
        tree.len()
    );

    let roles: Vec<Role> = tree.in_order().iter().map(|entry| entry.role).collect();
    for role in [Role::Button, Role::Checkbox, Role::Slider, Role::TextField] {
        assert!(
            roles.contains(&role),
            "the Controls page must expose a {role:?}; saw {roles:?}"
        );
    }

    // Press the checkbox purely by what the tree reported about it.
    let checkbox = find(&tree, Role::Checkbox, "Checkbox");
    assert!(checkbox.actions.contains(Actions::ACTIVATE));
    assert!(
        !checkbox.state.contains(State::CHECKED),
        "the showcase starts unchecked"
    );

    let (mut ui, _) = frame(&root, cache, &mut renderer);
    click(&mut ui, &mut renderer, center(checkbox.bounds));
    cache = ui.into_cache();

    // The next tree reports the new state: no message, no update function.
    let (ui, tree) = frame(&root, cache, &mut renderer);
    let _ = ui.into_cache();
    assert!(
        find(&tree, Role::Checkbox, "Checkbox")
            .state
            .contains(State::CHECKED),
        "clicking the checkbox where the tree said it is must check it"
    );

    // A disabled button is published as such, with nothing to activate.
    let disabled = find(&tree, Role::Button, "Disabled");
    assert!(disabled.state.contains(State::DISABLED));
    assert!(!disabled.actions.contains(Actions::ACTIVATE));
}
