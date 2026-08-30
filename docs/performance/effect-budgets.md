# Effect budgets

> Status: Phase 12 baseline. This document records the per-tier effect
> limits, the cost model those limits bound, how tier changes are
> stabilized, and what remains unverified without reference hardware.

## The bounded cost model

Effects are layer-local, never surface-global. Every effect declares
sampling bounds (`taim_wgpu::effects::Effect::bounds`) computed as its
source region expanded by whatever it samples outside itself, then
clipped to the surface. Concretely:

- **Blur** expands by its radius on each side, where the radius is first
  clamped to the active tier's `max_blur_radius`. An oversized or
  untrusted radius therefore cannot expand damage without limit.
- **Shadow** expands by `blur_radius * 2 + spread`, offset by its own
  offset vector.
- **Backdrop blur** expands by its clamped radius only when the tier
  permits backdrop capture *and* the declared area fits
  `max_effect_area`; otherwise it falls back to the unexpanded source.
  Full-surface capture is never a fallback.
- **Color matrix, saturation, noise, mask** sample only their source and
  expand nothing.

An effect whose bounds do not intersect the surface contributes no
damage at all (`bounds` returns `None`).

### Blur is separable and downsampled, never a full kernel

A radius-`r` blur runs as two 1D passes (`2r + 1` taps each) over a
*downsampled* copy of the region, not one `(2r+1)²` 2D kernel over the
full-resolution region. `Blur::downsample` selects the factor:

| Radius | Downsample factor |
|---|---|
| `<= 8` | 1 |
| `> 8`, `<= 16` | 2 |
| `> 16` | 4 |

`Blur::separable_sample_cost` reports the resulting texel-sample count:
`(area / factor²) * 2 * (2r + 1)`. `tests/gpu/effect_bounds.rs` pins that
this stays far below the naive full-kernel cost.

`Blur::is_bounded_below_surface` encodes the "never blur the full surface
when a smaller sampling region is correct" rule: a small element on a
large surface must produce a strictly smaller sampling region. The
counter-case is respected too -- an element that genuinely covers the
surface legitimately samples all of it.

## Per-tier limits

These are the approved, pinned policies (`graphics/src/quality.rs`, held
to by `tests/golden/effects.rs`):

| Tier | Max blur radius | Max effect area (px²) | Texture budget | Backdrop blur | Noise |
|---|---:|---:|---:|:---:|:---:|
| Ultra | 32 | 2,073,600 | 96 MiB | yes | yes |
| Balanced (default) | 20 | 1,048,576 | 48 MiB | yes | yes |
| Efficient | 8 | 262,144 | 16 MiB | no | no |
| Recovery | 0 | 0 | 4 MiB | no | no |

Invariants enforced by tests, not convention:

- Every limit is **monotonically non-increasing** from Ultra to Recovery.
- A capability disabled at one tier never re-enables at a lower one.
- Every tier keeps a **non-zero texture budget**: Recovery must still be
  able to render the shell, just without expensive effects.
- Recovery's zero blur radius means no effect can expand damage bounds at
  all.

## Texture pool

Intermediate textures are byte-budgeted (`taim_wgpu::effects::pool`).
The pool tracks two quantities:

- `used` -- bytes of textures currently live (acquired, not released).
- `reserved` -- bytes of idle textures kept for reuse.

`used + reserved` never exceeds the budget. `release` moves bytes from
`used` to `reserved` (idle and reusable, not freed); a matching `acquire`
moves them back with **no new allocation**. When a fresh allocation would
overflow the budget, the pool first evicts idle textures (largest first);
if it still would not fit, `acquire` returns `false` and allocates
nothing. A single texture larger than the whole budget is always refused.

This is the concrete "no effect can trigger unbounded texture
allocation" guarantee. `tests/gpu/effect_budget.rs` drives 10,000
acquire/release cycles and asserts the footprint never exceeds budget.

At the graph level, `Graph::push_within_budget` refuses an effect whose
clipped sampling area would push the layer's total past the tier's
`max_effect_area`.

## Tier switching and hysteresis

`runtime::quality_controller::Controller` maps `Pressure` (GPU ratio,
missed-frame ratio, thermal ratio, GPU-recovery flag) to a tier:

- **Recovery** when GPU recovery is in progress or thermal `>= 0.95`.
- **Efficient** when GPU `>= 0.9`, missed frames `>= 0.1`, or thermal
  `>= 0.8`.
- **Ultra** when GPU `<= 0.55`, missed frames `<= 0.01`, and thermal
  `<= 0.4`.
- **Balanced** otherwise.

Ordinary changes are gated by a **5-second minimum residency**
(`DEFAULT_MINIMUM_RESIDENCY`), so a signal jittering at a threshold
cannot flip the tier frame-to-frame; a test drives 600 frames of a
hovering signal and asserts at most two changes.

**Recovery bypasses hysteresis in one direction only.** An escalation
*into* Recovery (GPU loss, thermal-critical) applies immediately -- it is
a protective action, and making it wait five seconds would keep expensive
effects running through exactly the emergency Recovery exists to prevent.
*Leaving* Recovery is still residency-gated, so a flapping thermal signal
cannot oscillate effects back on.

### Where the pressure signals come from

`missed_frame_ratio` is **measured**, not synthesized:
`quality_controller::FramePressure` records each frame's real duration
against the target budget over a sliding window and reports the fraction
that overran; `FramePressure::apply_to` folds it into a `Pressure`.
Thermal and GPU-loss signals must come from the platform -- there is no
portable sensor to invent -- so a real event loop supplies those fields.

## The cost curve

`tools/taim_bench/src/scenes/effect_ladder.rs` walks a fixed ladder that
scales one axis at a time (radius, then source area, then layer count,
then tier) and reports, per rung, the separable sample count and the
intermediate bytes the pool must commit. Its tests pin the curve's shape:

- sample cost grows with source area;
- sample cost grows **linearly** with layer count (16 layers = 4x the
  cost of 4), while pooled intermediates do **not** scale with layers;
- degrading the tier never increases cost;
- Recovery admits nothing and costs exactly zero;
- every admitted rung fits its tier's texture budget.

## What Phase 12 does not close

| Clause | Status | Evidence |
|---|---|---|
| "effects respect damage expansion and budgets" | **Met** | `tests/gpu/effect_bounds.rs`: radius expansion, tier clamping, surface clipping, off-surface culling, never-full-surface (plus its counter-case). |
| "no effect can trigger unbounded texture allocation" | **Met** | `tests/gpu/effect_budget.rs` + `effects::pool` unit tests: budget never exceeded across 10,000 reuse cycles; oversized textures refused; graph area budget enforced. |
| "tier switching is stable" | **Met** | 600-frame hovering-signal test bounds tier changes to <= 2; residency gating and emergency bypass both tested in each direction. |
| "Efficient and Recovery remain readable and coherent" | **Partially met** | Structurally enforced and tested: monotonic degradation, no capability re-enabling, non-zero texture budget at every tier, and Recovery admitting no bounds-expanding effect. *Readability itself is a visual property* -- guaranteed here by policy (effects are never required to distinguish content or controls) but not confirmed by human visual review, which needs the design system and a real display. |
| "effect ladder quantifies the cost curve on every reference GPU" | **Partially met** | The ladder quantifies the *model's* curve deterministically, and its shape is pinned by tests. Correlating those counts with wall-clock GPU time **on each reference GPU** requires that hardware and has not been done. |

## Follow-ups carried forward

1. Correlate the ladder's sample/byte counts with measured GPU time on
   each reference GPU, turning the model curve into a device-calibrated
   one.
2. Wire real thermal and GPU-loss signals from the platform
   (`winit`/`wgpu`) into `Pressure`; only `missed_frame_ratio` is
   measured by the runtime today.
3. **Connect `effects::Graph` to the actual `taim_wgpu` render path.**
   The cost model, bounds, budgets, and tier gating are complete and
   tested, but no live draw call consumes them yet -- the shaders and
   pass scheduling are the remaining renderer-side work.
4. Human visual review of Efficient/Recovery readability once the design
   system (Phase 11) provides real components to review them with.
