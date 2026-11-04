//! Two fingers over a pinchable list, delivered as the Android shell does:
//! the first finger as pointer events and every finger as a raw touch.
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use taim::prelude::*;
use taim::view::Root;
use taim_accessibility::tree::Tree;
use taim_core::renderer::Headless as _;
use taim_core::semantics::Role;
use taim_core::time::Instant;
use taim_core::{Event, Point, Size, mouse, pointer, touch};

const WINDOW: Size = Size {
    width: 360.0,
    height: 640.0,
};

fn renderer() -> taim::Renderer {
    taim_futures::futures::executor::block_on(taim::Renderer::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("the CPU renderer starts headless")
}

struct Pinched {
    root: Root,
    pressed: Arc<AtomicUsize>,
    scale: Rc<Cell<f32>>,
    ended: Rc<Cell<Option<f32>>>,
}

fn pinched_list() -> Pinched {
    let pressed = Arc::new(AtomicUsize::new(0));
    let scale = Rc::new(Cell::new(1.0));
    let ended = Rc::new(Cell::new(None));

    let root = Root::new({
        let (pressed, scale, ended) = (pressed.clone(), scale.clone(), ended.clone());
        move |_| {
            let rows: Vec<AnyView> = (0..40)
                .map(|index| {
                    let pressed = pressed.clone();
                    button(format!("Row {index}"))
                        .on_press(move || {
                            let _ = pressed.fetch_add(1, Ordering::SeqCst);
                        })
                        .into()
                })
                .collect();

            pinch(scroll(column(rows).gap(8.0).fill_width()))
                .on_pinch(move |factor, _| scale.set(factor))
                .on_end(move |factor| ended.set(Some(factor)))
        }
    });

    Pinched {
        root,
        pressed,
        scale,
        ended,
    }
}

fn center_of(root: &mut Root, name: &str) -> Point {
    let tree = Tree::from_forest(&[root.describe(WINDOW)]);
    let entry = tree
        .in_order()
        .into_iter()
        .find(|entry| entry.role == Role::Button && entry.name.as_deref() == Some(name))
        .expect("the row")
        .clone();
    entry.bounds.center()
}

fn send(root: &mut Root, event: Event, pointer: Option<Point>) {
    let _ = root.handle_input(&event, pointer, Instant::now(), true);
}

fn finger(phase: fn(touch::Finger, Point) -> touch::Event, id: u64, at: Point) -> Event {
    Event::Touch(phase(touch::Finger(id), at))
}

fn pressed(id: touch::Finger, position: Point) -> touch::Event {
    touch::Event::FingerPressed {
        id,
        position,
        metadata: pointer::Metadata::NONE,
    }
}

fn moved(id: touch::Finger, position: Point) -> touch::Event {
    touch::Event::FingerMoved {
        id,
        position,
        metadata: pointer::Metadata::NONE,
    }
}

fn lifted(id: touch::Finger, position: Point) -> touch::Event {
    touch::Event::FingerLifted {
        id,
        position,
        metadata: pointer::Metadata::NONE,
    }
}

#[test]
fn spreading_two_fingers_reports_the_scale_and_presses_nothing() {
    let renderer = renderer();
    let Pinched {
        mut root,
        pressed: presses,
        scale,
        ended,
    } = pinched_list();
    root.layout(&renderer, WINDOW);

    let row = center_of(&mut root, "Row 3");
    let one = Point::new(row.x, row.y);
    let two = Point::new(row.x, row.y + 100.0);

    // First finger: pointer events plus its raw touch.
    send(
        &mut root,
        Event::Mouse(mouse::Event::CursorMoved { position: one }),
        Some(one),
    );
    send(
        &mut root,
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Some(one),
    );
    send(&mut root, finger(pressed, 1, one), Some(one));

    // Second finger lands: it is a pinch now.
    send(&mut root, finger(pressed, 2, two), Some(one));

    // Spread to twice the distance: the second finger moves 100px down.
    let two_far = Point::new(two.x, two.y + 100.0);
    send(&mut root, finger(moved, 2, two_far), Some(one));
    assert!(
        (scale.get() - 2.0).abs() < 0.01,
        "twice the distance is a scale of 2, got {}",
        scale.get()
    );

    // The first finger also moves: the list must not scroll under it.
    let one_up = Point::new(one.x, one.y - 60.0);
    send(
        &mut root,
        Event::Mouse(mouse::Event::CursorMoved { position: one_up }),
        Some(one_up),
    );
    send(&mut root, finger(moved, 1, one_up), Some(one_up));

    // Lift both.
    send(&mut root, finger(lifted, 2, two_far), Some(one_up));
    send(
        &mut root,
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        Some(one_up),
    );
    send(&mut root, Event::Mouse(mouse::Event::CursorLeft), None);
    send(&mut root, finger(lifted, 1, one_up), None);

    assert_eq!(presses.load(Ordering::SeqCst), 0, "a pinch presses nothing");
    assert!(
        ended.get().is_some_and(|factor| factor > 1.5),
        "the end reports the final scale"
    );
    assert_eq!(
        center_of(&mut root, "Row 3"),
        row,
        "and the list did not scroll"
    );
}

#[test]
fn one_finger_alone_is_not_a_pinch() {
    let renderer = renderer();
    let Pinched {
        mut root,
        pressed: presses,
        scale,
        ended,
    } = pinched_list();
    root.layout(&renderer, WINDOW);

    let row = center_of(&mut root, "Row 2");
    send(
        &mut root,
        Event::Mouse(mouse::Event::CursorMoved { position: row }),
        Some(row),
    );
    send(
        &mut root,
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Some(row),
    );
    send(&mut root, finger(pressed, 1, row), Some(row));
    send(
        &mut root,
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        Some(row),
    );
    send(&mut root, Event::Mouse(mouse::Event::CursorLeft), None);
    send(&mut root, finger(lifted, 1, row), None);

    assert_eq!(presses.load(Ordering::SeqCst), 1, "a tap still presses");
    assert_eq!(scale.get(), 1.0);
    assert_eq!(ended.get(), None);
}
