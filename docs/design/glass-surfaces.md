# Native glass surfaces

`Container` exposes the existing GPU material renderer through two presets:

```rust
container(content).glass().padding(24.0)
container(content).frosted_glass().padding(24.0)
container(content).frosted_glass().reduce_transparency(true)
```

Draw background content first using `layers(views![background, panel])`.
Glass containers start a render layer so the backdrop is captured before
the panel and its children draw. Clear glass uses the material renderer's
refraction, dispersion and edge lighting. Frosted glass adds a bounded
nine-tap tent filter to scatter transmitted detail, including at the centre.
The filter radius is capped at eight physical pixels; it is an approximation,
not a full Gaussian blur or an Apple system API.

The default tint follows the current theme. `.background(color)` customizes it;
`.border(...)` after the preset customizes its rounded edge. Keep text contrast
under review against actual content: glass cannot guarantee readable text on
every moving backdrop. Use reduced transparency for dense or critical content.

`.reduce_transparency(true)` disables transmission and layer capture, forces
an opaque tint, and retains the panel's border and child content. Applications
pass this preference explicitly; it does not automatically read OS settings.
Software rendering draws the theme tint and border without refraction or blur.
The GPU material path requires linear-light compositing; `web-colors` does not
provide the physical material effect.

The Android counter example demonstrates a frosted panel over coloured bands.
The current APK build helper produces development-signed APKs, including when
`--release` selects optimized Rust. That flag does not constitute production
distribution clearance or make the fixture non-debuggable.

Verification covers native view layering, software fallback, opaque preference,
child text painting, actual backdrop softening and existing GPU material
regressions. Physical Android GPU performance, accessibility contrast and thermal
cost still require device acceptance. Each glass layer captures a backdrop;
avoid adding glass independently to every row of a long scrolling list.
