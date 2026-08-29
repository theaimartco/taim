# Aura Future — The Twenty-Five Year Architecture

**Repository:** `vyom-aura` · **Branch:** `capstone-open-findings` · **Head:** `9f9167e4`
**Written:** 2026-08-24 · **Revised:** 2026-08-25 · **Horizon:** 2051
**Toolchain verified:** `cargo test --workspace` → **1,851 passed, 0 failed**, 64 binaries, this tree.

> **Revision, 2026-08-25.** Pillar II is under construction; §Pillar II carries its
> progress, its measurements, and the four bugs it produced. Three audit findings changed
> as a result, and two of them make this document's original assessment *too kind*:
> **Ceiling 3** is worse than described (compositing is not linear at all, by default, and
> no test can observe it), **Ceiling 6** gained a third dormant subsystem, and a new
> **Ceiling 6b** records that one `Engine` fixes one texture format for every pipeline.
> Corrections are marked inline rather than silently applied.

---

## 0. How to read this document

`Capstone.md` answers *"can Aura catch and pass Flutter and React Native?"* It is a
catch-up plan, and it is correct as far as it goes. This document answers a different
question: **"what does a UI framework have to be, structurally, so that Flutter and
React Native read as period pieces?"**

Those are not the same project. The first is won by closing findings. The second is won
by choosing a different set of primitives, and it is mostly lost by people who spend
their years closing findings.

Three rules govern everything below.

1. **Every claim about the current tree was verified against this working copy today.**
   File and line references are real. Where something is machinery without adoption, it
   says so. Where something is unmeasured, it says **unmeasured** rather than guessing.
2. **Every proposal is costed.** A design with no engineering estimate and no proof gate
   is a wish. Each pillar carries "what it costs", "what it buys", and a falsifiable gate.
3. **Priority order is the one you asked for: UI first, then speed, then everything else.**
   Pillars I–V are the image. Pillars VI–IX are the clock. X–XIV are the rest. If you only
   ever execute the first five, you will still have something no competitor has.

---

## 1. The honest framing

You asked how to beat every top framework on everything. The direct answer is that
nobody does, and the frameworks that tried are the ones nobody uses.

Flutter does not win because it is fast. It wins because it has roughly thirty thousand
packages, a hiring pool, Google's name on the box, and a `flutter run` that works on the
first try on six platforms. React Native wins because the JavaScript labour market is
the largest in software history and RN lets you spend it. **Neither of those advantages
is an engine advantage, and neither can be taken from them by writing a better renderer.**

So the framing that actually wins is:

> Do not try to be a better Flutter. Be a category Flutter cannot enter.
>
> Flutter and React Native are *guests* on somebody else's compositor, somebody else's
> colour pipeline, somebody else's input stack, somebody else's power manager. They will
> be guests forever, because portability is their entire product. **Aura runs on VYOM,
> which you own.** That is not a small advantage to be exploited at the margins. It is
> the only structural advantage in this repository that money cannot buy and a bigger
> team cannot copy, and every single pillar below is built to compound it.

The gap that makes Flutter look eighty years old is not "Aura renders a list faster." It
is a user putting a VYOM device next to a flagship Android phone and seeing that one of
them shows *light* and the other shows *pictures of light*, and that one of them responds
to a finger before the other has finished sampling the touch.

That is achievable. It is achievable on hardware that already exists. The rest of this
document is how.

---

# PART I — AUDIT: WHERE AURA STANDS, 2026-08-24

## 2. What was verified

| Command | Result |
|---|---|
| `cargo check --workspace --all-targets` | **exit 0**, 9.18s incremental |
| Rust source, excluding `target/` | **560 files, 158,925 lines** |
| Test functions (`#[test]` + `#[tokio::test]`) | **1,634** |
| Compute shaders (`@compute` / `ComputePipeline` in `wgpu/`) | **0 matches** |
| `RetainedLayerCache` call sites outside its own tests | **0** |
| `Tree::layout_with_cache` call sites outside its own tests | **0** |
| `effects::pool::Pool` call sites outside its own tests | **0** *(added 2026-08-25)* |
| Threads spawned on the frame path | **0** |
| `graphics::color::GAMMA_CORRECTION` in a default build | **`false`** *(added 2026-08-25 — see Ceiling 3)* |
| Configurations of the integration suite in which it is `true` | **none** *(added 2026-08-25)* |

Crate sizes, measured:

| Crate | Files | Lines | Role |
|---|---:|---:|---|
| `widget` | 96 | 52,273 | 75 public constructors |
| `core` | 94 | 19,686 | types, layout, text, input, semantics |
| `wgpu` | 40 | 11,493 | GPU backend |
| `tools` | 34 | 8,489 | bench harness, Aura Lab, profiler CLI |
| `runtime` | 23 | 8,012 | reconcile, schedule, hit-test |
| `graphics` | 31 | 7,881 | damage, layers, display list |
| `aura-accessibility` | 15 | 5,203 | semantic tree + AT-SPI bridge |
| `winit` | 7 | 4,314 | shell, event loop |
| `aura-gesture` | 12 | 3,844 | arena and recognisers |
| `tiny_skia` | 10 | 3,326 | CPU renderer, golden backend |
| `aura-design` | 31 | 2,501 | tokens + 17 component styles |
| `aura-platform-vyom` | 13 | 1,760 | Wayland/Smithay backend |
| `aura-motion` | 11 | 1,081 | springs, decay, timelines |

## 3. What is genuinely strong

These are not compliments. They are load-bearing assets that the twenty-five year plan
is built on top of, and each one would take a competitor a year to reproduce.

**3.1 The damage system is real and on the live path.** `graphics/src/damage.rs` is a
`SmallVec<[Rectangle; 8]>` with a hard `MAX_RECTS = 32` and an area-fraction collapse to
full redraw. The wgpu renderer scissors the render pass to the damaged region and loads
rather than clears. Critically, `Renderer::accumulate_damage` redraws the union of the
last *N* frames where *N* is swapchain depth, because a swapchain hands back a buffer
holding what was drawn *N* frames ago and wgpu reports no buffer age. Six tests pin the
fallback cases. **Most frameworks get this wrong or skip it. This one is correct.**

**3.2 The effect pipeline renders pixels.** `wgpu/src/effects/pipeline.rs` is 1,050 lines
behind `shader/effect.wgsl`: separable Gaussian blur, backdrop blur, 4×5 colour matrix,
Rec. 709 saturation, noise, mask. Two bugs were found *only* because someone rendered the
result and looked at it — a shared uniform slot that made both blur passes run vertically,
and a mask that blended instead of replacing. That is a team that has learned the right
lesson about numeric tests.

**3.3 The adaptive quality policy is a designed system, not a toggle.**
`graphics/src/quality.rs` defines four tiers (Ultra / Balanced / Efficient / Recovery),
each a `const fn` returning a `Policy` with a max blur radius, a max effect area, a
texture budget in bytes, and per-feature permission for backdrop and noise. This is the
right shape for thermal and battery response, and it is already renderer-neutral.

**3.4 The VYOM backend has the hard parts scaffolded.** `capabilities.rs` fails closed on
unadvertised globals. `frame_clock.rs` correlates submission against presentation feedback
with a bounded in-flight queue and computes target error. `power.rs` carries visibility,
suspend, battery fraction, and a four-level thermal enum. `SurfaceLifecycle::clip_damage`
knows that an out-of-surface damage rectangle is a protocol error that kills the
connection, not a request the compositor ignores. **Nobody scaffolds this correctly by
accident.**

**3.5 The semantic tree is built every frame regardless of whether anyone is listening,
and it now reaches a real bus.** `aura-accessibility/tests/atspi_bus.rs` serves the tree
over a live D-Bus session and queries it as a screen reader would, including that a
retired node stops answering. This is the single most under-valued asset in the
repository, and Pillar X is built entirely on it.

**3.6 The visual truth harness exists and has already caught framework-wide bugs.** The
golden suite renders through the real `UserInterface` pipeline on `tiny-skia`, on the CPU,
with no GPU and no driver — so a golden reproduces on any machine. It found that the
bundled icon font declared the family `Iced-Icons` while both renderers looked up
`Aura-Icons`, meaning every checkmark and dropdown arrow in the framework had been
resolving to a missing glyph while 1,705 tests passed.

**3.7 The discipline is unusual.** `unsafe_code = "deny"` and `missing_docs = "deny"` at
the workspace level, an explicit allow-list of files permitted to opt out, a 260-dependency
budget, and a binary-size budget enforced in CI. Frameworks that ship at this scale
almost never hold these lines.

## 4. The nine ceilings

These are not bugs. Every one of them is a *correct implementation of an architecture
that caps how good the result can get*, and no amount of optimisation inside the current
shape removes them. This list is the real content of the audit.

---

### Ceiling 1 — Every rebuild reconstructs and re-lays-out the entire world

`winit/src/lib.rs:1320-1338` calls `program.view(id)`, which allocates a fresh boxed
widget tree. That tree goes to `UserInterface::build` (`runtime/src/user_interface.rs:111`),
which does three things in sequence:

```rust
state.diff(root.as_widget_mut());                        // walk the whole tree
state.mark_dirty(widget::tree::DirtyFlags::PAINT);       // ← every node, unconditionally
let base = root.as_widget_mut().layout(                  // from the root, full limits
    &mut state, renderer, &layout::Limits::new(Size::ZERO, bounds),
);
```

Toggling one checkbox in a 10,000-row list allocates 10,000 widgets, diffs 10,000 nodes,
marks 10,000 nodes dirty for paint, and re-lays-out 10,000 nodes. The measured cost of
this is visible in the committed baselines and nowhere better than in `virtual_list`,
where **`update` p95 is 30.08 µs against a `draw` p95 of 32.42 µs** — more than half the
frame is spent deciding what to draw rather than drawing it. That ratio is the signature
of this ceiling, and it is why `virtual_list` is 8.8× the cheapest scene.

Flutter has the same disease in a milder form: it rebuilds subtrees rather than the
world. React Native has it worse. **This is the single largest available win in the
repository and it is worth more than every renderer optimisation combined.** See Pillar VII.

---

### Ceiling 2 — The frame pipeline is single-threaded

Grepping the whole engine for thread spawns returns four sites, and not one is on the
frame path:

- `wgpu/src/image/cache.rs:424` — image decode worker (correct, and predates the audit)
- `winit/src/window.rs:84` — AT-SPI bridge
- `winit/src/clipboard.rs:60,107` — clipboard
- `runtime/src/task.rs:480,502` — application task executor

Layout, paint recording, primitive batching, and GPU submission all happen on one thread,
in sequence. Flutter has had a separate raster thread since 2015 and Impeller compiles
shaders concurrently. **Aura is competing against a two-thread pipeline with one thread
and winning on frame cost anyway** — which tells you exactly how much is left on the
table. See Pillar VIII.

---

### Ceiling 3 — The colour pipeline is 8-bit sRGB with no colour management

> **RESOLVED 2026-08-25.** Linear-light compositing is now the production default on
> both backends, `web-colors` is opt-in legacy, and `--no-default-features` genuinely
> disables defaults. See `docs/architecture/color-pipeline.md` for the live contract and
> `docs/testing/golden-linear-recapture.md` for the 67 goldens this moved. The correction
> below is kept because it records what was wrong and how it stayed invisible.
>
> **Corrected 2026-08-25, and it is worse than this section originally said.** The
> paragraph below described the pipeline as compositing in sRGB. It does not composite in
> linear sRGB at all. `web-colors` is a **default feature** of the `aura` crate, and
> Cargo.toml describes it in its own words as *"broken 'sRGB linear' blending to reproduce
> color management of the Web"*. With it on, `graphics::color::GAMMA_CORRECTION` is
> `false`, `pack()` hands the GPU gamma-encoded components, and every blend in the
> framework operates on them directly. There is no linear light anywhere in the default
> pipeline.
>
> **And no test can currently observe this.** The root crate lists *itself* as a
> dev-dependency with default features on, so `cargo test --no-default-features` does not
> reach the feature: `GAMMA_CORRECTION` is `false` in every configuration of the
> integration suite. Verified by probe, both ways. No golden in this repository has ever
> been rendered through linear compositing.
>
> This makes Ceiling 3 a *prerequisite* rather than a parallel workstream. Pillar II's
> arithmetic is built and proven, but a luminance above diffuse white has nothing to
> scale until blending is linear, because scaling a gamma-encoded component changes hue
> rather than brightness. Flipping the default moves every blended pixel in the framework
> — which is the point, since the current blending is mathematically wrong — and requires
> re-capturing all 40 committed goldens.

`graphics/src/color.rs` is the whole story: a `Packed([f32; 4])`, and `pack()` either
calls `color.into_linear()` or passes sRGB components straight through under the
`web-colors` feature. There is no white point, no primaries, no transfer function, no
peak-luminance concept, no tone mapping, and no path to a display that can do more than
sRGB.

Every panel worth shipping on in 2026 does Display P3 and 1,000+ nits. In 2051 the idea
of a UI framework that composites in sRGB will read the way a 256-colour palette reads
now. **This is the highest-leverage UI change in this document**, because it is the one
a person sees from across a room. See Pillars II and III.

---

### Ceiling 4 — Zero compute shaders; rasterisation is fixed-function with MSAA

`grep -rn "@compute\|ComputePipeline" wgpu/` returns **nothing**. Every pipeline is a
vertex/fragment rasterisation pipeline, and antialiasing is
`graphics/src/antialiasing.rs`: MSAA at 2/4/8/16 samples.

MSAA costs bandwidth linearly in sample count, does nothing for interior shading, and
still produces conflation artifacts where two shapes meet. The modern answer — Vello,
Pathfinder, Forma — is a compute pipeline that computes *analytic* area coverage per
pixel, at a cost that is nearly independent of path complexity. Flutter's Impeller is
still tessellation-and-stencil. **Nobody in the mainstream UI world ships a compute
rasteriser. There is an open lane here and it is wide.** See Pillar I.

---

### Ceiling 5 — Text is CPU-rasterised into an atlas

`wgpu/src/text.rs` sits on `cosmic-text 0.19` and `cryoglyph` (a glyphon fork). Glyphs
are shaped and rasterised on the CPU, uploaded to an atlas, and drawn as textured quads.

That architecture caps five things at once, permanently:

1. Scale is not free — a zoom re-rasterises, and mid-animation it either pops or blurs.
2. Variable-font axes cannot animate per frame; each weight is a new raster.
3. 3D-transformed or perspective text is a stretched bitmap.
4. Multilingual and emoji-heavy content thrashes the atlas.
5. Glyph rasterisation burns CPU **on the single frame thread** (see Ceiling 2), which is
   exactly why `text_edit` p95 is 23.38 µs and `text_heavy` holds the peak RSS at 196.7 MiB.

See Pillar IV.

---

### Ceiling 6 — Incremental layout, retained layers, and the effect budget are machinery without adoption

**Three** substantial subsystems are fully built, fully tested, and **called by nothing**:

- `Tree::layout_with_cache` (`core/src/widget/tree.rs:289`) — constraint/revision-based
  layout reuse. Call sites outside its own tests: **zero**. `widget/src/row.rs:207`
  mentions it in a comment.
- `RetainedLayerCache` (`graphics/src/retained_layer.rs`, 682 lines, LRU + budget +
  telemetry) — referenced **only** by `tests/graphics/retained_layers.rs`. The renderer
  holds three single-slot `Option`s instead (`wgpu/src/lib.rs:109-111`).
- `effects::pool::Pool` (`wgpu/src/effects/pool.rs`) — the bounded intermediate-texture
  accounting, with its careful `used`/`reserved` model and its budget eviction.
  **Found 2026-08-25:** referenced only by its own unit tests and
  `tests/gpu/effect_budget.rs`. The effect pipeline allocates intermediates directly and
  never consults it, which means `graphics::quality::Tier`'s `texture_budget_bytes` — the
  thermal and battery response in §3.3 — **is not enforced by anything on the live path.**
  Its `Descriptor::bytes_per_pixel` is only ever set in tests, always to `4`, so it has
  also never seen a half-float intermediate.

And retention cannot be adopted incrementally, because `recording_retained_frame` is a
single `Option` and `end_retained_layer` panics without a matching start —
**the recorder has to become a stack before any widget can call it.**

This is a specific and recurring failure mode in this repository: build the mechanism,
test the mechanism, never wire the mechanism. It is worth naming explicitly, because the
plan below adds a lot of mechanism — and because each instance costs twice: once to build
the thing, and again when a later reader assumes a budget or a cache is doing work that
nothing has ever asked it to do.

> **Updated 2026-08-25.** Two of the four instances are now **LIVE**:
>
> - **`effects::pool::Pool`** — synced to `quality::Tier`'s budget once per frame in
>   `Renderer::draw`, and effect intermediates are admitted through it. Over budget, the
>   pipeline halves the intermediate and retries, then refuses below 64px. Cost is now
>   format-aware (`estimated_texture_bytes`), so a half-float intermediate prices at
>   exactly twice an 8-bit one — the old hard-coded `bytes_per_pixel = 4` under-priced
>   every HDR frame by half. Proven by `tests/gpu/quality_budget.rs`.
> - **`PipelineCache`** — retained and resolving per target format. See Ceiling 6b.
>
> `RetainedLayerCache` and `Tree::layout_with_cache` remain **TEST-ONLY**, unchanged.

---

### Ceiling 6b — One `Engine`, one texture format, every pipeline

**Found 2026-08-25, while putting the display transform on the present path.**

> **RESOLVED 2026-08-25.** `Engine` no longer owns pipelines or a binding format. It
> retains the format-keyed `PipelineCache` — which already existed, was already keyed by
> `(kind, format, antialiasing, backend)`, and was previously built inside `Engine::new`,
> used once, and dropped — and `Engine::pipelines(format)` resolves per target.
> `Renderer::use_format` re-resolves from `target_texture.format()` once per frame (one
> integer compare in the steady state) and rebuilds only what depends on the format: the
> pipelines, the triangle MSAA state, and the text glyph viewport. Nothing device-global
> is touched, so a window that changes format keeps its images, caches and primitive
> storage.
>
> Consequence 1 is gone: `present_hdr` now always uses a half-float intermediate
> regardless of what the surface or engine prefer, so HDR no longer requires an engine
> that was *created* half-float. Consequences 2 and 3 are unblocked. Proven by
> `tests/gpu/target_formats.rs`: one engine, three formats, in any order, repeatedly.

`Engine::new` takes a `wgpu::TextureFormat` and builds the quad, text, triangle, image,
and effect pipelines against it, once. Nothing can later draw into a target of a
different format: attempting it is not a wrong colour but a render-pass validation
error, reported as *"render pipeline targets are incompatible with render pass"* — a
message that says nothing about the actual cause.

Three consequences, each of which shapes a pillar below:

1. **Any intermediate render target must be in the engine's format.** Pillar II's
   half-float frame works only because a `PreferHdr` surface makes the engine half-float
   too. The coupling is real and silent, and it is now asserted at
   `Renderer::present_hdr`'s entry rather than discovered inside a draw call.
2. **A window cannot change format without rebuilding the engine.** Dragging a window
   between an SDR and an HDR monitor is exactly that. Pillar II's display transform
   handles the *panel* changing; it does not handle the *surface format* changing.
3. **Pillar I's compute rasteriser inherits the constraint** — its G-buffer (Pillar III)
   is several targets in several formats, so the one-format assumption has to be
   dismantled before that work rather than during it.

This is cheap to fix now (a format-keyed pipeline cache, which
`wgpu/src/pipeline_cache.rs` already has the shape for) and expensive to fix later.

---

### Ceiling 7 — Nothing presents through VYOM

`aura-platform-vyom` creates surfaces, negotiates capabilities, handles configure, and
can submit `wl_surface.damage_buffer`. It **attaches no buffers**. Nothing has ever
appeared on a screen through this backend.

Every strategic advantage in Section 1 — the entire moat — is downstream of this one
gap. Until a VYOM surface presents a real frame, the compositor advantage is a document,
not a product. **This is the highest-priority item in the repository and it is not close.**

---

### Ceiling 8 — Three of the four numbers that decide adoption are unmeasured

The committed baselines are excellent and honest: worst p95 **57.59 µs** (`virtual_list`),
worst p99 **74.40 µs**, **0.000%** missed frames across 13 scenes, net allocation delta of
**2** over five runs of ten thousand rows. The methodology is reproducible and the README
correctly refuses to quote anything else.

But frame cost is the axis where a GC-free native framework is *expected* to win, and it
is not the axis buyers decide on. The three that are:

| Axis | Aura | Flutter | Status |
|---|---|---|---|
| Cold start to first pixel | **unmeasured** | ~300–800 ms typical | no harness exists |
| Steady-state app memory | **unmeasured** (196.7 MiB is the *harness*) | 60–120 MiB typical | no harness exists |
| Energy per hour of use | **unmeasured** | measurable | no harness exists |
| Frame cost p95 | 57.59 µs | ~1–4 ms typical | **won, and by a lot** |

Binary size *is* measured — 20.53 MiB unstripped, **15.76 MiB stripped** for
`mobile_shell`, against a 22 MiB budget. That number is not competitive with the story
this document tells and Pillar XIII addresses it.

And per `benchmarks/competitors/README.md`, **no Flutter or React Native comparison exists
in this repository.** The rules for a fair one are written down; no result has been captured.

---

### Ceiling 9 — The ecosystem gap is the one that actually decides this

75 public constructors. 36 icons. Not yet built: calendar range selection, tree view,
pagination, breadcrumb, rating, video, web view, map.

Flutter ships Material 3 *and* Cupertino and has ~30,000 pub.dev packages. React Native
has all of npm. **No engine advantage closes a 30,000-package gap, and any plan that
pretends otherwise is a plan to lose slowly.** The strategic response is not to race —
it is Section 15.

---

## 5. Scorecard, today

Ratings are against Flutter 3.x and React Native 0.7x as of 2026-08.

| Axis | Aura | Flutter | RN | Note |
|---|:--:|:--:|:--:|---|
| Frame cost, framework-side | **A** | B | C | 224× margin at 60 Hz, worst p99 |
| Damage / partial redraw | **A−** | C | D | correct swapchain-depth accumulation |
| Memory safety | **A** | B | C | `unsafe_code = "deny"` workspace-wide |
| Test discipline | **A−** | B+ | B | 1,634 tests, golden suite on CPU |
| Accessibility architecture | **A−** | B | C+ | tree built unconditionally, live bus |
| Rasterisation quality | C | **B+** | B | MSAA vs Impeller vs Skia |
| Colour / HDR | **D** | C+ | C | sRGB only, no management |
| Text | C+ | **A−** | B+ | CPU atlas vs full-featured stacks |
| Reactivity cost | **D** | B | C+ | full rebuild + full relayout |
| Threading | **D** | A | B | one thread vs raster thread |
| Cold start | **?** | B | C | unmeasured |
| Binary size | C− | B | B− | 15.76 MiB stripped |
| Platform reach | **F** | A | A | nothing presents anywhere yet |
| Ecosystem | **F** | A | A | 75 constructors vs 30,000 packages |
| Tooling / DX | C | **A** | B+ | no state-preserving hot reload |

**Read that honestly:** the engine core is already better than both competitors on the
things it has actually built, and the product is nowhere, because it does not run on a
screen and there is nothing to build with. The plan below fixes the second problem first
and then makes the first one unassailable.

---

# PART II — THE 2051 TARGET

## 6. What the hardware will be

Design against the machine you will ship on, not the one on your desk. Extrapolating
from committed roadmaps and physics rather than from marketing:

| | 2026 | 2051 |
|---|---|---|
| Panel peak luminance | 1,000–1,600 nits | 10,000+ nits, per-pixel emissive standard |
| Gamut | Display P3 common | Rec. 2020+ standard; spectral rendering viable |
| Refresh | 120 Hz, LTPO 1–120 | 480–1,000 Hz, continuously variable, per-region |
| Resolution | ~500 ppi phone | retinal-limit; **foveated rendering is mandatory, not an optimisation** |
| Form | rectangle | rectangle + varifocal HMD + waveguide glasses + volumetric |
| Compute | 8–16 CPU cores, iGPU | 64+ heterogeneous cores, unified memory, no PCIe copy |
| NPU | 40 TOPS, idle during UI | 1,000+ TOPS, idle during UI |
| Input | touch @ 120 Hz | gaze @ 1 kHz, hand tracking, sub-ms surface sensing |

Four consequences follow, and every one of them is fatal to a framework built on the
2015 assumptions that Flutter and React Native are built on:

1. **The pixel grid stops being the unit of design.** At retinal density with foveation,
   what you author is geometry and what gets shaded is a variable-rate, gaze-dependent
   sample set. A framework whose layout system speaks in integer logical pixels and whose
   text is a bitmap atlas cannot follow.
2. **Luminance becomes a design parameter.** When a panel does 10,000 nits, "white" is a
   meaningless instruction. Designers will specify *nits*, and a compositor that cannot
   carry a scene-referred value will clip everything a designer intends.
3. **Latency becomes the only performance metric anyone quotes.** At 1,000 Hz the frame
   budget is 1 ms and nobody counts frames any more; they count milliseconds from finger
   to photon. A pipeline with three frames of buffering is disqualified regardless of how
   fast each frame is.
4. **UI stops being one presentation.** The same interface has to render to a screen, to
   a pair of waveguides at 60 cm focal depth, to a voice channel, and to a braille cell —
   from one description, not four codebases.

## 7. The thesis, in one sentence

> **Aura wins by treating a user interface as a scene of physically-grounded, light-emitting
> materials, resolved analytically on GPU compute, composited in scene-referred HDR,
> updated only where state actually changed, and presented in phase with a compositor it
> owns — so that the number a user feels is finger-to-photon latency and the thing a user
> sees is light.**

Every pillar below is a clause of that sentence.

---

# PART III — THE PILLARS

## Priority I — THE IMAGE

---

## Pillar I — A compute rasteriser (`aura-raster`)

**Ceiling addressed:** 4. **Horizon:** H2. **Cost:** 6–9 months, 1–2 engineers.

### The design

Replace vertex/fragment quad rendering plus MSAA with a Vello-class compute pipeline.
Four stages, all on GPU, all in one command encoder:

```
stage 1  encode      CPU or GPU writes a flat scene: path segments, draw objects,
                     clip stack, material ids. Append-only, delta-uploadable (Pillar VII).
stage 2  path_coarse compute: transform segments, bin them into 16×16 tiles.
                     One workgroup per path chunk. Output: per-tile segment lists.
stage 3  coarse      compute: walk the draw order per tile, resolve clips and opacity
                     groups, emit a per-tile command list. Cull anything the damage
                     region (graphics/src/damage.rs) does not touch — the two systems
                     compose exactly, which is why this is the right fit here.
stage 4  fine        compute: one workgroup per tile. Evaluate the command list,
                     accumulate *analytic* area coverage per pixel, blend in linear
                     HDR (Pillar II), write the G-buffer (Pillar III).
```

New crate `aura-raster`, sitting under `aura_graphics`, behind a
`renderer::Settings::rasteriser` enum so the existing quad/triangle path stays as the
fallback for devices below the compute floor. `tiny_skia` remains the golden backend
untouched — which is the whole reason the golden suite was worth building.

### What it buys

| | today | with `aura-raster` |
|---|---|---|
| AA quality | MSAA 4×, conflation artifacts at seams | exact analytic coverage, no conflation |
| Cost vs path complexity | linear in triangles | ~flat; complexity moves to the tile list |
| Rounded rect + shadow + border | 3 pipelines, 3 draws | one tile command list, one pass |
| Arbitrary clip paths | stencil, expensive | free, it is a command in the list |
| Zoom | re-tessellate | resolution-independent by construction |
| Bandwidth | 4× samples per pixel | 1× |

### The proof gate

A scene of 5,000 overlapping rounded rectangles with per-element clip paths, rendered
both ways, byte-compared for coverage error against a CPU reference, and benchmarked. Gate:
**analytic path within 1/255 of reference on 99.9% of pixels, and total frame cost no worse
than the MSAA path at 4× the geometry count.**

### The trap

Do not start here. A compute rasteriser is the most fun item in this document and the
second-most likely to consume a year and ship nothing, because it is a rewrite of the
only part of the system that currently works. It comes *after* Pillar VII, because it is
worth far less on a pipeline that re-uploads the whole scene every frame.

---

## Pillar II — Scene-referred HDR compositing

**Ceiling addressed:** 3. **Horizon:** H1. **Cost:** 6–10 weeks. **This is the highest
value-per-week item in the entire document.**

### Progress — 2026-08-25

Started, and further along than the estimate suggested, because of one favourable
discovery and one blocking one.

**The favourable one.** The fragment shaders already emit **linear light** and rely on an
sRGB surface to apply the transfer in hardware. Pointing them at a half-float target
therefore needs no shader change at all — it simply stops discarding range they were
always producing. That removed most of the renderer work this pillar had budgeted for.

**The blocking one.** See Ceiling 3's correction. `web-colors` is a default feature, it
disables linear compositing entirely, and the integration suite cannot switch it off.
**Everything below is built and proven; none of it does anything in a default build.**

| Stage | State | Evidence |
|---|---|---|
| Colorimetry — primaries, white points, PQ/HLG/sRGB transfers, Bradford adaptation | done | `core/src/color/space.rs`, 22 tests against published reference matrices |
| `Radiance` — the unbounded scene-referred value | done | `core/src/color/radiance.rs`, 11 tests |
| HDR surface selection — `ColorPolicy` | done | `wgpu/src/window/compositor.rs`, 7 tests on the rule table |
| Display transform — BT.2390 EETF, gamut mapping | done | `core/src/color/display.rs`, 22 tests |
| WGSL mirror + CPU/GPU conformance | done | `wgpu/src/shader/display.wgsl`, `tests/gpu/display_transform.rs`, 5 GPU tests |
| Transform on the live present path | done | `Renderer::present_hdr`, 3 GPU tests |
| Authorable luminance — `Quad::luminance`, `LuminanceTokens` | done, **inert** | 9 tests; blocked by `web-colors` |
| f16 effect intermediates | satisfied by construction | the effect pipeline takes the engine's format; unverifiable until linear |
| Panel negotiation (`wp_color_management_v1`) | not started | needs Linux and a compositor |

**Measured, on a real GPU:** diffuse white reaches an HDR panel at 203 nits and a
1,000-nit highlight reaches it above 900 — the gate, in the signal. And the HDR present
path reproduces the standard path to within **0.0019** per component on content with no
HDR in it, which is what makes it safe to have on the live path before anything produces
HDR.

**Four real bugs, each caught by a test written to catch it, and all four the same
mistake.** Every one confused *"the brightest the panel can do"* with *"what 1.0 means"*:

1. Defaulting the tone mapper's source peak to the PQ container's 10,000 nits put
   ordinary white above BT.2390's knee and rolled it off — **encoding diffuse white at
   0.78 instead of 1.0, dimming every interface on every SDR monitor by 22%** to leave
   headroom for content that was not in the scene.
2. Encoding relative to the panel's peak rather than its reference white put diffuse
   white at `203/1600` on an extended (macOS EDR) surface — **the whole interface at
   12.7% brightness.** Worth dwelling on: *all 53 existing tests still passed*, because
   not one of them asked what a panel actually received. The fix added that invariant.
3. HLG decode returned **infinity** outside its domain — it is an exponential, and one
   infinity spreads through every blend it touches for the rest of the frame.
4. Resolving onto an sRGB target would have encoded the frame **twice**, since the
   hardware applies the transfer too. That one is the most dangerous of the four because
   it produces a *plausible* image: black and white stay exactly right and only midtones
   move, so it reads as a badly-tuned theme rather than a bug.

**The lesson worth keeping.** Three of these four were invisible to a numeric test suite
and visible only to a test that asked *what does the panel receive*. That is the same
lesson §3.2 records the effect pipeline learning, and §3.6 records the icon font
teaching. It keeps costing this repository the same way.

**What unblocks the rest:** the `web-colors` decision. Flipping it moves every blended
pixel in the framework — which is the point, since the present blending is
mathematically wrong — and requires re-capturing all 40 committed goldens. It is a
one-line change; the work is in reviewing what moved.

### The design

Replace `Color` as "four sRGB floats" with a colour *value* that knows what it means.

```rust
// core/src/color.rs
pub struct Color {
    /// Scene-referred radiance, Rec. 2020 linear primaries.
    /// 1.0 == SDR diffuse white == 203 nits (ITU-R BT.2408 reference).
    /// Values above 1.0 are legal, expected, and are the whole point.
    pub components: [f32; 3],
    pub alpha: f32,
}

pub struct ColorSpace {
    pub primaries: Primaries,        // sRGB | DisplayP3 | Rec2020 | ACEScg
    pub transfer: Transfer,          // sRGB | Linear | PQ | HLG
    pub white_point: WhitePoint,     // D65 | D50 | adaptive
    pub peak_nits: f32,
}
```

Then, in order:

1. **Every intermediate surface becomes `Rgba16Float`.** The effect pool
   (`wgpu/src/effects/pool.rs`) and the G-buffer allocate half-float. Cost: 2× the
   bandwidth of `Rgba8Unorm` on intermediates only. On the measured scenes this is
   noise; `blur_layers` p95 is 15.95 µs against a 16.67 ms budget.
2. **Compositing happens in Rec. 2020 linear, always.** Delete the `web-colors` feature
   from the compositing path. Keep it as an *input* interpretation for authored colours,
   which is what it was actually for.
3. **A display-transform pass at the end**, and only at the end: gamut-map to the panel's
   primaries, then tone-map via BT.2390 EETF to its peak luminance, then encode to its
   transfer function. One extra full-screen pass, or folded into the existing blit.
   Under the scissor, so it costs damage area, not screen area.
4. **The token system gains luminance.** `aura-design/src/tokens/color.rs` gets a
   `nits` field. `elevation.rs` — which today means a shadow — becomes a *physical*
   quantity: how far a surface floats, and therefore how much light it catches.
5. **VYOM negotiates the display.** `aura-platform-vyom` learns the
   `wp_color_management_v1` protocol and reports the panel's real primaries and peak
   luminance into `ColorSpace`, so the display transform targets the actual panel rather
   than an assumption. **Flutter cannot do this. It is a guest.**

### What it buys, in the terms a person on a sofa would use

A notification arrives and the accent stripe on it is *brighter than the white of the
page* — not lighter, brighter, physically emitting more light. A media control surface
has a specular highlight that actually blows out. A dark-mode UI has true black next to a
1,200-nit highlight. **Every competitor's UI, side by side, looks like a photograph of a
UI.** No amount of Flutter engineering closes this, because Flutter composites in 8-bit
and hands the result to an OS that can only tone-map what it was given.

### The proof gate

Render the same scene through Aura and through Flutter on the same HDR panel. Photograph
both with a fixed-exposure camera. **Gate: the Aura highlight measures above 1,000 nits
on a luminance meter and the Flutter one measures at panel SDR white.** This is the
single most persuasive demo available to this project and it costs ten weeks.

---

## Pillar III — Materials, not decorations

**Ceiling addressed:** 3, 4. **Horizon:** H2. **Cost:** 4–6 months. Requires I and II.

> **Foundation started 2026-08-25.** The physics exists, is tested, and the CPU and
> GPU implementations are pinned to each other — but **nothing shades a material yet**.
>
> Built: `core/src/material.rs` (Fresnel/Schlick, GGX, slab refraction, dispersion,
> `F0` derived from IOR), `wgpu/src/shader/material.wgsl` as its mirror,
> `tests/gpu/material_shading.rs` proving they agree across every material at every
> viewing angle on real hardware, and `aura-design`'s `MaterialTokens` for the semantic
> roles. 25 tests. See `docs/architecture/material-pipeline.md`.
>
> Not built: no primitive carries a `Material`, there is no lighting pass, and nothing
> calls `specular()` during a frame. A widget cannot ask for glass.
>
> **The dependency on Pillar I is narrower than this section assumed.** The full
> deferred G-buffer does need a compute rasteriser, for coverage and gradients over
> arbitrary paths. But `shader/quad.wgsl` already computes `rounded_box_sdf`, and an
> SDF's gradient *is* a normal — so a lit, bevelled, refracting rounded rectangle needs
> no compute stage and no extra attachment. `sdf_normal()` is written and unused. That
> is the intended next step, and it is forward shading rather than deferred.

### The problem with every current "glass" UI

`wgpu/src/effects/` implements backdrop blur, noise, and mask. That is the state of the
art across the whole industry — Apple's Liquid Glass, Windows Mica, Flutter's
`BackdropFilter` are all the same trick: **sample what is behind, blur it, tint it,
composite**. It is a screen-space approximation with no thickness, no refraction, no
dispersion, no correct edge behaviour, and no consistent light source. It looks like glass
until you move something behind it.

### The design: a deferred G-buffer for UI

UI geometry is trivially cheap compared to a game — a few thousand quads and paths. So
you can afford what games spend their whole budget on. The `fine` stage of Pillar I
writes, instead of a colour:

```
RT0  Rgba16Float   .rgb = emissive radiance (Rec.2020 linear, Pillar II)
                   .a   = analytic coverage
RT1  Rgba16Float   .rg  = octahedral normal, derived from the SDF gradient of the
                          element's own outline — a rounded rect gets a real bevel
                   .b   = roughness
                   .a   = thickness in millimetres
RT2  R8Uint        material id → indexes a GPU material table
```

≈12 bytes/pixel. At 1440p that is **40 MB**, and it is only allocated over the damage
region. Then one lighting pass:

```wgsl
// aura-raster/shader/light.wgsl  (sketch)
let n     = decode_octahedral(rt1.rg);
let v     = view_dir(uv);
let mat   = materials[rt2];

// refraction through a slab of known thickness — this is the part
// backdrop-blur cannot do, and the part a person notices immediately
let offset = refract(v, n, mat.ior) * rt1.a * SCREEN_MM;
var behind = sample_backdrop(uv + offset);

// wavelength-dependent IOR: real glass separates colour at its edges
behind.r = sample_backdrop(uv + offset * mat.dispersion.r).r;
behind.b = sample_backdrop(uv + offset * mat.dispersion.b).b;

// one scene light, defined by the design system, not by the app
let spec  = ggx(n, v, scene_light.dir, rt1.b);
let fres  = schlick(dot(n, v), mat.f0);

out = mix(behind, scene_light.color * spec, fres) + rt0.rgb;
```

The design system gains **one global light** — direction, colour, intensity in nits —
declared in `aura-design/src/tokens/`. Every surface in the OS responds to it coherently.
Tilt the device, the light moves, and **every highlight in every application moves
together**, because the compositor owns the light and you own the compositor.

`shadow` is currently unimplemented in `wgpu/src/effects/shadow.rs`, deliberately, because
a rasterised region cannot know the outline that cast it. In this architecture that
problem evaporates: the G-buffer *has* the outline and the normal, so contact shadows come
out of the same lighting pass for free.

### The proof gate

Two surfaces, one glass panel over a moving photograph, in Aura and in Flutter. **Gate:
the Aura panel shows parallax between the backdrop and the panel edge as content moves
behind it, and chromatic separation at the bevel. The Flutter one shows a blur that
translates rigidly.** Anyone can see the difference in a two-second video, which is
exactly the point.

---

## Pillar IV — Text as geometry

**Ceiling addressed:** 5. **Horizon:** H2–H3. **Cost:** 5–8 months. High risk; text is
the hardest correctness surface in any UI framework.

### The design

Keep `cosmic-text` for shaping — it is good, it handles bidi and complex scripts, and
rewriting shaping is a five-year mistake. Replace **rasterisation** only.

1. **Outlines live on the GPU.** A glyph is uploaded once as its Bézier contours into a
   persistent storage buffer, keyed by `(face_id, glyph_id, variation_coords)`. Never
   rasterised on the CPU. Never re-uploaded on scale change.
2. **The fine stage rasterises glyphs like any other path.** A glyph run becomes a draw
   object in the Pillar I scene. Analytic coverage, same code path as a rounded rectangle.
   The glyph atlas — and its eviction, its thrash, its CPU cost on the frame thread —
   ceases to exist.
3. **Variable axes become animatable.** `wght`, `wdth`, `opsz` are uniforms. Interpolating
   weight during a transition costs nothing, because there is no raster to redo. **No
   mainstream framework can animate a variable-font axis per frame.**
4. **Optical sizing becomes automatic.** The renderer knows the final on-screen size after
   every transform, so it can drive `opsz` correctly — which is what the axis was designed
   for and what nobody does.
5. **Subpixel positioning and hinting become continuous** rather than quantised to atlas
   slots.

### What it buys

Text that stays exact through arbitrary scale, rotation, perspective, and 3D transform.
Text whose weight can breathe with a spring. Text that costs the frame thread nothing.
And in a foveated 2051 pipeline, text that is shaded at native rate in the fovea and
coarse rate outside it — **which a bitmap atlas simply cannot express**.

### The proof gate

Golden suite. `tests/golden/multilingual_text.rs` already exists. **Gate: every existing
text golden passes within tolerance on the new path, plus new cases at 0.25×, 1×, 8× scale
and at 45° rotation showing no resampling error.** If the multilingual goldens do not
hold, the path does not ship. Text regressions are how frameworks lose trust permanently.

---

## Pillar V — Motion the renderer understands

**Ceiling addressed:** design-level. **Horizon:** H1–H2. **Cost:** 8–12 weeks.

### The problem

`aura-motion` is good — springs, decay, keyframes, timelines, shared elements,
reduced-motion. But it is an *application-level* library. It produces a number each frame
and hands it to a widget. The renderer has no idea anything is moving, and neither does
the compositor.

### The design: velocity is a scene property

1. **Every transform in the scene carries its velocity.** The scene encoding (Pillar I)
   stores `(transform, d_transform/dt)` per draw object.
2. **Damage becomes predictive.** `graphics/src/damage.rs` already correctly damages both
   the previous and current bounds of moving content. With velocity it can damage the
   *swept* region, and more importantly it can tell the compositor where content will be
   next frame.
3. **Frames can be re-timed.** `aura-platform-vyom/src/frame_clock.rs` already computes
   `target_error` from real presentation feedback. With trajectories, a frame that is
   going to miss its deadline can be **re-evaluated at the actual presentation time**
   instead of being presented late and wrong. This is the mechanism that makes 0.000%
   missed frames hold under real load rather than in a benchmark.
4. **Motion blur becomes possible and nearly free** — per-object velocity is exactly the
   input a blur pass wants, and at 480 Hz displays it is what makes fast motion read as
   smooth rather than as strobing.
5. **Reduced motion becomes a renderer transform, not an app concern.** One user setting
   scales every velocity in the scene, coherently, in every application, without a single
   app opting in. `aura-motion/src/reduced_motion.rs` becomes system policy.

### The proof gate

Under a synthetic 3× CPU load spike, a spring animation re-timed at present-time shows
**less than 1 ms of positional error against its analytic trajectory**, where the current
pipeline shows a visible stutter. Measured, not eyeballed — `frame_clock.rs` already
records what is needed.

---

## Priority II — THE CLOCK

---

## Pillar VI — Finger-to-photon, not frames-per-second

**Ceiling addressed:** strategic. **Horizon:** H1–H2. **Cost:** 3–5 months.
**This is the pillar that requires owning VYOM, and it is therefore the pillar nobody can
copy.**

### The number nobody publishes

Touch-to-photon on a modern Android flagship is 60–100 ms. On iOS, 50–70 ms. Users cannot
name it and can feel every millisecond of it. The budget breaks down roughly:

| Stage | typical | Aura on VYOM, achievable |
|---|---:|---:|
| Sensor → kernel | 8–16 ms (120 Hz sampling) | 1 ms (1 kHz sampling) |
| Kernel → compositor → app | 8–16 ms (one frame) | 0.5 ms (shared memory, no round trip) |
| App processes input | 1–16 ms (next vsync) | 0.2 ms (sub-frame, Pillar VII) |
| Render | 8–16 ms | **0.06 ms** (measured: 57.59 µs worst p95) |
| Composite | 8–16 ms (one frame) | 0 ms (direct scanout) |
| Scan out | 8–16 ms | 2 ms (480 Hz) |
| **Total** | **~50–100 ms** | **~4 ms** |

The render column is already won by a factor of a hundred and it does not matter, because
it is 1% of the total. **Everything else on that list is compositor and platform work,
and that is exactly the work Flutter and React Native are structurally forbidden from
doing.**

### The design

1. **High-rate input sampling decoupled from the frame.** VYOM delivers input events at
   sensor rate with hardware timestamps. `aura-gesture`'s arena consumes them on a
   dedicated thread and maintains a *current* pointer state that the frame samples, rather
   than a queue the frame drains.
2. **Present-time-anchored scheduling.** Every frame is rendered *for* a predicted
   presentation timestamp, and input is sampled at the latest possible moment before
   submission. `frame_clock.rs` has the prediction machinery; it needs to drive the
   sampling point.
3. **Late-latch transforms.** A scroll or a drag is a transform. Submit the frame, and
   patch the transform from the newest input sample immediately before scanout. Standard
   in VR, absent from every 2D UI framework on Earth.
4. **Direct scanout.** A full-screen VYOM surface with no translucent content above it
   goes straight to the display controller plane. Zero composite. Zero copy. Measurable in
   both latency and battery.
5. **Speculative input.** For a drag, render the predicted next position and correct on
   arrival. `aura-gesture/src/velocity.rs` already computes what the predictor needs.

### The proof gate

A high-speed camera at 1,000 fps, a capacitive test rig, and a bar chart. **Gate: Aura on
VYOM measures under 15 ms touch-to-photon on the same physical panel where Android
measures over 50 ms.** Publish the video. That single video is worth more than every
microbenchmark in `benchmarks/baseline/`, because it is the only performance claim a
non-engineer can verify with their own eyes.

---

## Pillar VII — Zero-rebuild reactivity (`aura-signal`)

**Ceiling addressed:** 1. **Horizon:** H1. **Cost:** 4–6 months. **Largest measurable
performance win available. Do this before Pillar I.**

### The design

Fine-grained reactivity — signals — instead of view rebuilding. SolidJS proved the model
in the DOM; nobody has taken it to a native GPU framework at scale, and Rust's ownership
model makes it *safer* here than it is in JavaScript.

```rust
// The Elm path, today:
//   state changes → view() allocates the world → diff the world
//                 → mark every node PAINT → layout from the root
//
// The signal path:
let items = Signal::new(vec![...]);
let count = Memo::new(move || items.with(Vec::len));
let label = Memo::new(move || format!("{count} items"));

text(label)      // subscribes to `label` only

items.update(|v| v.push(x));
// → `count` invalidates → `label` invalidates → exactly one text node
//   marks PAINT|LAYOUT. No view() call. No tree diff. No root layout.
```

Then make invalidation an escalation ladder rather than a blanket. The flags already
exist at `core/src/widget/tree.rs:595` — `LAYOUT`, `PAINT`, `SEMANTICS`. Add `VALUE` below
them and enforce that a change stops at the lowest class that suffices:

| class | example | recomputes |
|---|---|---|
| `VALUE` | a colour token changes | one uniform write, no repaint of geometry |
| `PAINT` | hover state | one node's draw commands |
| `LAYOUT` | text content changes width | that node and its ancestors' flex line |
| `STRUCTURE` | a row is inserted | keyed reconcile of one subtree |

`runtime/src/user_interface.rs:129`'s unconditional
`state.mark_dirty(DirtyFlags::PAINT)` is the line this pillar deletes.

### Then wire the two dormant subsystems

Signals make Ceiling 6 tractable, because now there is a *reason* to ask whether a subtree
changed:

- `Tree::layout_with_cache` gets its first real caller. A subtree whose signals did not
  fire returns its cached node.
- `RetainedLayerCache` gets adopted — but **first** make `recording_retained_frame` a
  stack, as Ceiling 6 requires. A subtree that has not invalidated replays its retained
  layer instead of re-recording.

### The projected win

`virtual_list` spends **30.08 µs p95 in `update`** against 32.42 µs in `draw`. A signal
graph that touches only changed rows should take that to single-digit microseconds, and
the same change collapses the `draw` half via retained layers. **Target: `virtual_list`
p95 from 57.59 µs to under 15 µs, and — far more importantly — a cost curve that is flat
in list length instead of linear.**

### The migration, because this is an API break

Do not break the Elm API. Add signals alongside it:

1. `aura-signal` ships standalone with its own tests. No framework changes.
2. `Signal<T>` becomes usable as a widget property anywhere a value is taken today.
3. A signal-driven `view` becomes an opt-in `Program` variant.
4. Elm-style `view()` is reimplemented *on top of* signals — a single root signal that
   invalidates everything, which is exactly today's semantics. **Both models coexist
   permanently, and the old one costs nothing to keep.**

### The proof gate

`tests/bench` gains a scene that mutates one row in a 100,000-row list, once per frame.
**Gate: frame cost is within 20% of `idle_shell` (6.51 µs), and does not change when the
list grows to 1,000,000 rows.** That gate is the whole pillar in one line.

---

## Pillar VIII — The frame graph runs on every core

**Ceiling addressed:** 2. **Horizon:** H2. **Cost:** 4–6 months. Requires VII.

### The design

Four stages, three threads, one pipeline:

```
thread A (UI)      signal propagation, event dispatch, application update
thread pool        parallel layout — independent subtrees are independent
                   problems; flex lines within a row are data-parallel
thread pool        parallel paint recording into per-tile command lists —
                   this is the natural shape of Pillar I's coarse stage
thread B (render)  GPU submission, present, damage accumulation
```

Prerequisites, in order:

1. **Signals first.** Parallel layout on a tree that is fully invalidated every frame
   parallelises the wrong work and wins little.
2. **`Widget::layout` must become `&self` for the parallel path**, or subtree layout must
   move to a separate immutable structure. This is the deep API question of this pillar
   and it should be answered before a line is written.
3. **The render thread needs the scene by value.** Pillar VII's structural sharing gives
   this for free — the render thread takes a cheap immutable snapshot.

### The proof gate

`ten_thousand_rows` and `text_heavy` on a 4-core and a 16-core machine. **Gate: ≥2.5×
speedup on 16 cores for layout-bound scenes, and zero change to any golden.** If a golden
moves, the parallelisation is wrong — determinism is non-negotiable and the golden suite
is exactly the instrument that proves it.

---

## Pillar IX — GPU-resident scene, delta-uploaded

**Ceiling addressed:** 1, 6. **Horizon:** H2–H3. **Cost:** 3–4 months. Requires I and VII.

Today the display list is rebuilt and re-uploaded every frame. With signals, the *scene*
is stable and only deltas move:

- Draw objects live in a persistent GPU buffer with a free-list allocator.
- A signal firing produces a small `SceneEdit` — patch these bytes at this offset.
- Uploads become proportional to *change*, not to scene size.
- On unified memory (every 2051 SoC, and most 2026 ones), the "upload" is a write to
  memory the GPU already sees.

**Gate: a 100,000-object scene with one object changing uploads under 1 KB per frame.**

---

## Priority III — EVERYTHING ELSE

---

## Pillar X — Semantics as the source of truth

**Ceiling addressed:** strategic. **Horizon:** H2–H3. **This is the most defensible
long-term position in this document and it is nearly free, because the asset already
exists.**

Aura builds a retained semantic tree **every frame, whether or not anyone is listening.**
No other framework does this; in Flutter and RN the accessibility tree is an export
generated on demand when a screen reader attaches, and it is perpetually, structurally
out of date.

**Invert the relationship.** Make the semantic tree the primary artifact and visual
rendering *one presentation of it*. Then:

- A **visual** presentation renders it to a screen. (Today's renderer.)
- A **spatial-audio** presentation renders it to a soundfield — a UI you navigate by ear,
  where a button is *to your left*.
- A **braille** presentation renders it to a cell array.
- A **voice** presentation makes the whole UI conversational, for free, because the tree
  already knows every control, its role, its state, and its actions.
- An **agent** presentation exposes it as a tool schema — in a world where software is
  increasingly driven by models, **a UI framework whose every control is already a typed,
  addressable, machine-readable action is a completely different product** from one where
  an agent has to look at a screenshot and guess.

That last point deserves emphasis. By 2051 the dominant consumer of user interfaces may
not be a human eye. A framework that ships a live semantic graph as a first-class output
is positioned for that; a framework that ships pixels is not.

**Gate: drive the full `aura-showcase` end to end with the screen off, through the
semantic tree alone, with no visual renderer instantiated.** If that works, the inversion
is real.

---

## Pillar XI — Natively spatial

**Horizon:** H3–H4. **Cost:** 6–9 months.

2051 displays are not rectangles. The framework should be 3D at the core with a
2D-privileged authoring path:

- The scene graph carries a real `Transform3D`; 2D is the `z = 0` case with an
  orthographic camera. Costs nothing on flat displays.
- Layout gains **physical units** — millimetres at a stated viewing distance — alongside
  logical pixels. A button that is 8 mm at arm's length is 8 mm on a phone, a monitor, and
  a waveguide. Logical pixels cannot express that and never could.
- Per-eye rendering with a shared scene and two view matrices.
- **Foveated shading**, which the tiled compute rasteriser of Pillar I supports natively:
  a tile knows its eccentricity from the gaze point and picks its sample rate accordingly.
  This is precisely the thing a fixed-function MSAA pipeline cannot do at all.
- Depth-correct compositing with the VYOM compositor, so windows occlude in 3D.

---

## Pillar XII — The idle NPU

**Horizon:** H3. **Cost:** 3–5 months.

Every SoC shipping today has 40+ TOPS sitting completely idle while the UI runs. By 2051
it is 1,000+. Four uses that are real engineering rather than a press release:

1. **Effect upscaling.** Render backdrop blur and refraction at quarter resolution,
   reconstruct with a small network. `graphics/src/quality.rs`'s `Tier` system is already
   the right place to hang it: a new `Tier::Neural` above `Ultra`.
2. **Input prediction.** A 2-layer model over `aura-gesture/src/velocity.rs`'s history
   beats linear extrapolation for finger trajectories, and directly buys milliseconds in
   Pillar VI's budget.
3. **Layout adaptation.** `responsive` and `adaptive` widgets exist. A model that has
   learned which layout a *particular user* actually reaches for, on this device, at this
   size, is a better breakpoint system than any set of hand-written thresholds.
4. **Per-user motion tuning.** `aura-motion`'s spring constants are currently a designer's
   guess. Learn the damping a specific user reads as responsive rather than frantic. This
   is accessibility as personalisation rather than as a binary toggle.

Constraint: **every one of these must degrade to a deterministic fallback**, and the golden
suite must run with all of them off. A framework whose output is not reproducible is not a
framework.

---

## Pillar XIII — Compile-time UI, and the size problem

**Ceiling addressed:** 8. **Horizon:** H2. **Cost:** 3–4 months.

15.76 MiB stripped does not support the story this document tells. Rust offers a lever no
competitor has — **resolve the static parts of a UI at compile time**:

- A `const` layout solver for subtrees whose constraints are known statically.
- Design tokens folded to constants; `aura-design`'s whole token graph collapses.
- Shader specialisation per material — no runtime branches for features an app never uses.
- Widget-set dead-code elimination: an app that never constructs a `pane_grid` should not
  link one.
- **Drop the default font embed.** `fira-sans` is on by default and costs ~430 KiB
  precisely so rendering is machine-independent. Correct for a test harness; wrong for a
  shipping app on an OS you control, because **VYOM can ship the font once, system-wide.**

**Gate: `mobile_shell` under 4 MiB stripped, and a trivial app under 1.5 MiB.** Then
measure cold start — which, per Ceiling 8, has never been measured at all, and which needs
a harness before it needs an optimisation.

---

## Pillar XIV — The authoring loop

**Ceiling addressed:** 9 (partially). **Horizon:** H1–H2. **Cost:** ongoing.

An engine nobody can build with is a research project. Four things, in priority order:

1. **State-preserving hot reload.** `hot` exists as an experimental feature. Signals make
   it genuinely tractable: swap the view function, keep the signal graph. **Sub-100 ms
   edit-to-pixel or it does not count.**
2. **The frame inspector.** `aura-profiler` and Aura Lab exist. What is missing is the
   thing Flutter DevTools does best: click a pixel, see the widget, see its layout
   constraints, see its GPU cost, see why it invalidated. **"Why did this repaint?" should
   be one click**, and with a signal graph the answer is exactly a path through that graph.
3. **Design tokens as a compiled artifact.** A designer edits tokens; the build emits
   Rust constants; the golden suite proves nothing else moved. Pillar II makes this
   physical — tokens carry nits and material parameters, so a design system becomes a
   lighting specification.
4. **`aura new` and `aura run` that work on the first try.** Unglamorous. Decides adoption.

---

# PART IV — HORIZONS

Dates are from 2026-08-24. Sequencing matters more than dates; the dependency arrows are
the real content.

### H0 — Make it exist (now → 3 months) · *blocks literally everything*

| | |
|---|---|
| **Attach buffers in the VYOM backend.** Ceiling 7. Nothing presents. | P0 |
| Cold start, energy, and steady-state memory harnesses. Ceiling 8. | P0 |
| One flagship application that ships, on VYOM, that people use daily. | P0 |
| First honest Flutter comparison per `benchmarks/competitors/README.md`. | P1 |

**Nothing below H0 matters if H0 does not happen.** A framework that does not present a
frame on a device has no users, and a twenty-five year plan with no users is a hobby.

### H1 — The visible generation gap (3–12 months)

- **Pillar II — HDR compositing.** Highest value per week in the document.
- **Pillar VII — `aura-signal`.** Largest measurable performance win.
- **Pillar VI part 1** — high-rate input, present-time anchoring, direct scanout.
- **Pillar V** — velocity in the scene, present-time re-timing.
- **Pillar XIV part 1** — state-preserving hot reload.
- Wire the two dormant subsystems (Ceiling 6): recorder-as-stack, then
  `RetainedLayerCache`, then `layout_with_cache`.

**End state:** a device that visibly emits light other devices cannot, responds faster than
any phone on the market, and holds a flat cost curve on lists a competitor chokes on.

### H2 — The engine nobody else has (1–2.5 years)

- **Pillar I — `aura-raster` compute rasteriser.**
- **Pillar III — materials and lighting.**
- **Pillar IV — GPU vector text.**
- **Pillar VIII — parallel frame graph.**
- **Pillar XIII — compile-time UI and size.**
- **Pillar VI part 2** — late-latch, speculative input.

### H3 — The different category (2.5–6 years)

- **Pillar IX** — GPU-resident scene.
- **Pillar X** — semantics as source of truth; the agent-addressable UI.
- **Pillar XI** — spatial, physical units, foveation.
- **Pillar XII** — NPU-assisted UI.

### H4 — Research (6–25 years)

Spectral rendering for perfect colour under any illuminant. Neural scene representation as
a render target. Direct display-controller programming with no compositor at all. Sub-1 ms
end-to-end at 1 kHz. UI generated per-user, per-context, from the semantic tree — where
Pillar X stops being an accessibility feature and becomes the entire product.

### The critical path

```
H0 present a frame ─┬─→ II  HDR ────────┬─→ III materials ─→ XI spatial
                    │                    │
                    ├─→ VII signals ─────┼─→ I  raster ─────┬─→ IV  text
                    │        │           │                   └─→ XII NPU
                    │        └─→ VIII parallel ─→ IX resident scene
                    │
                    └─→ VI  latency ────────────────────────→ X  semantics
```

Read it as: **presenting a frame unblocks everything; HDR and signals unblock everything
else; and neither depends on the other, so they run in parallel from month three.**

---

# PART V — HOW YOU KNOW YOU ARE WINNING

Public claims are earned by gates, not by adjectives. This project has already learned
that lesson the hard way — the README's refusal to quote an unverified Flutter comparison
is the most credible paragraph in the repository. Keep that standard.

| # | Gate | Unlocks the claim |
|---|---|---|
| 1 | A VYOM device presents an Aura frame, daily-driven by a real person | *"It ships."* |
| 2 | Touch-to-photon under 15 ms, high-speed camera, same panel as Android | *"Fastest UI on any device."* |
| 3 | HDR highlight measures >1,000 nits where Flutter measures SDR white | *"The only UI framework that shows light."* |
| 4 | One row changed in 1,000,000 costs the same as idle | *"Cost scales with change, not size."* |
| 5 | Glass panel shows parallax and dispersion; Flutter's blur translates rigidly | *"Materials, not decorations."* |
| 6 | Text exact at 0.25×–8× and 45°, all multilingual goldens pass | *"Resolution independent."* |
| 7 | Cold start under 50 ms, app under 4 MiB, measured | *"Instant."* |
| 8 | Same app, three frameworks, one device, published per the competitor rules | *"Measurably faster."* |
| 9 | Full showcase driven with the screen off, via semantics alone | *"The interface is not the pixels."* |
| 10 | 500 third-party components exist that the core team did not write | *"It is an ecosystem."* |

**Gate 10 is the one that decides whether any of this matters.** Gates 1–9 are engineering
and this team can clearly do engineering. Gate 10 is a community, and it is the only gate
here that cannot be closed by writing code.

---

# PART VI — WHAT NOT TO DO

Ranked by how much time each one will cost you, worst first.

1. **Do not start with the compute rasteriser.** It is the most interesting item in this
   document and it will eat a year. It is worth a fraction of its value on a pipeline that
   rebuilds the world every frame. Signals first. Always signals first.

2. **Do not chase Flutter's component list.** You will never catch it, and every week
   spent on a breadcrumb widget is a week not spent on the things it cannot copy. Ship the
   forty components a real application needs, make them the best forty in the industry,
   and let the ecosystem do the long tail — which it will only do if Gate 1 happens.

3. **Do not build mechanism you do not immediately wire.** This is a documented, repeated,
   specific failure mode in this repository — `RetainedLayerCache` and `layout_with_cache`
   are both complete, both tested, and both called by nothing but their own tests. **Rule:
   a subsystem lands with its first production call site in the same pull request, or it
   does not land.**

4. **Do not claim anything without a committed measurement.** The README's discipline here
   is a genuine competitive asset. One inflated benchmark, discovered by one hostile
   engineer on one forum, destroys it permanently.

5. **Do not port to iOS and Android to chase reach.** It is the obvious move and it is
   the wrong one. The moment Aura is a guest on someone else's compositor, Pillars II, III,
   VI, XI, and the entire moat evaporate, and you are competing with Flutter *on Flutter's
   terms* with 1% of its team. **Ship on VYOM. Let the device be the argument.** A port is
   a decision to be a worse Flutter.

6. **Do not let the semantic tree become an accessibility checkbox.** It is the most
   strategically valuable asset in this repository and almost nobody in the industry
   understands why yet. Treat it as Pillar X describes.

7. **Do not rewrite text shaping.** `cosmic-text` is good, bidi and complex-script
   correctness is a decade of work, and replacing rasterisation gets you everything you
   want without touching the part that is genuinely hard to get right.

8. **Do not let the golden suite rot.** It has already caught two framework-wide bugs that
   1,705 passing tests missed. It is the only instrument in the repository that looks at
   what a user actually sees, and every pillar above is gated on it.

---

## Closing

The engine in this repository is, on the axes it has actually built, already better than
Flutter's and React Native's — measurably, reproducibly, and with a test discipline
neither can match. It is also, today, a framework that has never put a pixel on a screen
through the platform it was built for, with 75 components against thirty thousand.

Those two facts are not in tension. They are the whole strategic picture. **The hard part
is done and the obvious part is not.**

Fix the obvious part first — H0, three months, present a frame and ship one application.
Then spend the following year on HDR and signals, which are cheap, independent, and
between them produce a device that a person can *see* and *feel* is from a later decade
than the phone in their other hand.

Everything after that is a compounding advantage on a stack nobody else owns.

Flutter cannot follow you into HDR compositing, because it composites in 8-bit and hands
its pixels to an OS that belongs to someone else. React Native cannot follow you into
4 ms latency, because it is three abstraction layers above the input event. Neither can
follow you into materials, or foveation, or physical units, or an agent-addressable
semantic graph, because all of those require owning the compositor — and portability, the
thing that made both of them successful, is precisely the thing that forbids it.

That is not a gap they close with more engineers. That is a gap that gets wider every year
you keep building.

---

### Appendix — Reproducing this audit

```bash
# Structure
find . -name "*.rs" -not -path "./target/*" | wc -l          # 560
find . -name "*.rs" -not -path "./target/*" -exec cat {} + | wc -l   # 158925
grep -rc "#\[test\]\|#\[tokio::test\]" --include="*.rs" . | grep -v target | \
  awk -F: '{s+=$2} END {print s}'                            # 1634

# Ceiling 1 — the unconditional full-tree paint invalidation
grep -n "mark_dirty" runtime/src/user_interface.rs           # :129 PAINT, whole tree

# Ceiling 2 — no thread on the frame path
grep -rn "thread::spawn" --include="*.rs" core graphics runtime wgpu winit
# → image decode, AT-SPI, clipboard, task executor. Nothing else.

# Ceiling 3 — the entire colour model
cat graphics/src/color.rs                                    # 52 lines, sRGB only

# Ceiling 4 — no compute anywhere
grep -rn "@compute\|ComputePipeline" wgpu/                   # (empty)

# Ceiling 6 — machinery with no callers
grep -rn "RetainedLayerCache" --include="*.rs" . | grep -v target | cut -d: -f1 | \
  sort | uniq -c
#   16  graphics/src/retained_layer.rs   ← its own definition + its own #[cfg(test)]
#    1  graphics/src/text/cache.rs       ← a doc comment, line 23
#   14  tests/graphics/retained_layers.rs
# → zero production call sites.

grep -rn "layout_with_cache" --include="*.rs" . | grep -v target | grep -v tests
# → core's own definition and tests, plus one comment in widget/src/row.rs:207.
# → zero production call sites.

# Ceiling 7 — nothing presents
grep -n "attach" aura-platform-vyom/src/surface.rs
# → may_attach_buffer(), a protocol-correctness predicate, and its test.
#   No call to wl_surface.attach exists anywhere in the crate.

# Build state
cargo check --workspace --all-targets                        # exit 0
```

*Companion documents: `Capstone.md` (competitive audit and the catch-up plan),
`phase.md` (execution roadmap through Aura 1.0), `audit.md` (the first audit, superseded),
`README.md` (measured baselines), `benchmarks/competitors/README.md` (comparison rules).*
