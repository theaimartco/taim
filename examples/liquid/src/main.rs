//! Liquid glass over living content: a scrolling feed of colourful cards,
//! with a glass header, a glass "now playing" card, glass transport controls
//! and a floating glass navigation capsule above it.
//!
//! Scroll to watch the glass lens and frost what passes beneath it. Move the
//! pointer over the window and the light on the glass follows it (on a phone,
//! tilt it). Press a control: its rim lights where the finger is, what is
//! under it is magnified, and it swells until it melts into its neighbour;
//! drag and it stretches after you, then wobbles back when let go. The cards
//! cascade in when the window opens, and the feed stretches past its ends.
//! Tap a card: its artwork flies into the collection's page as that page
//! slides in; swipe from the left edge to drag it back.
// No console window behind the app in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
use taim::prelude::*;
use taim::view::{Constraints, DescribeCx, MeasureCx, PaintCx};
use taim_core::gradient::Linear;
use taim_core::time::Duration;
use taim_core::{Border, Color, Radians, Size, semantics};

/// A tall artwork card: a gradient with a few soft shapes.
struct Artwork {
    hue: usize,
}

const PALETTES: [[u32; 3]; 6] = [
    [0xFF5F6D, 0xFFC371, 0x246BFD],
    [0x00C6FF, 0x0072FF, 0x8E2DE2],
    [0x11998E, 0x38EF7D, 0xF7971E],
    [0xFC466B, 0x3F5EFB, 0x00D2FF],
    [0xF7971E, 0xFFD200, 0xFF5F6D],
    [0x8E2DE2, 0x4A00E0, 0x00C9A7],
];

fn rgb(hex: u32) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl View for Artwork {
    fn measure(&mut self, _: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        Size::new(constraints.max.width.min(2_000.0), 220.0)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let [a, b, c] = PALETTES[self.hue % PALETTES.len()];
        cx.fill(
            bounds,
            Linear::new(Radians(0.7 + self.hue as f32 * 0.4))
                .add_stop(0.0, rgb(a))
                .add_stop(0.55, rgb(b))
                .add_stop(1.0, rgb(c)),
            Border::default().rounded(26.0),
        );

        for (index, (x, y, size)) in [(0.12, 0.2, 70.0), (0.62, 0.45, 110.0), (0.35, 0.65, 50.0)]
            .into_iter()
            .enumerate()
        {
            let at = taim_core::Point::new(
                bounds.x + bounds.width * x,
                bounds.y + bounds.height * y - size / 2.0,
            );
            cx.fill(
                taim_core::Rectangle::new(at, Size::new(size, size)),
                Color::from_rgba(1.0, 1.0, 1.0, 0.18 + 0.1 * index as f32),
                Border::default().rounded(size / 2.0),
            );
        }
    }

    fn describe(&self, _: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }
}

fn main() -> taim::Result {
    taim::app("Liquid", |cx| {
        let tab = cx.signal(0_usize);
        let playing = cx.signal(true);

        // Route 0 is the feed; route `n + 1` is collection `n`.
        let pages = Navigator::new(cx, 0_usize);
        let feed = navigator(pages, move |_, route| match *route {
            0 => scroll(
                column(
                    std::iter::once(AnyView::from(space().height(84.0)))
                        .chain((0..12).map(|hue| {
                            // The cards cascade in when the window opens;
                            // tapping one flies its artwork into its page.
                            appear(
                                mouse_area(
                                    column(views![
                                        hero(hue, Artwork { hue }),
                                        text(format!("Collection {}", hue + 1)).size(17.0),
                                        text("Curated for you").size(13.0),
                                    ])
                                    .gap(6.0),
                                )
                                .on_release(move || pages.push(hue + 1)),
                            )
                            .delay(Duration::from_millis(60 * hue.min(5) as u64))
                            .rise(24.0)
                            .into()
                        }))
                        .chain(std::iter::once(AnyView::from(space().height(120.0)))),
                )
                .gap(22.0)
                .padding([0.0, 20.0]),
            )
            .into(),
            collection => {
                let hue = collection - 1;
                scroll(
                    column(views![
                        space().height(84.0),
                        hero(hue, Artwork { hue }),
                        appear(text(format!("Collection {collection}")).size(30.0))
                            .delay(Duration::from_millis(120)),
                        appear(
                            text(
                                "Twelve tracks picked for late nights. Swipe from the \
                                 left edge, or press back, to return.",
                            )
                            .size(15.0),
                        )
                        .delay(Duration::from_millis(180)),
                        appear(button("Back").on_press(move || {
                            let _ = pages.pop();
                        }))
                        .delay(Duration::from_millis(240)),
                    ])
                    .gap(16.0)
                    .padding([0.0, 20.0]),
                )
                .into()
            }
        });

        let header = container(
            glass(
                row(views![
                    text("Discover").size(22.0),
                    space(),
                    icon_button(Icon::SEARCH).ghost().on_press(move || {
                        let _ = tab.set(1);
                    }),
                ])
                .align(Align::Center)
                .fill_width(),
            )
            .padding([10.0, 18.0])
            .radius(22.0)
            .fill_width(),
        )
        .padding([12.0, 16.0])
        .fill_width();

        let now_playing = container(
            glass(
                row(views![
                    container(icon(Icon::PLAY).size(20.0))
                        .width(44.0)
                        .height(44.0)
                        .center()
                        .background(Color::from_rgba(1.0, 1.0, 1.0, 0.35))
                        .border(Border::default().rounded(12.0)),
                    column(views![
                        text("Midnight City").size(15.0),
                        text("M83 · Hurry Up, We're Dreaming").size(12.0),
                    ])
                    .gap(2.0),
                    space(),
                    rebuild(playing, move |on: &bool| {
                        let glyph = if *on { Icon::PAUSE } else { Icon::PLAY };
                        Some(
                            icon_button(glyph)
                                .ghost()
                                .on_press(move || playing.update(|on| *on = !*on))
                                .into(),
                        )
                    }),
                ])
                .gap(12.0)
                .align(Align::Center)
                .fill_width(),
            )
            .padding(12.0)
            .radius(24.0)
            .elevation(1.5)
            .fill_width(),
        )
        .padding([0.0, 16.0])
        .fill_width();

        // Close enough to feel each other at rest, close enough to melt
        // together when one is pressed and swells.
        let control = |glyph: Icon| {
            glass(icon(glyph).size(22.0))
                .padding(15.0)
                .radius(27.0)
                .clear()
                .interactive()
        };
        let transport = container(
            glass_group(
                row(views![
                    control(Icon::CHEVRON_LEFT),
                    control(Icon::PLAY),
                    control(Icon::CHEVRON_RIGHT),
                ])
                .gap(12.0),
            )
            .blend(22.0),
        )
        .center_x()
        .fill_width();

        let bottom = column(views![
            transport,
            now_playing,
            nav_bar(
                [
                    (Icon::HOME, "Home"),
                    (Icon::SEARCH, "Search"),
                    (Icon::FOLDER, "Library"),
                    (Icon::PERSON, "Profile"),
                ],
                tab,
            ),
        ])
        .gap(4.0)
        .fill_width();

        layers(views![
            feed,
            column(views![header, space(), bottom])
                .fill_width()
                .fill_height(),
        ])
    })
    .size(420.0, 860.0)
    .run()
}
