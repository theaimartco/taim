# Liquid glass

![examples/liquid: a glass header, now-playing card and navigation capsule over a scrolling feed](images/liquid-glass.png)

TAIM's glass is a material the GPU renderer shades per pixel. It is not a
blurred, semi-transparent rectangle.

| What you see | How it is made |
|---|---|
| The backdrop shows through, softly | The frame behind the pane is copied, a mip chain is built from it with a tent filter, and the pane reads the level that matches its frost radius, rounded off by a ring of taps. There is no grain or ghosting, at nine samples per pixel. |
| Colours behind the glass glow rather than grey out | A vibrance lift in linear light, scaled by the frost. |
| The rounded edge bends what is behind it | The bevel's normal comes from the pane's own signed distance field; the backdrop is displaced by the refraction through a slab of the material's thickness, scaled to the screen's density. |
| A faint rainbow at the rim | Each colour channel is refracted with its own index (dispersion). |
| A lit rim and a sheen across the top | A hairline highlight border, a specular term from the scene light, and a gradient fading out before the middle. |
| It floats | A soft shadow scaled by `elevation`. |

## Using it

```rust
use taim::prelude::*;

layers(views![
    content,                                   // anything with colour and shape
    container(glass(text("Now playing")).radius(24.0)).align_bottom().padding(16.0),
])
```

- `glass(content)`: frosted liquid glass (`Material::LIQUID`).
- `.clear()`: almost no frost (`Material::LIQUID_CLEAR`), for small
  controls over busy content.
- `.tint(color)`, `.radius(r)`, `.padding(p)`, `.elevation(e)`,
  `.material(m)`, `.fill_width()`, `.fill()`.

Built-in surfaces use the same pane: the bottom navigation capsule
(`nav_bar`), dialogs and bottom sheets. `examples/liquid` puts all of
them over a scrolling feed: `cargo run -p liquid`.

## Rules the views follow

- Glass is drawn in a layer of its own (`View::is_layer`). The renderer
  copies the frame before that layer, so the glass sees everything painted
  earlier. A transmissive quad in the same layer as its content would see
  an empty frame.
- The fill is opaque: it is the tint, and the material's `transmission`
  sets how much backdrop shows through it. A translucent fill would let the
  sharp page show through as well as the frosted one.
- Light glass tints towards white; dark glass is smoked, with less
  transmission, so white text stays readable on it.
- Without a GPU (the tiny-skia renderer), glass is drawn as a solid tinted
  pane, which is also how platforms show glass when "reduce transparency"
  is on.

## Readable text

Secondary text (placeholders, inactive tabs and segments, the quiet days
of a date picker) fades towards the surface it sits on only as far as WCAG
AA allows: 4.5:1 for text and 3:1 for icons. `PaintCx::muted`,
`PaintCx::muted_on(surface)` and `PaintCx::quiet_icon_on(surface)` give
those colours, and a test holds them in every theme. Before this, those
labels read at 1.4 to 2:1.

## Testing

`tests/glass` renders glass on the GPU renderer and checks that the pane
shows the scene behind it, frosts fine detail, and falls back to a solid
pane on the CPU renderer. GPU tests need a Vulkan or GL adapter; on a
machine without a GPU install Mesa's software Vulkan driver
(`apt install mesa-vulkan-drivers`). Without an adapter they print
"skipped: no GPU adapter available" and pass, so check for that line
before treating a GPU test as run.
