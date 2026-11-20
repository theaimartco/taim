use crate::{Lifetime, support::App};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use taim::prelude::*;
use taim::view::Root;
use taim_core::{Event, Point, Size, mouse};

#[test]
fn headless_roots_release_editor_state_after_resize_and_focus_loss() {
    let live = Arc::new(AtomicUsize::new(0));
    for cycle in 0..128 {
        let mut value = None;
        let root = Root::new(|cx| {
            let _ = cx.signal(Lifetime::new(&live));
            let state = cx.signal(format!("Window {cycle}"));
            value = Some(state);
            text_field("", state)
        });
        let mut app = App::new(root);
        assert_eq!(live.load(Ordering::SeqCst), 1);
        let at = Point::new(15.0, 15.0);
        let _ = app.root.handle(
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Some(at),
            app.now,
        );
        assert!(app.root.wants_text_input());
        for size in [Size::ZERO, Size::new(1.0, 1.0), Size::new(1920.0, 1080.0)] {
            app.root.layout(&app.renderer, size);
        }
        let _ = app.root.release_focus(app.now);
        assert!(!app.root.wants_text_input());
        assert_eq!(value.expect("state").get(), format!("Window {cycle}"));
        drop(app);
        assert_eq!(live.load(Ordering::SeqCst), 0, "cycle {cycle}");
    }
}
