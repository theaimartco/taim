//! A counter written against the TAIM view API: state is a signal, the
//! buttons change it, and only the label that reads it is laid out again.
// No console window behind the app in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
use taim::prelude::*;

fn main() -> taim::Result {
    taim::launch("Counter", |cx| {
        let count = cx.signal(0_i64);

        container(
            column(views![
                text(move || format!("Count: {}", count.get())).size(32),
                row(views![
                    button("−1").on_press(move || count.update(|n| *n -= 1)),
                    button("Reset")
                        .on_press(move || {
                            count.set(0);
                        })
                        .enabled(move || count.get() != 0),
                    button("+1").on_press(move || count.update(|n| *n += 1)),
                ])
                .gap(8.0),
            ])
            .gap(16.0)
            .align(taim::view::views::Align::Center),
        )
        .center()
        .fill()
    })
}
