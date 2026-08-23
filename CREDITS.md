# Credits

## iced

Aura is a fork of [iced](https://github.com/iced-rs/iced), created by
**Héctor Ramón Jiménez** and the iced contributors.

The fork point is upstream commit
[`56b706fc`](https://github.com/iced-rs/iced/commit/56b706fc) ("Fix lints for
Rust 1.97", 2026-07-09). Every commit before it in this repository's history is
iced's; there are 6,831 of them, against 11 of Aura's own at the time of
writing. Run `git log 56b706fc..HEAD` to see exactly what Aura has added.

**The following originate with iced and remain substantially its work:**

- The application model — `boot` / `update` / `view`, `Task`, `Subscription`
- The `Widget` trait, the element/tree model, and the layout system
- The great majority of `widget/` — every control inherited rather than added
  by Aura, including `button`, `checkbox`, `radio`, `slider`, `text_input`,
  `text_editor`, `scrollable`, `pane_grid`, `canvas`, `markdown`, and the
  layout primitives
- The `wgpu` renderer's quad, triangle, text, and image pipelines, its layer
  and batching model, and the shaders in `wgpu/src/shader/`
- The `tiny_skia` software renderer
- The `winit` shell integration, the `futures` executors, and `program/`
- The theme and palette system

**What Aura adds** (see `git log 56b706fc..HEAD`, and `Capstone.md` §3 for a
verified inventory): the damage and invalidation model, keyed structural
reconciliation, retained layers, the frame scheduler, `aura-gesture`,
`aura-accessibility`, `aura-design`, `aura-motion`, `aura-profiler`,
`aura-highlighter`, `aura-platform-vyom`, the mobile widget set, and the
`aura_bench` / `aura_lab` tooling.

iced is MIT licensed. That licence, and its copyright notice, are reproduced in
`LICENSE` unchanged and continue to govern the inherited code.

## Assets

- `docs/logo.svg`, `docs/logo-no-shadow.svg`, `widget/assets/iced-logo.svg`,
  and `docs/graphs/*.png` are **iced's**, inherited with the fork. Aura has no
  visual identity of its own yet; these are placeholders and must be replaced
  before any public release. See `Capstone.md` F-14.
- `assets/icons/manifest.toml` declares icons that have **not** been imported.
  No SVG in that set ships today, and both entries record
  `license = "TBD-before-asset-import"`. No icon may be added without a
  reviewed licence record.
- `assets/fonts/` contains no font. Text renders with whatever the host
  system provides.

## Dependencies

Aura's dependency graph is enumerated by `cargo tree` and its licences are
enforced by `deny.toml` and `.github/workflows/licenses.yml`. `DEPENDENCIES.md`
records the policy for adding one.

Two dependencies deserve specific mention for the work they do here:

- **[cosmic-text](https://github.com/pop-os/cosmic-text)** — text shaping,
  layout, and editing, and with it the rustybuzz/swash/fontdb lineage beneath.
- **[wgpu](https://github.com/gfx-rs/wgpu)** — the GPU abstraction the whole
  renderer is built on.

## Infrastructure

`Cross.toml` currently cross-compiles using **iced's** published container
images (`ghcr.io/iced-rs/aarch64`, `ghcr.io/iced-rs/armv7`). This is a build
dependency on another project's infrastructure and should be replaced with
Aura's own images before release.
