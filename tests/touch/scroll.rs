//! A finger on a scrollable list, the way Android delivers it: the shell
//! turns the first finger into pointer events marked as touch.
//!
//! These pin the behaviour a phone user expects from any list: a tap on a
//! row activates it, a drag that starts on a row scrolls instead and does
//! not activate it, and a quick drag keeps moving after the finger lifts.
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use taim::prelude::*;
use taim::view::Root;
use taim_accessibility::tree::Tree;
use taim_core::renderer::Headless as _;
use taim_core::semantics::Role;
use taim_core::time::{Duration, Instant};
use taim_core::{Color, Event, Point, Size, mouse};

type Renderer = taim::Renderer;

const WINDOW: Size = Size {
    width: 360.0,
    height: 640.0,
};

fn renderer() -> Renderer {
    taim_futures::futures::executor::block_on(Renderer::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("the CPU renderer starts without a GPU or a window")
}

/// A long list of buttons; returns the root and how often each was pressed.
fn list() -> (Root, Arc<Vec<AtomicUsize>>) {
    let (root, presses, _) = list_with(false);
    (root, presses)
}

/// The list, optionally inside pull-to-refresh; also returns how often a
/// refresh started.
fn list_with(refreshable: bool) -> (Root, Arc<Vec<AtomicUsize>>, Arc<AtomicUsize>) {
    let refreshes = Arc::new(AtomicUsize::new(0));
    let presses: Arc<Vec<AtomicUsize>> = Arc::new((0..60).map(|_| AtomicUsize::new(0)).collect());

    let root = Root::new({
        let presses = presses.clone();
        let refreshes = refreshes.clone();
        move |cx| {
            let rows: Vec<AnyView> = (0..60)
                .map(|index| {
                    let presses = presses.clone();
                    button(format!("Row {index}"))
                        .on_press(move || {
                            let _ = presses[index].fetch_add(1, Ordering::SeqCst);
                        })
                        .into()
                })
                .collect();

            let list = scroll(column(rows).gap(8.0).fill_width());

            if refreshable {
                let refreshing = cx.signal(false);
                let refreshes = refreshes.clone();
                pull_to_refresh(list, refreshing)
                    .on_refresh(move || {
                        let _ = refreshes.fetch_add(1, Ordering::SeqCst);
                    })
                    .into()
            } else {
                AnyView::from(list)
            }
        }
    });

    (root, presses, refreshes)
}

/// Where the button named `name` is, from the semantic tree.
fn center_of(root: &mut Root, name: &str) -> Point {
    let tree = Tree::from_forest(&[root.describe(WINDOW)]);
    let entry = tree
        .in_order()
        .into_iter()
        .find(|entry| entry.role == Role::Button && entry.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("a button named {name:?}"))
        .clone();

    Point::new(
        entry.bounds.x + entry.bounds.width / 2.0,
        entry.bounds.y + entry.bounds.height / 2.0,
    )
}

/// One finger, as the shell reports it to the views.
struct Finger<'a> {
    root: &'a mut Root,
    at: Point,
    now: Instant,
}

impl<'a> Finger<'a> {
    fn down(root: &'a mut Root, at: Point, now: Instant) -> Self {
        let mut finger = Self { root, at, now };
        finger.send(Event::Mouse(mouse::Event::CursorMoved { position: at }));
        finger.send(Event::Mouse(mouse::Event::ButtonPressed(
            mouse::Button::Left,
        )));
        finger
    }

    fn send(&mut self, event: Event) {
        let _ = self
            .root
            .handle_input(&event, Some(self.at), self.now, true);
    }

    /// Moves to `to` in `steps` equal steps, `every` apart.
    fn slide(&mut self, to: Point, steps: u32, every: Duration) {
        let from = self.at;
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            self.at = Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
            self.now += every;
            self.send(Event::Mouse(mouse::Event::CursorMoved {
                position: self.at,
            }));
        }
    }

    fn lift(mut self) -> Instant {
        self.send(Event::Mouse(mouse::Event::ButtonReleased(
            mouse::Button::Left,
        )));
        let _ = self.root.handle_input(
            &Event::Mouse(mouse::Event::CursorLeft),
            None,
            self.now,
            true,
        );
        self.now
    }
}

fn pressed(presses: &[AtomicUsize]) -> usize {
    presses
        .iter()
        .map(|count| count.load(Ordering::SeqCst))
        .sum()
}

#[test]
fn a_tap_on_a_row_presses_it() {
    let renderer = renderer();
    let (mut root, presses) = list();
    root.layout(&renderer, WINDOW);

    let at = center_of(&mut root, "Row 2");
    let _ = Finger::down(&mut root, at, Instant::now()).lift();

    assert_eq!(presses[2].load(Ordering::SeqCst), 1);
    assert_eq!(pressed(&presses), 1);
}

#[test]
fn a_tap_that_wobbles_less_than_the_slop_still_presses() {
    let renderer = renderer();
    let (mut root, presses) = list();
    root.layout(&renderer, WINDOW);

    let at = center_of(&mut root, "Row 1");
    let mut finger = Finger::down(&mut root, at, Instant::now());
    finger.slide(
        Point::new(at.x + 2.0, at.y - 4.0),
        2,
        Duration::from_millis(16),
    );
    let _ = finger.lift();

    assert_eq!(presses[1].load(Ordering::SeqCst), 1);
}

#[test]
fn a_drag_that_starts_on_a_row_scrolls_and_presses_nothing() {
    let mut renderer = renderer();
    let (mut root, presses) = list();
    root.layout(&renderer, WINDOW);

    let before = center_of(&mut root, "Row 5");
    // Slowly, so the release does not fling.
    let mut finger = Finger::down(&mut root, before, Instant::now());
    finger.slide(
        Point::new(before.x, before.y - 200.0),
        20,
        Duration::from_millis(50),
    );
    finger.now += Duration::from_millis(300);
    let released = finger.lift();

    let after = center_of(&mut root, "Row 5");

    assert_eq!(pressed(&presses), 0, "scrolling must not press a row");
    // The content follows the finger from the moment the movement became
    // a scroll: past the 8px slop, which here is the first 10px step. It
    // does not jump to catch up with the slop, as on Android.
    let moved = before.y - after.y;
    assert!(
        (moved - 190.0).abs() < 1.0,
        "the content follows the finger: moved {moved} for a 200px drag"
    );

    root.paint(
        &mut renderer,
        Color::BLACK,
        None,
        released + Duration::from_secs(1),
    );
    assert_eq!(
        center_of(&mut root, "Row 5"),
        after,
        "a slow release does not fling"
    );
}

#[test]
fn a_quick_drag_flings_and_slows_to_a_stop() {
    let mut renderer = renderer();
    let (mut root, presses) = list();
    root.layout(&renderer, WINDOW);

    let start = center_of(&mut root, "Row 3");
    let mut finger = Finger::down(&mut root, start, Instant::now());
    finger.slide(
        Point::new(start.x, start.y - 150.0),
        5,
        Duration::from_millis(10),
    );
    let released = finger.lift();
    let dragged = center_of(&mut root, "Row 3");

    assert_eq!(pressed(&presses), 0);
    assert!(root.is_animating(released), "a quick release keeps moving");

    let mut paint_at = |root: &mut Root, at: Instant| {
        root.paint(&mut renderer, Color::BLACK, None, at);
        center_of(root, "Row 3").y
    };

    let soon = paint_at(&mut root, released + Duration::from_millis(100));
    let later = paint_at(&mut root, released + Duration::from_millis(400));
    let later_still = paint_at(&mut root, released + Duration::from_millis(500));
    let settled = paint_at(&mut root, released + Duration::from_secs(5));

    assert!(soon < dragged.y, "the fling carries on past the drag");
    assert!(later < soon, "and keeps going for a while");
    assert!(
        dragged.y - soon > later - later_still,
        "decelerating: {} in the first 100ms, {} 400ms later",
        dragged.y - soon,
        later - later_still
    );
    assert!(
        !root.is_animating(released + Duration::from_secs(5)),
        "and stops"
    );
    assert!(settled <= later, "never springing back");
}

#[test]
fn touching_a_moving_list_stops_it_without_pressing() {
    let mut renderer = renderer();
    let (mut root, presses) = list();
    root.layout(&renderer, WINDOW);

    let start = center_of(&mut root, "Row 3");
    let mut finger = Finger::down(&mut root, start, Instant::now());
    finger.slide(
        Point::new(start.x, start.y - 150.0),
        5,
        Duration::from_millis(10),
    );
    let released = finger.lift();

    let caught_at = released + Duration::from_millis(50);
    root.paint(&mut renderer, Color::BLACK, None, caught_at);
    let row = center_of(&mut root, "Row 8");

    let _ = Finger::down(&mut root, row, caught_at).lift();

    assert_eq!(pressed(&presses), 0, "catching a fling is not a tap");

    root.paint(
        &mut renderer,
        Color::BLACK,
        None,
        caught_at + Duration::from_millis(500),
    );
    assert_eq!(
        center_of(&mut root, "Row 8"),
        row,
        "the list stays where the finger stopped it"
    );
}

#[test]
fn a_mouse_drag_does_not_scroll() {
    // Dragging with a mouse selects or does nothing, as on the desktop;
    // only a finger drags content.
    let renderer = renderer();
    let (mut root, _) = list();
    root.layout(&renderer, WINDOW);

    let before = center_of(&mut root, "Row 5");
    let now = Instant::now();
    let send = |root: &mut Root, event: Event, at: Point| {
        let _ = root.handle_input(&event, Some(at), now, false);
    };
    send(
        &mut root,
        Event::Mouse(mouse::Event::CursorMoved { position: before }),
        before,
    );
    send(
        &mut root,
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        before,
    );
    let to = Point::new(before.x, before.y - 200.0);
    send(
        &mut root,
        Event::Mouse(mouse::Event::CursorMoved { position: to }),
        to,
    );

    assert_eq!(center_of(&mut root, "Row 5"), before);
}

#[test]
fn pulling_down_at_the_top_refreshes_even_from_a_row() {
    let renderer = renderer();
    let (mut root, presses, refreshes) = list_with(true);
    root.layout(&renderer, WINDOW);

    let at = center_of(&mut root, "Row 1");
    let mut finger = Finger::down(&mut root, at, Instant::now());
    finger.slide(
        Point::new(at.x, at.y + 200.0),
        20,
        Duration::from_millis(16),
    );
    let _ = finger.lift();

    assert_eq!(refreshes.load(Ordering::SeqCst), 1, "a long pull refreshes");
    assert_eq!(pressed(&presses), 0, "and presses nothing");
}

#[test]
fn pulling_down_a_scrolled_list_scrolls_it_back_instead_of_refreshing() {
    let renderer = renderer();
    let (mut root, _, refreshes) = list_with(true);
    root.layout(&renderer, WINDOW);

    // Scroll down first, slowly.
    let start = center_of(&mut root, "Row 6");
    let mut finger = Finger::down(&mut root, start, Instant::now());
    finger.slide(
        Point::new(start.x, start.y - 300.0),
        30,
        Duration::from_millis(40),
    );
    finger.now += Duration::from_millis(300);
    let _ = finger.lift();
    let scrolled = center_of(&mut root, "Row 6");

    // Then pull down by less than the distance scrolled.
    let mut finger = Finger::down(&mut root, scrolled, Instant::now());
    finger.slide(
        Point::new(scrolled.x, scrolled.y + 150.0),
        15,
        Duration::from_millis(40),
    );
    finger.now += Duration::from_millis(300);
    let _ = finger.lift();

    assert_eq!(refreshes.load(Ordering::SeqCst), 0);
    assert!(
        center_of(&mut root, "Row 6").y > scrolled.y + 100.0,
        "the list scrolled back towards its top"
    );
}
