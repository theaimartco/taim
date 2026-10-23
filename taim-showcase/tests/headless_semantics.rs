//! Drives the whole showcase with no window: the view tree runs on its own
//! [`Root`] with the CPU (tiny-skia) renderer created headless, and the test
//! finds controls only through the semantic tree — role, name, actions,
//! bounds — the way a screen reader or an agent would, then clicks at the
//! bounds the tree reported.
use taim::view::Root;
use taim_accessibility::tree::Tree;
use taim_core::renderer::Headless as _;
use taim_core::semantics::{Actions, Role, State};
use taim_core::time::Instant;
use taim_core::{Event, Point, Rectangle, Size, mouse};

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

/// Lays the tree out and returns its semantics.
fn frame(root: &mut Root, renderer: &Renderer) -> Tree {
    root.layout(renderer, WINDOW);
    Tree::from_forest(&[root.describe(WINDOW)])
}

/// A move, press and release at `point`.
fn click(root: &mut Root, point: Point) {
    let events = [
        Event::Mouse(mouse::Event::CursorMoved { position: point }),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    ];
    for event in &events {
        let _ = root.handle(event, Some(point), Instant::now());
    }
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
    let mut root = Root::new(taim_showcase::build);
    let renderer = renderer();

    let tree = frame(&mut root, &renderer);

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

    click(&mut root, center(checkbox.bounds));

    // The next tree reports the new state: no message, no update function.
    let tree = frame(&mut root, &renderer);
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
