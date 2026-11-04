//! A feed of a hundred thousand items, built only near the screen.
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::support::{App, WINDOW};

use taim::prelude::*;
use taim::view::Root;
use taim_core::Point;
use taim_core::time::Duration;

const ITEMS: usize = 100_000;

fn feed(builds: &Arc<AtomicUsize>) -> Root {
    let builds = builds.clone();
    Root::new(move |_| {
        list(ITEMS, move |index| {
            let _ = builds.fetch_add(1, Ordering::SeqCst);
            text(format!("Item {index}")).size(16.0).into()
        })
        .gap(8.0)
    })
}

fn indices(app: &mut App) -> Vec<usize> {
    app.names()
        .iter()
        .filter_map(|name| name.strip_prefix("Item ")?.parse().ok())
        .collect()
}

#[test]
fn only_items_near_the_screen_are_built() {
    let builds = Arc::new(AtomicUsize::new(0));
    let mut app = App::new(feed(&builds));

    let built = builds.load(Ordering::SeqCst);
    assert!(
        built < 150,
        "a 640px screen of 100,000 items built {built} of them"
    );
    assert_eq!(indices(&mut app).first(), Some(&0));
}

#[test]
fn scrolling_far_builds_the_new_items_and_drops_the_old() {
    let builds = Arc::new(AtomicUsize::new(0));
    let mut app = App::new(feed(&builds));
    let middle = Point::new(WINDOW.width / 2.0, WINDOW.height / 2.0);

    app.wheel(middle, -300_000.0);
    let shown = indices(&mut app);

    assert!(
        shown.first().is_some_and(|first| *first > 5_000),
        "far down the list: {:?}",
        shown.first()
    );
    assert!(!shown.contains(&0), "the first items were dropped");
    assert!(
        shown.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "the items shown are consecutive"
    );
    assert!(
        builds.load(Ordering::SeqCst) < 400,
        "and only nearby items were ever built"
    );
}

#[test]
fn a_fling_asks_for_layout_when_it_runs_past_the_built_items() {
    let builds = Arc::new(AtomicUsize::new(0));
    let mut app = App::new(feed(&builds));
    let start = Point::new(WINDOW.width / 2.0, WINDOW.height - 40.0);

    app.flick(start, 300.0);
    app.frame();
    assert!(app.root.is_animating(app.now), "the list is flinging");

    // Paint frames without laying out, as the shell does while nothing
    // asks for layout: the list must ask once it outruns its items.
    let mut asked = false;
    for _ in 0..120 {
        app.now += Duration::from_millis(16);
        let _ = app.root.take_changes();
        {
            use taim_core::Renderer as _;
            app.renderer.reset(taim_core::Rectangle::with_size(WINDOW));
        }
        app.root
            .paint(&mut app.renderer, taim_core::Color::BLACK, None, app.now);
        if app.root.take_changes().1 {
            asked = true;
            break;
        }
    }
    assert!(asked, "a long fling must request a layout to build items");

    app.wait(Duration::from_secs(3));
    let shown = indices(&mut app);
    assert!(
        shown.first().is_some_and(|first| *first > 20),
        "the fling carried the list down to {:?}",
        shown.first()
    );
}

#[test]
fn items_of_different_heights_do_not_overlap() {
    let mut app = App::new(Root::new(|_| {
        list(1_000, |index| {
            let size = if index % 3 == 0 { 40.0 } else { 14.0 };
            text(format!("Item {index}")).size(size).into()
        })
    }));
    let middle = Point::new(WINDOW.width / 2.0, WINDOW.height / 2.0);
    app.wheel(middle, -2_000.0);

    let tree = app.semantics();
    let mut bounds: Vec<_> = tree
        .in_order()
        .into_iter()
        .filter(|entry| {
            entry
                .name
                .as_deref()
                .is_some_and(|name| name.starts_with("Item "))
        })
        .map(|entry| entry.bounds)
        .collect();
    bounds.sort_by(|a, b| a.y.total_cmp(&b.y));

    assert!(bounds.len() > 5);
    for pair in bounds.windows(2) {
        assert!(
            pair[0].y + pair[0].height <= pair[1].y + 0.5,
            "{:?} overlaps {:?}",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn the_list_follows_its_count() {
    let mut app = App::new(Root::new(|cx| {
        let count = cx.signal(3_usize);
        column(views![
            button("More").on_press(move || count.update(|n| *n += 2)),
            container(list(count, |index| text(format!("Item {index}")).into())).height(400.0),
        ])
    }));

    assert_eq!(indices(&mut app), vec![0, 1, 2]);
    app.tap("More");
    assert_eq!(indices(&mut app), vec![0, 1, 2, 3, 4]);
}
