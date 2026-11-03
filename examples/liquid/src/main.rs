//! Liquid glass over living content: a scrolling feed of colourful cards,
//! with a glass header, a glass "now playing" card and a floating glass
//! navigation capsule above it. Scroll to watch the glass lens and frost
//! what passes beneath it.
use taim::prelude::*;
use taim::view::{Constraints, DescribeCx, MeasureCx, PaintCx};
use taim_core::gradient::Linear;
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

        let feed = scroll(
            column(
                std::iter::once(AnyView::from(space().height(84.0)))
                    .chain((0..12).map(|hue| {
                        column(views![
                            Artwork { hue },
                            text(format!("Collection {}", hue + 1)).size(17.0),
                            text("Curated for you").size(13.0),
                        ])
                        .gap(6.0)
                        .into()
                    }))
                    .chain(std::iter::once(AnyView::from(space().height(120.0)))),
            )
            .gap(22.0)
            .padding([0.0, 20.0]),
        );

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
                    container(text("♪").size(22.0))
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

        let bottom = column(views![
            now_playing,
            nav_bar(
                [
                    ("\u{2302}", "Home"),
                    ("\u{2315}", "Search"),
                    ("\u{266B}", "Library"),
                    ("\u{263A}", "Profile"),
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
