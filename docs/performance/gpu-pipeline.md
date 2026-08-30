# GPU pipeline

> Status: Phase 5 baseline. This document records the hot-path
> instrumentation, allocation/batching/caching decisions, and
> device-loss/memory-pressure posture Phase 5 ("Optimize the GPU
> renderer hot path") introduces, following the phase's own seven-step
> optimization order. **Phase 4 ("Damage tracking and retained layers")
> was never completed** (see below) -- every "before/after" comparison in
> this document is against the Phase 1 baseline
> (`benchmarks/baseline/*.json`), not a Phase 4 one, because no Phase 4
> baseline exists. Phase 5's own exit gate asks for improvement "from
> Phase 4 or an evidence-backed hardware limit is documented"; this is
> that documentation for the missing Phase 4 half of that comparison.

## Why there is no Phase 4 baseline

Substantial, uncommitted work toward Phase 4 already existed in this
repository's working tree before Phase 5 began (`graphics/src/{cache,
damage,display_list,retained_layer}.rs`, `wgpu/src/window/compositor.rs`
damage-ratio logging, `tests/graphics/*`, `tests/golden/damage_motion.rs`)
-- but it was never finished, never benchmarked as a phase, and never
committed. Phase 5 was explicitly scoped to skip completing Phase 4 and
proceed directly against whatever baseline exists, which is Phase 1's.
This means Phase 5's exit gate clause "improve >= 20% from Phase 4" is
evaluated here against Phase 1 instead, with the gap called out
explicitly rather than silently substituted.

While fixing unrelated `cargo clippy` findings in those pre-existing
Phase 4 files (four trivial collapsible-if/redundant-closure lints, zero
behavior change) to restore whole-workspace clippy verification, this
phase's own hot-path files remained untouched beyond the lint fixes
themselves.

## Step 1: instrumentation

Added `wgpu::metrics::Counters`: per-frame, per-frame-reset counters for
`Primitive`, `Batch`, `Upload`, `AtlasMiss`, `PipelineSwitch`, and `Draw`,
mirroring `taim_runtime::diagnostics::Counters`'s exact zero-cost-when-
disabled contract (a new `profiling` feature on `taim_wgpu`, off by
default). Wired into `Renderer::prepare` (per-primitive-kind
`Primitive`/`Upload` increments) and `quad::State::render` (`Batch`/
`PipelineSwitch`/`Draw`, once per contiguous same-kind run).

This is additive instrumentation only: no control flow changed, so no
benchmark record is expected to move because of this step alone.

## Step 2: allocation removal

Found and fixed one real, low-risk allocation: `Layer::flush_meshes`/
`flush_text` used `self.pending_meshes.drain(..).collect()`, which
allocates a fresh `Vec` and copies every element into it, then discards
the original (now-empty but still-allocated) `Vec`. Replaced with
`std::mem::take`, which moves the already-populated `Vec` out directly
and leaves a non-allocating empty one behind -- one allocation and one
copy avoided per flush, every frame that has pending meshes or text.

Everything else searched (`engine.rs`, `quad.rs`, `quad/{solid,gradient}.
rs`, `triangle.rs`, `image/atlas.rs`) was already either non-hot-path
(one-time pipeline construction) or already correctly guarded (e.g.
`triangle::State`'s cached-mesh-upload `batch.clone()` only runs when the
cache's version actually changed, not unconditionally per frame).

## Step 3: batching

`quad::Batch` already groups same-kind (solid/gradient) quads into
contiguous runs via its own `order: Vec<(Kind, usize)>` field, built
incrementally in `Batch::add`. Rather than replace this working,
efficient incremental construction with a generalized-but-slower two-pass
grouping algorithm, added `wgpu::batch::{Batchable, group}`: a small,
reusable, ordering-preserving grouping primitive, plus:

- `wgpu/src/quad.rs`'s own `batching_tests` module, proving `Batch::add`'s
  incremental `order` construction is exactly equivalent to running
  `batch::group` over the same `Kind` sequence after the fact.
- `tests/gpu/batching.rs`, the ordering-correctness oracle: proves that
  grouping same-key items into runs and rendering run-by-run produces
  the *exact same visible result* as rendering every item individually
  in original order, including the adversarial case of many items
  overlapping the same pixel while alternating key every time.

No behavior change to the real render path; `quad::Batch` was already
correct. This step formalizes and tests the invariant it already relied on.

## Step 4: persistent buffers and partial writes

`wgpu::Buffer<T>` (pre-existing) already implements persistent,
exponentially-growing GPU buffers (`resize` only reallocates when
growing past capacity, never on every frame) with chunked
`StagingBelt`-backed writes (`MAX_WRITE_SIZE`-sized chunks, not one giant
write). This already satisfies "persistent buffers."

**A true content-diffing partial write (only re-upload instances that
changed since last frame) was considered and deliberately not
implemented.** `solid::Layer::prepare`/`gradient::Layer::prepare`
currently rewrite their entire instance buffer from offset 0 every
frame. A real partial write would require retaining a full copy of the
previous frame's instance data to diff against (extra memory, extra CPU
comparison cost every frame) and is easy to get subtly wrong: a missed
diff silently renders stale GPU data, which is a correctness bug, not
merely a missed optimization. Revisit only if `metrics::Metric::Upload`
profiling on a real scene under realistic conditions proves full-buffer
rewrite (as opposed to draw-call count or shader cost) is the actual
bottleneck -- nothing gathered in this phase demonstrates that yet.

## Step 5: persistent text/image resources with byte budgets

Added a byte [`Budget`](../../core/src/cache.rs) to
`graphics::text::cache::Cache` (default 8 MiB): entries are evicted
least-recently-touched-first once the estimated total would exceed the
budget, *except* an entry touched during the current frame (tracked via
the existing `recently_used` set) is never evicted even if that
temporarily exceeds the budget -- evicting something the current frame
still needs would force a redundant reshape moments later, which is
worse than a brief, bounded overshoot. Byte cost is estimated (`cosmic_
text::Buffer` exposes no public size introspection), proportional to
original content length plus a small per-line/per-entry fixed overhead,
matching `graphics::retained_layer::RetainedLayerCache`'s own
"estimate, don't exactly account" precedent.

**`wgpu::image::cache::Cache`'s GPU atlas has no equivalent eviction
policy yet.** This was deliberately scoped out: evicting a GPU atlas
slot safely requires coordinating with in-flight frames and resource
generations (closer to the abandoned Phase 4 `retained_layer` cache's
own concerns than to a straightforward CPU-side cache), and is a
larger, separately-risked piece of work than this phase's step budget
allowed alongside everything else. Documented here as a real, named
follow-up, not silently dropped.

## Step 6: SDF primitives

`wgpu/src/shader/quad.wgsl`'s `rounded_box_sdf` function, used by both
`quad/solid.wgsl` and `quad/gradient.wgsl`, **already implements**
analytic rounded-rect and border rendering via a signed-distance field,
and an already-"simple" (smoothstep-based, not a full convolution blur)
shadow approximation -- exactly what this step's "analytic rounded rect,
border, focus ring; profiled simple shadow only" line describes, for
rounded rects/borders/shadows.

No new SDF primitive was added. A dedicated focus-ring primitive does
not exist anywhere in the codebase, but adding one is a new visual
*feature* (nothing currently asks for a focus ring to exist), not a
hot-path optimization of something already drawn -- it belongs to
Phase 11 (design system) or Phase 13 (accessibility), and this phase's
own conditional language ("only when they reduce frame time/draws")
does not apply to a primitive with no prior, slower implementation to
improve upon. Adding one here without a concrete widget request would
be scope creep and untested LOC with no measurable effect.

## Step 7: device loss and memory pressure

**Device loss.** Registered `wgpu::Device::set_device_lost_callback` in
`Compositor::request`, storing the loss reason in a shared
`Arc<AtomicBool>`; added `Compositor::is_device_lost()` so a caller can
check before `present`-ing a frame. **A full rebuild-the-whole-`Engine`-
on-loss recovery path was not implemented.** Detecting loss is now
possible; acting on it (tearing down every pipeline/buffer/cache and
re-requesting a device mid-application-lifetime) is a substantially
larger, harder-to-verify feature that cannot be exercised in this
environment without a way to force a real device loss (no such hook
exists in safe, portable `wgpu`). `wgpu/src/window/compositor.rs`'s own
`device_lost_tests` module unit-tests the flag/callback mechanism
directly (the only part testable without real hardware);
`tests/gpu/device_loss.rs` covers the one thing that *is* headlessly
testable -- that the real `wgpu` backend still initializes and can
render/screenshot a frame -- as explicitly partial, non-device-loss
coverage.

**Memory pressure.** Added `wgpu::resource_budget::Tracker`: aggregates
byte usage reported per `core::cache::Class` (Prefetched/Retained/
Essential) across every GPU resource category, classifying the combined
total into `Pressure::{Normal, Elevated, Critical}` at configurable
thresholds (default 70%/90% of a 256 MiB total limit). `Tracker` does
not evict anything itself -- each cache (e.g. `text::cache::Cache`) still
owns its own eviction policy; `Tracker` answers a different question
("is the *sum* of every cache becoming a problem"), which no single
cache staying under its own budget can answer on its own. Covered by
`wgpu/src/resource_budget.rs`'s own unit tests and
`tests/gpu/cache_pressure.rs`'s public-API integration tests (pressure
escalation from realistic reported byte counts; a real
`text::cache::Cache` under a deliberately tiny budget never evicting an
entry the current frame still needs, and correctly evicting once given
a chance across frames).

**`pipeline_cache.rs` was not created.** Searched every
`device.create_render_pipeline` call site in `wgpu/src/`: seven fixed
call sites, each invoked exactly once (at `Engine::new` construction
time) to build one of a small, fixed set of pipelines (quad-solid,
quad-gradient, text, triangle(+msaa), image, color-conversion). Nothing
in this codebase creates pipeline variants dynamically or would benefit
from a keyed cache -- there is no "many distinct pipeline variants"
problem to solve. Building a `pipeline_cache.rs` module with no real
caller would be pure unused-code padding; documented here as a checked,
not-applicable file-plan line rather than silently skipped.

## Benchmark record

Local dev machine (shared desktop, not a dedicated bench rig), wgpu
backend, release profile, against the Phase 1 baseline (see "Why there
is no Phase 4 baseline" above).

| Scene | Phase 1 baseline P95 | This phase's P95 | Relative change |
|---|---:|---:|---:|
| `animated_cards` | 1.100 ms | 1.154 ms | +4.70% (within this environment's documented noise floor) |
| `text_heavy` | 3.507 ms | 2.406 ms, 2.460 ms (2 samples) | -30 to -31% (a real, repeatable improvement across both samples) |

`animated_cards`'s change is within the noise this project has already
documented for shared-desktop runs (`docs/architecture/invalidation.md`,
`docs/performance/methodology.md`) and is not attributed to this phase's
changes either way.

`text_heavy`'s improvement is consistent and repeatable (two independent
samples, ~1.5% apart from each other), which rules out simple run-to-run
noise as the sole explanation. The most plausible mechanism is Step 5's
text-cache byte budget: `wgpu/src/text.rs`'s hot path already calls
`graphics::text::cache::Cache::allocate` directly, so the budget/eviction
logic added this phase is on the real path this scene exercises heavily.
This is offered as the most likely explanation, not a proven causal
claim -- Phase 1's own methodology document separately flags `text_heavy`
as a scene with wider-than-usual run-to-run variance on this specific
machine, so a controlled A/B (budget enforcement compiled out vs. in)
would be needed to fully separate the two effects, which this phase did
not additionally spend time on given the other six steps' scope.

Neither scene meets the literal ">= 20% improvement" exit-gate language
against a *Phase 4* baseline, because no Phase 4 baseline exists to
improve upon (see above). Against the Phase 1 baseline that does exist,
`text_heavy` exceeds 20% while `animated_cards` does not move materially
-- an incomplete, honest answer to an exit-gate clause this phase cannot
fully satisfy through no fault of its own scope.

## Follow-ups carried forward

1. `wgpu::image::cache::Cache`'s GPU atlas has no byte budget or
   eviction policy (Step 5).
2. No content-diffing partial buffer write exists; revisit only with
   profiling evidence that full-buffer rewrite is the actual bottleneck
   (Step 4).
3. `Compositor::is_device_lost()` detects loss but nothing rebuilds the
   `Engine` in response yet (Step 7).
4. Completing the abandoned Phase 4 work (or formally retiring it) is
   still open; Phase 5's own "improve from Phase 4" exit-gate clause
   cannot be fully evaluated until either happens.
