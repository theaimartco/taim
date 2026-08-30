# The colour pipeline

> Status: live. Every claim here is asserted by a test named in the text, and
> those tests run in CI across the feature matrix in §7.

## 1. The contract

Taim composites in **linear light**. One sentence, and everything else in this
document follows from it.

```text
authored colour                     Color, sRGB-encoded, 0.0..=1.0
      |
      | decode transfer             graphics::color::pack
      v
LINEAR WORKING SPACE                unbounded above 1.0
      |
      | all blending, all effects, all compositing
      v
scene-referred frame                1.0 == SDR diffuse white == 203 nits
      |
      | tone map      DisplayTransform, BT.2390 EETF
      | gamut map     to the panel's primaries
      | encode        the panel's transfer function
      v
surface                             what the display receives
```

The steps are not reorderable. Encoding before blending is the defect this
pipeline was rebuilt to remove; gamut mapping before tone mapping produces hue
shifts that read as a design-system fault rather than a renderer fault.

## 2. What `1.0` means, at each stage

The single most expensive recurring mistake in this codebase has been
conflating *the brightest the panel can emit* with *what the value 1.0 means*.
They are different numbers, and four separate defects came from assuming
otherwise.

| Stage | `1.0` means | Type |
|---|---|---|
| Authored `Color` | full-scale sRGB white | `taim_core::Color` |
| Packed / working space | SDR diffuse white, **203 nits** | `graphics::color::Packed` |
| `Radiance` | SDR diffuse white; a 1,000-nit highlight is `4.93` | `core::color::Radiance` |
| Quad luminance multiplier | ordinary surface; `4.93` emits 1,000 nits | `renderer::Quad::luminance` |
| PQ-encoded signal | **10,000 nits**, not diffuse white | `Transfer::Pq` |
| Display-referred surface (sRGB monitor) | the panel's peak, which is also its white | `Display::SDR` |
| Extended surface (macOS EDR, scRGB) | SDR reference white, with headroom **above** `1.0` | `Display::P3_EXTENDED` |

`Display` therefore carries `reference_white_nits` *and* `peak_nits`
separately. Encoding relative to peak on an extended surface put diffuse white
at `203/1600` and rendered entire interfaces at 12.7% brightness; that is what
the distinction exists to prevent.

Scene-referred values are unbounded above. `Radiance` does not clamp, and
neither does `Packed::scale_light`, because clamping a highlight to `1.0` is
exactly the information loss the pipeline exists to avoid.

## 3. Who encodes

Exactly one encode per frame. Never zero, never two.

`TargetColorState::encodes_in_hardware` is the **single** source of truth, and
nothing else in the renderer asks a format whether it is sRGB.

| Target format | Hardware applies transfer | So the shader outputs |
|---|---|---|
| `*_Srgb` | yes | linear |
| `Rgba8Unorm`, `Bgra8Unorm` | no | encoded |
| `Rgba16Float` | no | encoded for the target's transfer |

A double encode leaves black and white exactly right and moves mid-grey from
0.5 to 0.74, so it reads as a badly tuned theme. `a_frame_is_encoded_exactly_once`
pins the invariant; `mid_grey_is_neither_washed_out_nor_crushed` pins the pixel.

## 4. The number that proves it

50% white over black, composited in linear light and written to an sRGB
surface, is **187**. The same composite in gamma space is **128**.

That is a 59-step difference out of 255 on the most common operation in any
interface, and it is the fastest way to tell which space a build is in.
`fifty_percent_white_over_black_is_composited_in_linear_light` asserts the
first; `the_legacy_mode_blends_in_gamma_space` asserts the second.

## 5. Target lifecycle

```text
surface creation
      |  select_format(available, ColorPolicy)      compositor.rs
      v
format negotiated  ->  Rgba16Float only under ColorPolicy::PreferHdr
      |
      v
Engine::pipelines(format)                           builds on first use, cached
      |
      v
Renderer::use_format(target.format())               once per frame, one compare
      |
      v
draw  ->  [present | present_hdr]  ->  DisplayTransform  ->  surface
```

**An `Engine` is not bound to a format.** It owns the device, the queue, the
shared caches and a format-keyed pipeline cache; a pipeline's identity includes
its attachment format, which belongs to the target. `Renderer::use_format`
re-resolves when the target changes and rebuilds only what depends on the
format — the pipelines, the triangle MSAA state, and the text glyph viewport.
Nothing device-global is touched, so a window dragged between an SDR and an HDR
display keeps its images, caches and primitive storage.

This is what `tests/gpu/target_formats.rs` proves: one engine, three formats,
in any order, repeatedly.

## 6. Texture-budget lifecycle

```text
request        prepare(tier, size)
      |
      v
account        Pool::acquire(Descriptor)     format-aware; see below
      |
      +-- fits           -> allocate/reuse, render
      |
      +-- does not fit   -> halve both dimensions, retry
      |
      +-- below 64px     -> refuse; queued effects are dropped this frame
      v
release        back to the free list, reusable
      |
      v
evict          largest idle first, when the budget needs the room
```

The budget is `quality::Tier::policy().texture_budget_bytes`, synced to the
pool once per frame in `Renderer::draw` — unconditionally, including frames
that queue no effects, so telemetry never reports a budget that is no longer
in force.

Cost is computed by `estimated_texture_bytes`, which accounts format, mip
chain, array layers and sample count. It is **an estimate, deliberately**:
`wgpu` cannot report committed device memory, so this is the accounted texel
size — deterministic, driver-independent, and the right basis for a budget.
Do not present it as VRAM usage.

A half-float intermediate costs exactly twice an 8-bit one, which is why the
previous hard-coded `bytes_per_pixel = 4` under-priced every HDR frame by half.

## 7. Feature matrix

| Configuration | Compositing | Notes |
|---|---|---|
| `cargo test` | **linear** | production |
| `cargo test --no-default-features` | **linear** | genuinely minimal |
| `cargo test --features web-colors` | gamma | legacy Web compatibility |
| `cargo test --all-features` | gamma | `web-colors` is included |

`web-colors` is opt-in. It was a *default* feature, which made gamma-space
blending the shipping behaviour, and the `taim` crate listing itself as a
dev-dependency with default features meant no test configuration could observe
it. Both halves are fixed: the feature is out of `default`, and the workspace
dependency sets `default-features = false`.

`tests/color/contract.rs` fails if either regresses.

## 8. The CPU backend, and what its precision costs

`tiny_skia` composites in the same working space as the `wgpu` backend.
`tiny_skia/src/engine.rs`'s `into_color` routes every authored colour —
solids, borders, shadows, gradient stops — through `working_components`, which
decodes to linear light whenever `GAMMA_CORRECTION` is set. The frame is
encoded once, on the way out, by `encode_channel`.

The two backends therefore agree, which is the property that makes a golden
mean anything: the golden suite renders through `tiny_skia`, so if it
composited in a different space from the shipping renderer it would be pinning
a pipeline no user ever sees. That was the case until this change, and it is
why flipping the default to linear initially moved **zero** goldens.

### The live path needs two buffers

`tiny_skia` presents through `softbuffer`, whose buffer persists between frames
and is reused by damage-based partial redraw. Blending reads that buffer back
as the destination, so it has to stay in the working space — encoding it in
place would feed the next frame's blends an sRGB destination and compound the
error on every frame the damage did not cover.

`Surface::working` is therefore a second, persistent, linear buffer. The frame
composites there and is encoded into the presentation buffer by `encode_into`.
Screenshots take the same path without the persistence.

### The precision this costs

`tiny_skia` blends in `f32` but **stores** 8-bit, so a linear working space is
re-quantised on every draw. Linear 8-bit spends its codes evenly across the
range while perception does not:

- sRGB `0..=31` maps onto only **four** distinct linear codes.
- A storage round trip moves a dark value by at most **6/255** (measured across
  all 256 values; mean 0.50).

This is exactly what transfer functions exist to prevent, and `tiny_skia` has
no sRGB-aware storage to avoid it — Skia has one, `tiny-skia` dropped it. The
cost is real, is paid on the CPU backend only, and is the price of the two
backends agreeing.

Measured on the golden suite, the effect is bounded and small: of 34 goldens
that changed, exactly one changed across almost its whole area
(`stepper_at_rest_dark`, 99.43% of pixels) and its worst channel was **4** —
inside the 6/255 bound, i.e. quantisation rather than a rendering change. The
other 33 changed at blend edges, where the difference is the intended
correction. See `docs/testing/golden-linear-recapture.md`.

### Authored bytes, which do not pass through `into_color`

Four sources reach `tiny_skia` as 8-bit values without going through
`into_color`, so each needs decoding explicitly:

| Source | Site | Decoded by |
|---|---|---|
| Text colour | `text.rs` | `decode_channel` |
| Colour-emoji bitmaps | `text.rs` | `decode_channel` |
| Decoded image samples | `raster.rs` | `decode_channel` |
| SVG colour filter | `vector.rs` | `decode_channel` |
| Rasterised SVG output | `vector.rs` | `decode_premultiplied` |

**Coverage is never decoded.** A glyph's alpha and an image's alpha are
fractions of a pixel, not quantities of light; putting them through a transfer
function makes every antialiased edge the wrong weight.

`resvg` output needs `decode_premultiplied` rather than `decode_channel`
because its pixels are premultiplied, and a transfer function is not linear:
`decode(c · a) ≠ decode(c) · a`. The component is un-premultiplied, decoded,
and re-premultiplied.

Missing this is not a subtle error. With the frame encoded on the way out,
treating an authored byte as already linear presents it far too bright — sRGB
128 text left the backend as 188. It is invisible to the golden suite, because
it produces per-channel differences in the same range as the legitimate
blend-space correction; what separates them is that a body error moves a
contiguous block by one constant delta while an edge correction leaves a thin
scatter. `authored_bytes_decode_into_the_working_space` pins the transform, and
`docs/testing/golden-linear-recapture.md` records how the shape of the change
was used to tell the two apart.
