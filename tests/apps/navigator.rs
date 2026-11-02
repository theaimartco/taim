//! Moving between screens: push, back, page state, transitions.
use crate::support::App;

use taim::prelude::*;
use taim::view::Root;
use taim_core::time::Duration;

#[derive(Debug, Clone, PartialEq)]
enum Route {
    Inbox,
    Message(u32),
}

/// An inbox whose messages open on their own page. Each page counts taps
/// in state of its own.
fn mail() -> Root {
    Root::new(|cx| {
        let nav = Navigator::new(cx, Route::Inbox);

        navigator(nav, move |cx, route| match route {
            Route::Inbox => {
                let opened = cx.signal(0_u32);
                column(views![
                    text("Inbox"),
                    text(move || format!("Opened {}", opened.get())),
                    button("Open 7").on_press(move || {
                        opened.update(|n| *n += 1);
                        nav.push(Route::Message(7));
                    }),
                ])
                .into()
            }
            Route::Message(id) => {
                let likes = cx.signal(0_u32);
                let id = *id;
                column(views![
                    text(format!("Message {id}")),
                    text(move || format!("Likes {}", likes.get())),
                    button("Like").on_press(move || likes.update(|n| *n += 1)),
                    button("Reply").on_press(move || nav.push(Route::Message(id + 100))),
                    button("Done").on_press(move || {
                        let _ = nav.pop();
                    }),
                ])
                .into()
            }
        })
    })
}

#[test]
fn pushing_shows_the_new_page_and_back_returns() {
    let mut app = App::new(mail());
    assert!(app.shows("Inbox"));

    app.tap("Open 7");
    app.wait(Duration::from_millis(400));
    assert!(app.shows("Message 7"));
    assert!(
        !app.shows("Inbox"),
        "the page underneath is hidden once settled"
    );

    assert!(app.back(), "back pops a pushed page");
    app.wait(Duration::from_millis(400));
    assert!(app.shows("Inbox"));
    assert!(!app.shows("Message 7"));

    assert!(!app.back(), "on the first page, back is the platform's");
}

#[test]
fn a_page_you_return_to_kept_its_state_and_a_popped_page_did_not() {
    let mut app = App::new(mail());

    app.tap("Open 7");
    app.wait(Duration::from_millis(400));
    app.tap("Like");
    app.tap("Like");
    assert!(app.shows("Likes 2"));

    // Deeper, then back: the message page is as it was.
    app.tap("Reply");
    app.wait(Duration::from_millis(400));
    assert!(app.shows("Message 107"));
    assert!(app.back());
    app.wait(Duration::from_millis(400));
    assert!(app.shows("Likes 2"), "the page kept its state");

    // Back to the inbox, which counted the open.
    app.tap("Done");
    app.wait(Duration::from_millis(400));
    assert!(app.shows("Opened 1"));

    // Opening again builds a fresh page.
    app.tap("Open 7");
    app.wait(Duration::from_millis(400));
    assert!(app.shows("Likes 0"), "a popped page's state was dropped");
}

#[test]
fn pages_slide_and_ignore_taps_while_moving() {
    let mut app = App::new(mail());

    app.tap("Open 7");
    assert!(app.root.is_animating(app.now), "a push slides in");

    // Mid-slide, the old page's button is still under the finger; it must
    // not react.
    app.wait(Duration::from_millis(50));
    app.tap("Open 7");

    app.wait(Duration::from_millis(500));
    assert!(!app.root.is_animating(app.now), "and settles");
    assert!(app.shows("Message 7"));
    assert!(app.back());
    app.wait(Duration::from_millis(500));
    assert!(app.shows("Opened 1"), "only the first tap opened a page");
}

#[test]
fn back_closes_a_dialog_on_the_page_before_popping_the_page() {
    let mut app = App::new(Root::new(|cx| {
        let nav = Navigator::new(cx, 0_u8);
        navigator(nav, move |cx, page| {
            if *page == 0 {
                button("Next").on_press(move || nav.push(1)).into()
            } else {
                let open = cx.signal(true);
                column(views![text("Second"), dialog(open, text("Saved"))]).into()
            }
        })
    }));

    app.tap("Next");
    app.wait(Duration::from_millis(400));
    assert!(app.shows("Second"));

    assert!(app.back(), "the dialog takes the first back");
    app.wait(Duration::from_millis(400));
    assert!(app.shows("Second"), "and the page stays");

    assert!(app.back(), "the second back pops the page");
    app.wait(Duration::from_millis(400));
    assert!(!app.shows("Second"));
}
