# Frame scheduling

> Status: Phase 6 baseline. This document describes
> `runtime::frame_scheduler::Scheduler`'s state machine, who owns the
> monotonic clock it runs against, and how far real backend wiring
> (`winit`'s event loop, `wgpu`'s compositor) goes today versus what
> remains a documented follow-up.

## Clock ownership

Exactly one clock exists per running application: `std::time::Instant`
(re-exported, on native targets, unchanged, as `taim_core::time::Instant`
via `web_time`). `Scheduler` never calls `Instant::now()` itself --
every method that needs "now" takes it as an explicit parameter. This is
not a style preference; it is the whole reason `Scheduler`'s 17+
internal unit tests and the 16 tests in `tests/runtime/{frame_scheduler,
idle,occlusion}.rs` can assert exact state-machine outcomes at all: a
fake, monotonically-offset clock (`epoch() + Duration::from_millis(n)`)
drives every test deterministically, with no flakiness from real wall-
clock timing.

In the live `winit` event loop, `Instant::now()` is called at exactly
three points per window per iteration (`Window::request_redraw`,
`WindowEvent`'s `NewEvents(ResumeTimeReached)` arm, and around
`Compositor::present`), each immediately handed to the relevant
`Scheduler` method. No other code path reads the clock on `Scheduler`'s
behalf -- "exactly one monotonic framework clock owns animation time"
(`phase.md`) means this narrow set of call sites, not a clock threaded
through every widget.

## The state machine

`runtime/src/frame_scheduler.rs`'s own module documentation includes the
full state diagram (`Idle -> Requested -> Animating`, `Submitted`,
`Suspended` pre-empting all of them). The short version: a widget's
`Shell::request_redraw_at`/`Shell::request_animation` calls become
`Scheduler::request_redraw_at`/`Scheduler::request_animation` calls (one
`Scheduler` per window); `WindowEvent::Occluded` and
`compositor::SurfaceError::Occluded` both call `Scheduler::suspend`/
`resume`; a successful `present` calls `Scheduler::submit` then
`Scheduler::feedback_received` (see
[Presentation feedback](#presentation-feedback) for why those two calls
are adjacent rather than separated by a real async wait); every event-
loop iteration calls `Scheduler::tick` so a frame stuck in `Submitted`
recovers after `FEEDBACK_TIMEOUT` (200ms) even if nothing ever reports
feedback for it.

"Widgets never create independent render loops" is enforced by
construction, not by a lint: nothing in this codebase gives a widget
access to a timer or an event-loop handle of its own. The only way a
widget can ask for continuous frames is `Shell::request_animation`,
which sets one boolean per `Shell` (merged across nested shells the
same way `is_semantics_invalid`/`damage_requests` already are), read
back by `UserInterface::update`'s `State::Updated::is_animating` field
and turned into exactly one `Scheduler::request_animation`/
`stop_animating` call per window per update.

## Coalescing

Two independent coalescing mechanisms compose:

1. `window::RedrawRequest`'s own `Ord`/`.min()` (pre-existing, Phase 2):
   many widgets each calling `Shell::request_redraw_at` in the same
   `update` pass collapse to the single earliest `RedrawRequest` by the
   time `UserInterface::update` returns.
2. `Scheduler::request_redraw_at`'s own coalescing: the *scheduler's*
   pending deadline is likewise the minimum of every deadline it has
   been asked for since the last frame, and a request arriving while
   `Animating` or `Submitted` is a no-op (both already imply "a frame is
   coming" at least as soon as any new one-shot request could ask for).

The result: however many widgets ask for a redraw, and however many
times `UserInterface::update` runs before a frame is actually produced,
exactly one deadline reaches the event loop's `ControlFlow::WaitUntil`
computation (`window::Manager::redraw_at`, which now reads
`Scheduler::deadline()` per window rather than a raw `Option<Instant>`).

## Occlusion and suspension

Two independent occlusion signals both reach `Scheduler::suspend`:
`winit::event::WindowEvent::Occluded(true)` (platform-reported, not
every platform supports this) and
`wgpu::window::compositor::SurfaceError::Occluded` (backend-reported,
observed when a `present` call itself fails because the surface turned
out to be occluded). Either one stops `Scheduler::should_produce_frame`
from ever returning `true` for that window, regardless of what was
pending, until the corresponding resume signal
(`Occluded(false)`) arrives.

`window::Manager::redraw_at`'s switch from raw `redraw_at: Option<
Instant>` fields to `Scheduler::deadline()` means occlusion also stops
the event loop from waking up early for a suspended window: a suspended
`Scheduler` reports no deadline, so it drops out of the `.min()` across
every window entirely, rather than the loop waking up on schedule only
to discover (again) that the surface is still occluded.

A `RedrawRequested` event that arrives after suspension was processed
(a stale event already in the OS's queue) is explicitly guarded against
in `winit/src/lib.rs`'s handler: `should_produce_frame(now)` is checked
before doing any of the real `update`/`draw`/`present` work for that
event, so occlusion genuinely stops submissions, not merely new
requests.

## Presentation feedback

**This is the least complete part of Phase 6, by necessity.** `wgpu` (at
the version this crate targets) exposes no cross-platform, per-frame
"was this specific frame presented, and when" API -- only
`wgpu::Adapter::get_presentation_timestamp`, a clock-correlation
primitive, not a per-frame report. Real per-frame feedback (Vulkan's
`VK_GOOGLE_display_timing`, DXGI's `GetFrameStatistics`, macOS's
`CVDisplayLink`) is backend- and platform-specific.

`wgpu::window::frame_feedback::FeedbackSource` is the seam a real,
platform-specific source is expected to plug into. Today, exactly one
implementation exists: `Synchronous`, which treats "the blocking
`present` call returned" as "the frame presented," with no `target_error`
computed (it has no independent clock to compare against). This is
correct in spirit -- nothing ever claims a frame presented before its
work was actually submitted -- but cannot detect a frame accepted by the
presentation engine and then displayed late or dropped after `present`
already returned control to the process.

`taim_bench` (Phase 1's benchmark tool) has the same gap one level up:
it runs entirely headlessly with no compositor at all, so
`tools/taim_bench/src/metrics/pacing.rs`'s `PacingSamples` -- built to
consume real `taim_core::frame::Feedback` values -- has nothing real to
consume in this tool today. `main.rs`'s `measure` function synthesizes a
`Feedback`-shaped sample from what it *does* measure (CPU frame time
against the target budget) as the best available proxy, documented
inline at the synthesis site and in `RunSummary::pacing`'s own field
documentation. `PacingSample::from_feedback`/`PacingSamples::
extend_from_feedback` are kept and unit-tested as the real bridge a
future scene driving an actual windowed compositor would use instead,
rather than deleted for being currently unreachable from `taim_bench`
itself.

## What Phase 6 does not close

Checked against the phase's own exit gate ("idle gate passes; animation
cadence has no periodic beat/jitter defect; queue depth stays within
policy; occlusion stops submissions; input-latency improvement does not
create frame instability"):

| Clause | Status | Evidence |
|---|---|---|
| "idle gate passes" | **Met** | `tests/runtime/idle.rs`: a fresh `Scheduler` never produces a frame; a real, unchanging `UserInterface` re-`update`d with no events re-runs no layout and reports `is_animating() == false`. |
| "occlusion stops submissions" | **Met** | `tests/runtime/occlusion.rs` plus the real `winit`/`wgpu` wiring described above (both signal sources tested to reach `Scheduler::suspend`, and a stale `RedrawRequested` is guarded against). |
| "animation cadence has no periodic beat/jitter defect" | **Partially met** | `Scheduler`'s own state machine is proven not to introduce beat/jitter itself (an `Animating` scheduler produces a frame on every tick it is asked about, with no artificial delay). Real-world cadence measurement (`taim_bench`'s new `pacing` metrics) exists but, per the gap above, is currently synthesized from CPU time, not real presentation feedback -- so this clause is verified for the scheduler's own logic, not yet for an actual windowed run's measured cadence. |
| "queue depth stays within policy" | **Partially met** | `wgpu::window::compositor::Settings::desired_maximum_frame_latency` is now a real, configurable field (previously hardcoded to `1`, the lowest-latency value, which remains the default). `PacingSummary::max_queue_depth`/`mean_queue_depth` exist to check this against a real run, but nothing in this phase populates `PacingSample::queue_depth` from a real submission-tracking source yet (it is `0` in every `taim_bench` sample today, since that tool has no real submission queue to observe depth in). |
| "input-latency improvement does not create frame instability" | **Not attempted** | This phase did not lower `desired_maximum_frame_latency` below its already-lowest-latency default of `1`, or otherwise attempt an input-latency optimization to check for a stability regression -- "lower-latency modes require evidence on every supported target GPU/compositor" (`phase.md`), and no such evidence was gathered this phase. Recorded here as explicitly not attempted, not as a failure of an attempted change. |

## Follow-ups carried forward

1. A real, platform-specific `FeedbackSource` (beyond `Synchronous`) --
   the actual per-frame presentation timing this phase's `pacing`
   metrics and `Scheduler`'s `target_error` tracking are designed to
   consume once one exists.
2. `taim_bench` driving a real windowed `Compositor` (rather than only
   the headless `Renderer` path), so `metrics::pacing` measures real
   presentation cadence instead of a CPU-time proxy.
3. An evidence-backed evaluation of lower-latency present modes/
   `desired_maximum_frame_latency` values against real target hardware,
   per `phase.md`'s explicit conditional ("require evidence... before
   lowering").
4. Populating `PacingSample::queue_depth` from a real submission-
   tracking source once (1) and (2) exist.
