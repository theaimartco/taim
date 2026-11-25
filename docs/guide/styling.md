# Styling

Taim views style themselves from the window's theme, so an app looks
consistent and follows light and dark mode without extra work. You change
the look in three ways: choose the theme, pick a view's variant, and
decorate containers.

## Themes

`Theme::Light`, `Theme::Dark` and `Theme::HighContrast` are built in. Bind
the window's theme to state with `cx.theme`, and it follows that state:

```rust,no_run
use taim::prelude::*;
use taim::Theme;

fn main() -> taim::Result {
    taim::launch("Themes", |cx| {
        let dark = cx.signal(false);
        cx.theme(move || if dark.get() { Theme::Dark } else { Theme::Light });

        column(views![
            text("Appearance").size(22),
            switch("Dark mode", dark),
        ])
        .gap(12.0)
        .padding(24.0)
    })
}
```

Without a `cx.theme` call the window follows the operating system's
appearance.

### A custom theme

`Theme::custom` generates a full palette from six seed colours:

```rust
use taim::theme::palette::Seed;
use taim::{Color, Theme};

fn ocean() -> Theme {
    Theme::custom(
        "Ocean",
        Seed {
            background: Color::from_rgb8(0x0B, 0x1A, 0x2A),
            text: Color::from_rgb8(0xE6, 0xF1, 0xFF),
            primary: Color::from_rgb8(0x3A, 0x9B, 0xFF),
            success: Color::from_rgb8(0x2E, 0xC2, 0x7E),
            warning: Color::from_rgb8(0xF5, 0xB0, 0x41),
            danger: Color::from_rgb8(0xF0, 0x55, 0x55),
        },
    )
}
```

Colours are blended in linear light. The `web-colors` feature restores
browser-style gamma blending for apps that must match a web page exactly;
see `docs/architecture/color-pipeline.md`.

## Variants

Views offer variants rather than free-form style closures:

```rust
use taim::prelude::*;

fn actions() -> impl View {
    row(views![
        button("Save"),
        button("Cancel").secondary(),
        button("More").ghost(),
        button("Delete").danger(),
    ])
    .gap(8.0)
}
```

Text takes `.size`, `.font` and `.color`:

```rust
use taim::prelude::*;
use taim::{Color, Font, font};

fn heading() -> impl View {
    text("Taim")
        .size(32)
        .font(Font {
            weight: font::Weight::Bold,
            ..Font::DEFAULT
        })
        .color(Color::from_rgb8(0x3A, 0x9B, 0xFF))
}
```

## Decorating containers

```rust
use taim::prelude::*;
use taim::{Color, border};

fn card() -> impl View {
    container(text("A card"))
        .padding(16.0)
        .background_themed(Color::WHITE, Color::from_rgb8(0x1E, 0x1E, 0x22))
        .border(
            border::rounded(12.0)
                .width(1.0)
                .color(Color::from_rgba8(0, 0, 0, 0.08)),
        )
}
```

| Method | Effect |
|---|---|
| `.background(color)` | One colour in every theme |
| `.background_themed(light, dark)` | A colour per appearance |
| `.border(Border)` | Built with `border::rounded`, `border::width`, `border::color` |
| `.glass()`, `.frosted_glass()` | Liquid glass material |
| `.reduce_transparency(true)` | An opaque fallback for the glass |

## Liquid glass

`glass(view)` draws its content on a refractive glass panel:

```rust
use taim::prelude::*;

fn floating_bar() -> impl View {
    glass(row(views![button("Play").ghost(), button("Next").ghost()]).gap(8.0))
        .radius(24.0)
        .padding(8.0)
        .interactive()
}
```

Nearby glass panels inside a `glass_group` merge into one shape as they
approach. The [liquid example](../../examples/liquid) and
[Liquid glass](../design/liquid-glass.md) cover the material in depth.

## Reading the theme in a custom view

A custom view's `paint` reads the active palette through
`cx.theme().palette()`, so it follows theme changes like the built-in views
do. See [Canvas drawing](canvas-drawing.md).
