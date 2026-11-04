//! A phone-style application shell on the TAIM view API: section tabs, a
//! bottom navigation bar, filter chips, pull-to-refresh, an edge-swipe
//! drawer, a draggable bottom sheet, a long scrolling list, a text field
//! and an undoable notice. All of it is state handles; there is no message
//! type and no update function.
//!
//! The same code runs as a desktop window (`cargo run -p mobile_shell`) and
//! as an Android activity (see `docs/platform/android.md`).
use std::time::Duration;
use taim::prelude::*;

const DESTINATIONS: [(Icon, &str); 3] = [
    (Icon::HOME, "Home"),
    (Icon::SEARCH, "Search"),
    (Icon::PERSON, "Profile"),
];
const SECTIONS: [&str; 3] = ["Overview", "Activity", "Settings"];
const FILTERS: [&str; 3] = ["Unread", "Starred", "Muted"];

/// The application, ready to run on either platform.
pub fn app() -> taim::App {
    taim::app("Taim — Mobile Shell", |cx| {
        let destination = cx.signal(0_usize);
        let section = cx.signal(0_usize);
        let query = cx.signal(String::new());
        let filters = [cx.signal(true), cx.signal(false), cx.signal(false)];
        let refreshing = cx.signal(false);
        let drawer_open = cx.signal(false);
        let sheet_open = cx.signal(false);
        let notice = cx.signal(None::<String>);
        let note = cx.signal(String::new());

        let listing = rebuild(refreshing, |busy: &bool| {
            Some(if *busy {
                column(views![
                    skeleton(240.0, 16.0),
                    skeleton(200.0, 16.0),
                    skeleton(220.0, 16.0)
                ])
                .gap(10.0)
                .into()
            } else {
                row(views![
                    avatar("Ada Lovelace").size(36.0),
                    badge(text("Inbox").size(16), 3_u32)
                ])
                .gap(12.0)
                .align(Align::Center)
                .into()
            })
        });

        let page = container(
            column(views![
                search("Search", query),
                row(FILTERS
                    .iter()
                    .zip(filters)
                    .map(|(name, on)| chip(*name, on).into()))
                .gap(8.0),
                text(move || DESTINATIONS[destination.get()].1.to_owned()).size(32),
                text(move || SECTIONS[section.get().min(SECTIONS.len() - 1)].to_owned()).size(16),
                listing,
                text("Swipe in from the left edge to open the drawer.").size(14),
                text_field("Write a note", note),
                button("Show bottom sheet").on_press(move || {
                    let _ = sheet_open.set(true);
                }),
                column(
                    (1..=40)
                        .map(|index| {
                            list_tile(
                                Some(Icon::MAIL),
                                format!("Message {index}"),
                                Some("Drag to scroll; tap to open".to_owned()),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
                .fill_width(),
            ])
            .gap(16.0)
            .align(Align::Center)
            .padding(16.0),
        )
        .center()
        .fill();

        let body = column(views![
            tabs(SECTIONS, section),
            pull_to_refresh(scroll(page), refreshing).on_refresh(move || {
                // Stand-in for real work: finish after a moment.
                spawn(async move {
                    std::thread::sleep(Duration::from_millis(900));
                    let _ = refreshing.set(false);
                    let _ = notice.set(Some("Refreshed".to_owned()));
                });
            }),
        ])
        .fill_width()
        .fill_height();

        let navigation = column(views![
            text("Taim").size(24),
            text("Mobile shell example").size(13),
            button("Close").on_press(move || {
                let _ = drawer_open.set(false);
            }),
        ])
        .gap(12.0)
        .padding(8.0);

        let sheet_body = column(views![
            text("Bottom sheet").size(20),
            text("Drag the handle to switch detents, or fling it down to dismiss.").size(13),
        ])
        .gap(12.0);

        scaffold(
            container(text("Taim").size(20)).padding(16.0),
            body,
            nav_bar(DESTINATIONS, destination),
            views![
                drawer(navigation, drawer_open),
                sheet(sheet_body, sheet_open)
                    .detents([0.35, 0.85])
                    .on_dismiss(move || {
                        let _ = notice.set(Some("Sheet dismissed".to_owned()));
                    }),
                snackbar(notice).action("Undo", move || {
                    let _ = sheet_open.set(true);
                }),
            ],
        )
    })
}

/// The Android entry point: android-activity calls it on its own thread
/// once the activity starts.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android: taim::android::AndroidApp) {
    if let Err(error) = app().run_android(android) {
        log::error!("mobile_shell stopped: {error}");
    }
}
