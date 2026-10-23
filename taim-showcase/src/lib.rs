//! Every TAIM view, in every state worth seeing, in one runnable window.
//!
//! Run it with `cargo run -p taim-showcase`.
//!
//! The showcase is written against the TAIM view API only: its state is a
//! handful of state handles, and every control binds to one of them. Each
//! section shows a family of views in its real states (default, selected,
//! disabled, loading, at a limit), because a gallery that only shows the
//! happy path hides exactly the cases that decide whether a view is done.
//!
//! [`build`] is public so `tests/headless_semantics.rs` can drive the exact
//! same tree without a window.
use taim::prelude::*;
use taim::theme::palette;
use taim::view::views::{Date, Time};
use taim::view::{AnyView, State};
use taim::{Color, Theme};

/// Opens the showcase window.
pub fn main() -> taim::Result {
    taim::launch("Taim — Showcase", build)
}

/// The showcase's sections, which double as the bottom bar's destinations.
const GROUPS: [(&str, &str); 4] = [
    ("\u{25C9}", "Controls"),
    ("\u{2261}", "Collections"),
    ("\u{25F4}", "Pickers"),
    ("\u{25F1}", "Surfaces"),
];

/// UI zoom levels offered by the accessibility panel.
const SCALES: [(f32, &str); 4] = [(1.0, "100%"), (1.25, "125%"), (1.5, "150%"), (2.0, "200%")];

/// Everything the showcase remembers.
#[derive(Clone, Copy)]
struct Model {
    dark: State<bool>,
    high_contrast: State<bool>,
    reduced_motion: State<bool>,
    scale: State<usize>,
    group: State<usize>,

    switched: State<bool>,
    checked: State<bool>,
    level: State<f32>,
    quantity: State<i32>,
    query: State<String>,
    notes: Buffer,
    filters: [State<bool>; 3],
    mode: State<usize>,
    size: State<&'static str>,
    country: State<Option<usize>>,

    tab: State<usize>,
    page: State<usize>,
    section: State<Option<usize>>,
    loading: State<bool>,

    month: State<Date>,
    day: State<Date>,
    time: State<Time>,

    drawer: State<bool>,
    sheet: State<bool>,
    dialog: State<bool>,
    notice: State<Option<String>>,
}

/// Builds the showcase into `cx`'s window.
pub fn build(cx: &mut Scope) -> AnyView {
    let model = Model {
        dark: cx.signal(true),
        high_contrast: cx.signal(false),
        reduced_motion: cx.signal(false),
        scale: cx.signal(0),
        group: cx.signal(0),

        switched: cx.signal(true),
        checked: cx.signal(false),
        level: cx.signal(0.4),
        quantity: cx.signal(3),
        query: cx.signal(String::new()),
        notes: Buffer::new(cx, "Multi-line notes.\nClick, drag, double-click a word,\nor type here."),
        filters: [cx.signal(true), cx.signal(false), cx.signal(false)],
        mode: cx.signal(0),
        size: cx.signal("Medium"),
        country: cx.signal(None),

        tab: cx.signal(0),
        page: cx.signal(0),
        section: cx.signal(Some(0)),
        loading: cx.signal(false),

        month: cx.signal(Date::new(2026, 8, 1)),
        day: cx.signal(Date::new(2026, 8, 13)),
        time: cx.signal(Time::new(9, 30)),

        drawer: cx.signal(false),
        sheet: cx.signal(false),
        dialog: cx.signal(false),
        notice: cx.signal(None),
    };

    cx.theme(move || theme(model.dark.get(), model.high_contrast.get()));
    cx.scale(move || SCALES[model.scale.get()].0);
    cx.reduced_motion(model.reduced_motion);

    // Choosing a day is worth a notice.
    let body = scroll(
        container(rebuild(model.group, move |group: &usize| {
            Some(match group {
                0 => controls(model),
                1 => collections(model),
                2 => pickers(model),
                _ => surfaces(model),
            })
        }))
        .padding(24.0)
        .fill_width(),
    );

    let header = container(
        row(views![
            icon(Icon::HOME).size(20),
            text("Taim Showcase").size(20),
            space(),
            icon_button(Icon::SETTINGS).on_press(move || {
                let _ = model.drawer.set(true);
            }),
            switch("Dark", model.dark),
        ])
        .gap(16.0)
        .align(Align::Center)
        .fill_width(),
    )
    .padding(16.0)
    .fill_width();

    let page = column(views![
        header,
        accessibility(model),
        container(body).fill(),
        nav_bar(GROUPS, model.group),
    ])
    .fill_width()
    .fill_height();

    layers(views![
        page,
        drawer(navigation(model), model.drawer),
        sheet(
            column(views![
                text("Bottom sheet").size(20),
                text("Two detents. Drag the handle, or fling down to dismiss.").size(13),
            ])
            .gap(12.0),
            model.sheet,
        )
        .detents([0.35, 0.8]),
        dialog(
            model.dialog,
            column(views![
                text("Discard draft?").size(20),
                text("This cannot be undone.").size(13),
                row(views![
                    space(),
                    button("Cancel").on_press(move || {
                        let _ = model.dialog.set(false);
                    }),
                    button("Discard").on_press(move || {
                        let _ = model.dialog.set(false);
                    }),
                ])
                .gap(8.0)
                .fill_width(),
            ])
            .gap(12.0),
        ),
        snackbar(model.notice),
    ])
    .into()
}

/// The theme for the dark and high-contrast switches.
fn theme(dark: bool, high_contrast: bool) -> Theme {
    match (high_contrast, dark) {
        (true, true) => Theme::custom(
            "Taim High Contrast Dark",
            palette::Seed {
                background: Color::BLACK,
                text: Color::WHITE,
                primary: Color::from_rgb8(120, 190, 255),
                success: Color::from_rgb8(0, 255, 128),
                warning: Color::from_rgb8(255, 213, 0),
                danger: Color::from_rgb8(255, 120, 120),
            },
        ),
        (true, false) => Theme::custom(
            "Taim High Contrast Light",
            palette::Seed {
                background: Color::WHITE,
                text: Color::BLACK,
                primary: Color::from_rgb8(0, 51, 153),
                success: Color::from_rgb8(0, 92, 46),
                warning: Color::from_rgb8(122, 74, 0),
                danger: Color::from_rgb8(158, 0, 0),
            },
        ),
        (false, true) => Theme::Dark,
        (false, false) => Theme::Light,
    }
}

/// The accessibility switches. A view that ignores reduced motion, clips
/// at a larger scale, or vanishes at high contrast is broken, and the only
/// way to find out is to be able to switch each one on and look.
fn accessibility(model: Model) -> AnyView {
    container(
        column(views![
            row(views![
                icon(Icon::VISIBILITY).size(18),
                text("Accessibility").size(15)
            ])
            .gap(8.0)
            .align(Align::Center),
            row(views![
                switch("Reduced motion", model.reduced_motion),
                switch("High contrast", model.high_contrast),
            ])
            .gap(24.0)
            .align(Align::Center),
            row(views![
                text("Zoom").size(13),
                segmented(SCALES.map(|(_, label)| label), model.scale),
            ])
            .gap(12.0)
            .align(Align::Center)
            .fill_width(),
        ])
        .gap(12.0)
        .fill_width(),
    )
    .padding(16.0)
    .fill_width()
    .into()
}

fn heading(title: &'static str) -> AnyView {
    text(title).size(16).into()
}

fn notify(model: Model, message: &'static str) -> impl Fn() + 'static {
    move || {
        let _ = model.notice.set(Some(message.to_owned()));
    }
}

fn controls(model: Model) -> AnyView {
    const FILTERS: [&str; 3] = ["Unread", "Starred", "Muted"];

    column(views![
        heading("Buttons"),
        row(views![
            button("Default").on_press(notify(model, "Something happened")),
            button("Disabled").enabled(false),
            tooltip(
                button("With tooltip").on_press(notify(model, "Something happened")),
                "Explains itself",
            ),
        ])
        .gap(12.0),
        heading("Selection"),
        row(views![
            switch("Switch", model.switched),
            checkbox("Checkbox", model.checked),
        ])
        .gap(24.0)
        .align(Align::Center),
        heading("Radio buttons"),
        row(["Small", "Medium", "Large"]
            .into_iter()
            .map(|size| radio(size, size, model.size).into()))
        .gap(16.0),
        heading("Dropdown"),
        dropdown(["India", "Japan", "Kenya", "Peru"], model.country).placeholder("Country"),
        heading("Segmented control"),
        segmented(["List", "Grid", "Compact"], model.mode),
        heading("Chips"),
        row(FILTERS
            .iter()
            .zip(model.filters)
            .map(|(name, on)| chip(*name, on).into()))
        .gap(8.0),
        heading("Stepper"),
        row(views![
            stepper(model.quantity, 1..=5),
            text(move || format!("Quantity: {}", model.quantity.get())).size(16),
        ])
        .gap(12.0)
        .align(Align::Center),
        heading("Slider and progress"),
        slider(0.0..=1.0, model.level).step(0.01),
        progress(0.0..=1.0, model.level),
        heading("Text entry"),
        text_field("A plain text field", model.query),
        search("Search", model.query),
        heading("Pictures"),
        row(views![
            svg(taim::view::svg_handle(
                br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" fill="#5b6cff"/><path d="M7 12l3 3 7-7" stroke="white" stroke-width="2.5" fill="none"/></svg>"##
            ))
            .size(48.0),
            image(gradient_swatch()).width(96.0).radius(12.0),
        ])
        .gap(16.0)
        .align(Align::Center),
        heading("Text editor"),
        container(
            text_editor(&model.notes)
                .placeholder("Write something")
                .size(14),
        )
        .fill_width()
        .height(120.0),
        text(move || format!(
            "Line {}, column {}",
            model.notes.cursor().position.line + 1,
            model.notes.cursor().position.column + 1
        ))
        .size(12),
    ])
    .gap(16.0)
    .fill_width()
    .into()
}

fn collections(model: Model) -> AnyView {
    let listing = rebuild(model.loading, |loading: &bool| {
        Some(if *loading {
            column(views![
                skeleton(240.0, 16.0),
                skeleton(200.0, 16.0),
                skeleton(220.0, 16.0),
            ])
            .gap(10.0)
            .into()
        } else {
            column(views![
                row(views![
                    avatar("Ada Lovelace").size(36.0),
                    text("Ada Lovelace").size(15),
                    badge(text("Inbox").size(14), 3_u32),
                ])
                .gap(12.0)
                .align(Align::Center),
                row(views![
                    avatar("Grace Hopper").size(36.0),
                    text("Grace Hopper").size(15),
                    badge(text("Alerts").size(14), 1_u32),
                ])
                .gap(12.0)
                .align(Align::Center),
            ])
            .gap(12.0)
            .into()
        })
    });

    column(views![
        heading("Tabs"),
        tabs(["Overview", "Activity", "Settings"], model.tab),
        heading("Pull to refresh"),
        container(
            pull_to_refresh(
                container(text("Drag me down").size(14)).center().fill(),
                model.loading
            )
            .on_refresh(move || {
                let loading = model.loading;
                spawn(async move {
                    std::thread::sleep(std::time::Duration::from_millis(900));
                    let _ = loading.set(false);
                });
            }),
        )
        .fill_width(),
        heading("Avatars, badges and skeletons"),
        button(move || if model.loading.get() {
            "Show content"
        } else {
            "Show loading"
        }
        .to_owned())
        .on_press(move || model.loading.update(|loading| *loading = !*loading)),
        listing,
        heading("Carousel"),
        container(carousel(
            (1..=3).map(|page| {
                container(text(format!("Page {page}")).size(24))
                    .center()
                    .fill()
                    .into()
            }),
            model.page,
        ))
        .fill_width(),
        heading("List tiles"),
        list_tile(
            Some(Icon::HOME),
            "Home",
            Some("Where it all starts".to_owned())
        ),
        list_tile(Some(Icon::SETTINGS), "Settings", None),
        heading("Grid"),
        grid(
            3,
            (1..=6).map(|cell| {
                container(text(format!("Cell {cell}")).size(14))
                    .padding(12.0)
                    .into()
            }),
        )
        .gap(8.0),
        heading("Accordion"),
        accordion(
            [
                (
                    "Account",
                    text("Account settings live here.").size(13).into()
                ),
                (
                    "Privacy",
                    text("Privacy settings live here.").size(13).into()
                ),
                ("About", text("Version information.").size(13).into()),
            ],
            model.section,
        ),
    ])
    .gap(16.0)
    .fill_width()
    .into()
}

fn pickers(model: Model) -> AnyView {
    column(views![
        heading("Date picker"),
        text(move || {
            let day = model.day.get();
            format!("Chosen: {} {} {}", day.day, day.month_name(), day.year)
        })
        .size(13),
        date_picker(model.month, model.day).today(Date::new(2026, 8, 13)),
        heading("Time picker"),
        text(move || {
            let time = model.time.get();
            format!("Chosen: {:02}:{:02}", time.hour, time.minute)
        })
        .size(13),
        time_picker(model.time, 15),
    ])
    .gap(16.0)
    .into()
}

fn surfaces(model: Model) -> AnyView {
    column(views![
        heading("Modal surfaces"),
        text("Each of these owns its own animation, scrim, and gestures.").size(13),
        button("Open drawer").on_press(move || {
            let _ = model.drawer.set(true);
        }),
        text("Or swipe in from the left edge.").size(12),
        button("Open bottom sheet").on_press(move || {
            let _ = model.sheet.set(true);
        }),
        text("Drag its handle between detents, or fling it down.").size(12),
        button("Open dialog").on_press(move || {
            let _ = model.dialog.set(true);
        }),
        button("Show snackbar").on_press(notify(model, "Something happened")),
        text("It dismisses itself after four seconds.").size(12),
    ])
    .gap(12.0)
    .into()
}

fn navigation(model: Model) -> AnyView {
    column(views![
        text("Taim").size(24),
        text("Showcase drawer").size(13),
        button("Close").on_press(move || {
            let _ = model.drawer.set(false);
        }),
    ])
    .gap(12.0)
    .padding(8.0)
    .into()
}

/// A small generated picture, so the showcase needs no image files.
fn gradient_swatch() -> taim::view::image_handle::Handle {
    let (width, height) = (96_u32, 48_u32);
    let pixels = (0..height)
        .flat_map(|y| {
            (0..width).flat_map(move |x| {
                [
                    (x * 255 / width) as u8,
                    (y * 255 / height) as u8,
                    200,
                    255,
                ]
            })
        })
        .collect::<Vec<u8>>();

    taim::view::image_handle::Handle::from_rgba(width, height, pixels)
}
