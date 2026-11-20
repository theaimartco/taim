use crate::{Lifetime, support::App};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use taim::prelude::*;
use taim::view::Root;

#[test]
fn a_thousand_navigation_cycles_release_page_owned_state() {
    let live = Arc::new(AtomicUsize::new(0));
    let mut handle = None;
    let root = Root::new(|cx| {
        cx.reduced_motion(true);
        let nav = Navigator::new(cx, 0_u32);
        handle = Some(nav);
        let live = live.clone();
        navigator(nav, move |scope, route| {
            let _ = scope.signal(Lifetime::new(&live));
            text(format!("Page {route}")).into()
        })
    });
    let nav = handle.expect("navigator");
    let mut app = App::new(root);
    let _ = app.root.settings_changed();
    assert_eq!(live.load(Ordering::SeqCst), 1);
    for route in 1..=1_000 {
        nav.push(route);
        app.frame();
        assert_eq!(nav.depth(), 2);
        assert_eq!(live.load(Ordering::SeqCst), 2);
        assert!(app.shows(&format!("Page {route}")));
        assert!(nav.pop());
        app.frame();
        app.frame(); // Reclaim the outgoing page after its transition completes.
        assert_eq!(live.load(Ordering::SeqCst), 1, "cycle {route}");
        assert_eq!(nav.depth(), 1);
    }
    assert!(app.shows("Page 0"));
    drop(app);
    assert_eq!(live.load(Ordering::SeqCst), 0);
}

#[test]
fn completed_pop_requests_layout_to_release_the_outgoing_page() {
    use taim_core::time::Duration;
    use taim_core::{Color, Rectangle, Renderer as _};

    let live = Arc::new(AtomicUsize::new(0));
    let mut handle = None;
    let root = Root::new(|cx| {
        let nav = Navigator::new(cx, 0_u32);
        handle = Some(nav);
        let live = live.clone();
        navigator(nav, move |scope, route| {
            let _ = scope.signal(Lifetime::new(&live));
            text(format!("Page {route}")).into()
        })
    });
    let nav = handle.expect("navigator");
    let mut app = App::new(root);
    let _ = app.root.settings_changed();
    nav.push(1);
    app.frame();
    app.wait(Duration::from_millis(400));
    assert!(nav.pop());
    app.frame();
    let _ = app.root.take_changes();

    // The shell paints animation frames without another layout unless asked.
    app.now += Duration::from_millis(400);
    app.renderer
        .reset(Rectangle::with_size(crate::support::WINDOW));
    app.root
        .paint(&mut app.renderer, Color::BLACK, None, app.now);
    assert!(!app.root.is_animating(app.now));
    assert!(
        app.root.take_changes().1,
        "a completed pop must request cleanup layout"
    );
    app.root.layout(&app.renderer, crate::support::WINDOW);
    assert_eq!(live.load(Ordering::SeqCst), 1);
    drop(app);
    assert_eq!(live.load(Ordering::SeqCst), 0);
}

#[test]
fn windows_with_different_motion_preferences_do_not_change_each_other() {
    fn window(reduced: bool) -> (App, Navigator<u32>) {
        let mut handle = None;
        let root = Root::new(|cx| {
            cx.reduced_motion(reduced);
            let nav = Navigator::new(cx, 0_u32);
            handle = Some(nav);
            navigator(nav, |_, route| text(format!("Page {route}")).into())
        });
        (App::new(root), handle.expect("navigator"))
    }
    let (mut reduced, reduced_nav) = window(true);
    let (mut normal, normal_nav) = window(false);
    let _ = reduced.root.settings_changed();
    let _ = normal.root.settings_changed();
    reduced_nav.push(1);
    reduced.frame();
    assert!(
        !reduced.root.is_animating(reduced.now),
        "reduced-motion window skips transitions"
    );
    normal_nav.push(1);
    normal.frame();
    assert!(
        normal.root.is_animating(normal.now),
        "normal window still animates"
    );
    reduced.frame();
    assert!(
        normal.root.is_animating(normal.now),
        "painting another window preserves the animation"
    );
}
