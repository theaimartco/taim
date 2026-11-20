//! Every phase-2 view of the TAIM view API in one window, all driven by
//! state handles.
// No console window behind the app in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
use taim::prelude::*;

fn main() -> taim::Result {
    taim::launch("Gallery", |cx| {
        let name = cx.signal(String::new());
        let password = cx.signal(String::new());
        let subscribe = cx.signal(true);
        let dark = cx.signal(false);
        let volume = cx.signal(40.0_f32);
        let submitted = cx.signal(String::from("Nothing submitted yet"));

        let form = column(views![
            text("Profile").size(24),
            text_field("Your name", name).on_submit(move || {
                submitted.set(format!("Submitted: {}", name.get()));
            }),
            text_field("Password", password).secret(),
            text(move || format!(
                "Hello, {}!",
                if name.get().is_empty() {
                    "stranger".into()
                } else {
                    name.get()
                }
            )),
            rule(),
            checkbox("Subscribe to updates", subscribe),
            switch("Dark mode", dark),
            row(views![
                text("Volume"),
                slider(0.0..=100.0, volume).step(5.0),
                text(move || format!("{:>3}", volume.get() as i32)),
            ])
            .gap(12.0)
            .align(Align::Center),
            progress(0.0..=100.0, volume),
            text(submitted),
        ])
        .gap(14.0)
        .padding(24.0);

        let long_list = column((1..=40).map(|n| text(format!("Item {n}")).into()))
            .gap(6.0)
            .padding(16.0);

        row(views![
            container(form).fill(),
            container(scroll(long_list)).fill(),
        ])
    })
}
