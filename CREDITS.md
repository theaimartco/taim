# Credits

## iced

Taim is a fork of [iced](https://github.com/iced-rs/iced), created by
**Héctor Ramón Jiménez** and the iced contributors.

The fork point is upstream commit
[`56b706fc`](https://github.com/iced-rs/iced/commit/56b706fc) ("Fix lints for
Rust 1.97", 2026-07-09). Every commit before it in this repository's history is
iced's; there are 6,831 of them, against 11 of Taim's own at the time of
writing. Run `git log 56b706fc..HEAD` to see exactly what Taim has added.

**The following originate with iced and remain substantially its work:**

- The application model — `boot` / `update` / `view`, `Task`, `Subscription`
- The `Widget` trait, the element/tree model, and the layout system
- The great majority of `widget/` — every control inherited rather than added
  by Taim, including `button`, `checkbox`, `radio`, `slider`, `text_input`,
  `text_editor`, `scrollable`, `pane_grid`, `canvas`, `markdown`, and the
  layout primitives
- The `wgpu` renderer's quad, triangle, text, and image pipelines, its layer
  and batching model, and the shaders in `wgpu/src/shader/`
- The `tiny_skia` software renderer
- The `winit` shell integration, the `futures` executors, and `program/`
- The theme and palette system

**What Taim adds** (see `git log 56b706fc..HEAD`, and `Capstone.md` §3 for a
verified inventory): the damage and invalidation model, keyed structural
reconciliation, retained layers, the frame scheduler, `taim-gesture`,
`taim-accessibility`, `taim-design`, `taim-motion`, `taim-profiler`,
`taim-highlighter`, `taim-platform-vyom`, the mobile widget set, and the
`taim_bench` / `taim_lab` tooling.

iced is MIT licensed. That licence, and its copyright notice, are reproduced in
`LICENSE` unchanged and continue to govern the inherited code.

## Assets

### Fonts

- **`graphics/fonts/FiraSans-Regular.ttf`** — [Fira Sans], digitized data
  copyright (c) 2012-2015 The Mozilla Foundation and Telefonica S.A.,
  licensed under the **SIL Open Font License 1.1**. The full licence is
  vendored beside it as `graphics/fonts/FiraSans-LICENSE.txt`, as the OFL
  requires. Enabled by default (the `fira-sans` feature) so an Taim
  application renders the same on every machine.

- **`graphics/fonts/Taim-Icons.ttf`** — the icon font. Seven glyphs at
  `U+E800`–`U+E805` and `U+F00C` are **iced's**, inherited with the fork and
  preserved byte for byte because the renderers address them by codepoint
  (`Text::CHECKMARK_ICON` and friends). The 36 glyphs from `U+E900` up were
  authored for this repository and are covered by its MIT licence.

### Icons

`assets/icons/*.svg` and `graphics/fonts/Taim-Icons.ttf` are generated from
`scripts/assets/icon_geometry.py` by `scripts/assets/build-icon-font.py`. No
third-party icon set is vendored, so there is no separate attribution
requirement. `assets/icons/manifest.toml` records each icon's codepoint,
grid, RTL mirroring, and licence.

### TAIM-owned visual identity

`docs/logo.svg`, `docs/logo-no-shadow.svg`, and `docs/graphs/*.svg` are original
TAIM vector assets. The inherited placeholder logo and raster architecture
graphs have been removed.

[Fira Sans]: https://mozilla.github.io/Fira/

## Dependencies

Taim's dependency graph is enumerated by `cargo tree` and its licences are
enforced by `deny.toml` and `.github/workflows/licenses.yml`. `DEPENDENCIES.md`
records the policy for adding one.

Two dependencies deserve specific mention for the work they do here:

- **[cosmic-text](https://github.com/pop-os/cosmic-text)** — text shaping,
  layout, and editing, and with it the rustybuzz/swash/fontdb lineage beneath.
- **[wgpu](https://github.com/gfx-rs/wgpu)** — the GPU abstraction the whole
  renderer is built on.

## Infrastructure

`Cross.toml` uses the neutral `cross-rs` target images. No Iced-owned container
is part of TAIM's current build infrastructure.
