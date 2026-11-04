//! {{name}}: a TAIM application for the desktop and Android.
//!
//! A list of notes kept between launches. Tap a note to open it on its own
//! page; back returns. Run it with `cargo run`, or on a phone with the
//! Gradle project in `android/` (see README.md).
use taim::prelude::*;
use taim::storage::Store;

#[derive(Debug, Clone, PartialEq)]
enum Page {
    Notes,
    Note(usize),
}

/// The application, ready to run on either platform.
pub fn app() -> taim::App {
    taim::app("{{name}}", |cx| {
        let notes = cx.signal(load());
        let draft = cx.signal(String::new());
        let nav = Navigator::new(cx, Page::Notes);

        navigator(nav, move |_cx, page| match page {
            Page::Notes => scaffold(
                app_bar("{{name}}", []),
                list(
                    move || notes.with(Vec::len),
                    move |index| {
                        let title = notes.with(|all| all.get(index).cloned().unwrap_or_default());
                        button(title)
                            .ghost()
                            .on_press(move || nav.push(Page::Note(index)))
                            .into()
                    },
                )
                .gap(4.0),
                container(
                    row(views![
                        container(text_field("New note", draft)).fill_width(),
                        button("Add").on_press(move || {
                            let note = draft.replace(String::new());
                            if !note.trim().is_empty() {
                                notes.update(|all| all.push(note));
                                save(&notes.get());
                            }
                        }),
                    ])
                    .gap(8.0)
                    .fill_width(),
                )
                .padding(12.0)
                .fill_width(),
                [],
            ),
            Page::Note(index) => {
                let index = *index;
                let body = notes.with(|all| all.get(index).cloned().unwrap_or_default());
                container(
                    column(views![
                        text(body).size(20.0),
                        row(views![
                            button("Back").secondary().on_press(move || {
                                let _ = nav.pop();
                            }),
                            button("Delete").danger().on_press(move || {
                                notes.update(|all| {
                                    if index < all.len() {
                                        let _ = all.remove(index);
                                    }
                                });
                                save(&notes.get());
                                let _ = nav.pop();
                            }),
                        ])
                        .gap(8.0),
                    ])
                    .gap(24.0),
                )
                .padding(24.0)
                .fill()
                .into()
            }
        })
    })
}

/// The notes saved by the last run.
fn load() -> Vec<String> {
    let Ok(store) = Store::open("notes") else {
        return Vec::new();
    };
    let count = store.get_as::<usize>("count").unwrap_or(0);
    (0..count)
        .filter_map(|index| store.get(&format!("note.{index}")).map(str::to_owned))
        .collect()
}

fn save(notes: &[String]) {
    let result = (|| -> Result<(), taim::storage::Error> {
        let mut store = Store::open("notes")?;
        let previous = store.get_as::<usize>("count").unwrap_or(0);
        for (index, note) in notes.iter().enumerate() {
            store.set(&format!("note.{index}"), note)?;
        }
        for index in notes.len()..previous {
            let _ = store.remove(&format!("note.{index}"))?;
        }
        store.set("count", notes.len())
    })();

    if let Err(error) = result {
        log::warn!("the notes could not be saved: {error}");
    }
}

/// The Android entry point: android-activity calls it on its own thread
/// once the activity starts.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android: taim::android::AndroidApp) {
    if let Err(error) = app().run_android(android) {
        log::error!("{{crate}} stopped: {error}");
    }
}
