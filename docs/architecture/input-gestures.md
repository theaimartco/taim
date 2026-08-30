# Input gestures

> Status: Phase 8 baseline. This document describes the unified pointer
> model (`taim_core::pointer`), the gesture competition engine
> (`taim_gesture::arena`), and the routing layer that connects them to a
> real widget tree (`taim_runtime::{hit_test, pointer_router}`) --
> together, "unified pointer input and gesture arena" (`phase.md`).

## The pointer model

Every input device this codebase supports -- mouse, touch, and
(structurally, though no backend emits it yet) pen/trackpad -- is
normalized into one shape: `taim_core::pointer::Pointer`. A `Pointer`
carries a stable `Id` for its whole contact's lifetime, a `DeviceKind`,
a `Phase` (`Down`/`Move`/`Up`/`Cancel`), a position, a timestamp, and
optional `Metadata` (pressure, tilt, contact size) that most backends
today report as absent (`Metadata::NONE`).

`Id` is stable per contact, not per device: a mouse gets the fixed
`Id::MOUSE` across its entire lifetime (it only ever has one contact at
a time), while each touch finger gets an `Id::from_finger`-derived value
tagged with bit 63, so a mouse `Id` and every possible finger `Id` can
never collide even though they are constructed independently. This is
what lets `taim_gesture::arena::Arena` and
`taim_runtime::pointer_router::Router` key their internal maps on one
`Id` type regardless of which device produced it.

Two existing types feed into this model without duplicating it:

- `taim_core::touch::Event` (unchanged in shape, extended with a
  `metadata` field on every variant) converts directly via
  `Event::to_pointer`, since every touch variant already carries its
  own position.
- `taim_core::mouse::Event` has no `to_pointer` of its own --
  `ButtonPressed`/`ButtonReleased`/`WheelScrolled` carry no coordinates
  (the platform reports the cursor position out-of-band). Instead,
  `taim_core::event::Event::to_pointer(cursor, timestamp)` takes the
  cursor position a caller is already tracking (e.g.
  `UserInterface::update`'s own `mouse::Cursor` parameter) and produces
  a `Pointer` for `ButtonPressed`/`ButtonReleased`/`CursorMoved`;
  `CursorEntered`/`CursorLeft`/`WheelScrolled` have no single position
  to report and return `None`, as do every non-pointer `Event` variant
  (keyboard, window, input method, clipboard, waken).

## Coalescing

`phase.md`'s own correctness rule -- "high-frequency moves may be
coalesced, but down/up/cancel and semantic boundary events may not be
dropped" -- is implemented once, as a pure classification function:
`taim_core::event::Event::coalescing() -> Coalescing`
(`Mergeable`/`Critical`). Only `CursorMoved`, `WheelScrolled`, and
`FingerMoved` are `Mergeable`; every other event (including every
lifecycle boundary: presses, releases, cancellations, cursor
enter/leave) is `Critical`.

`taim_runtime::pointer_router::coalesce` is the consumer: given a
backlog of queued `Pointer`s for the same contact, it collapses
consecutive `Move`s to the latest one while leaving every `Down`/`Up`/
`Cancel` untouched and in order. `Router::route` itself takes events
one at a time -- the routing decision never needs more than one -- so
`coalesce` is meant to run before a caller's queue ever reaches
`Router::route`, not inside it.

## The gesture arena

`taim_gesture::arena::Arena` is a single, bounded gesture competition:
every `Recognizer` registered into it receives an identical event
stream (fanned out by `Arena::dispatch`) and reports a `Disposition`
(`Pending`/`Accepted`/`Rejected`); the arena resolves to at most one
`Resolution` (`Won(EntrantId)`/`NoWinner`/`Cancelled`) via four ordered
rules:

1. **Eager accept wins.** The first `Accepted` disposition resolves the
   arena immediately -- a recognized long-press should not wait for
   every sibling recognizer to also finish deciding.
2. **Same-pass ties break by priority, then registration order.** If
   two entrants accept in the same resolution pass, the higher
   `Priority` wins (`Priority::SYSTEM` for a reserved
   `edge_swipe`-style gesture, `Priority::NORMAL` for everything else);
   a further tie goes to whichever registered first.
3. **Last one standing wins by default.** Once every entrant but one
   has rejected, the remaining entrant wins without itself needing to
   accept -- including the common case of a single registered
   recognizer, which wins the moment it is registered (there is
   nothing left to compete against).
4. **All-rejected resolves with no winner.** If every entrant rejects
   with none ever accepting, the arena resolves `NoWinner`; each
   entrant already received its own `Verdict::Lost` as it individually
   rejected.

A `Team` groups recognizers that must win or lose *together* (e.g.
several recognizers internal to one composite widget); members are each
polled independently, but the team as a whole only rejects once every
member has, and any one member accepting immediately accepts the whole
team. `Team::solo()` is the default every entrant gets unless a caller
explicitly groups it with others.

`Arena::dispatch` only calls `Recognizer::add_pointer` on `Down` --
`on_event` (and thus any disposition) is only invoked from a *following*
event. A lone entrant still resolves on `Down` itself, via rule 3 (there
is exactly one pending team from the moment it is registered); a
multi-entrant competition needs at least one more event before any
disposition exists to resolve against.

`Arena::tick(now)` lets a deadline-based recognizer (`long_press`, most
notably) resolve with no new pointer event at all, mirroring
`runtime::frame_scheduler::Scheduler::tick`'s own pattern from Phase 6:
a caller is expected to tick every still-unresolved arena once per
frame/timer tick, exactly as recognizer's own documentation describes.

### Recognizers shipped this phase

| Recognizer | File | Contract |
|---|---|---|
| `Tap` / `MultiTap` | `tap.rs` | Accepts on release within a slop/timeout window; `MultiTap` chains consecutive taps within an interval and slop of each other. |
| `LongPress` | `long_press.rs` | Accepts once a stationary hold crosses a duration deadline, resolved via `Arena::tick`, not a new event. |
| `Pan` | `pan.rs` | Accepts once movement crosses a slop threshold; locks to a `Free`/`Horizontal`/`Vertical` `AxisLock` the first time it crosses slop, never changed afterward. |
| `Swipe` | `swipe.rs` | Accepts a fast, sufficiently long single-direction movement; exposes `Direction::from_vector` (`pub(crate)`) for reuse by `edge_swipe`. |
| `EdgeSwipe` | `edge_swipe.rs` | A `Swipe` variant starting within a threshold of a screen edge; carries no priority of its own -- a caller registers it with `Priority::SYSTEM` to make it win system-edge ties (see below). |
| `Pinch` | `pinch.rs` | Tracks scale (distance ratio between exactly two pointers) and focal point (their midpoint); rejects if a third pointer joins. |
| `Rotate` | `rotate.rs` | Tracks angle between exactly two pointers via `atan2`, normalized to `(-π, π]`; rejects if a third pointer joins. |

`velocity::Estimator` is shared plumbing, not a recognizer itself: a
`VecDeque<Sample>` bounded by both count (8) and age (100ms), producing
a simple linear displacement/time `Vector` estimate for anything that
needs one (`Swipe`, `Pan`, a future fling).

## System-reserved edge gestures

`phase.md`'s "system-edge gestures remain reserved by policy" is
satisfied entirely at registration time, not inside `EdgeSwipe` itself:
`Arena::register` takes an explicit `Priority`, and rule 2 above means a
`Priority::SYSTEM` entrant wins any same-pass tie against a
`Priority::NORMAL` one regardless of registration order. A caller
wiring up system gestures (e.g. a back-swipe reserved by the platform
shell) registers its `EdgeSwipe` at `Priority::SYSTEM` alongside
ordinary content gestures at `Priority::NORMAL`; the arena's own,
already-tested resolution order is the enforcement mechanism -- there is
no separate "reserved gesture" concept to keep in sync with it.

## Nested scroll handoff

`taim_gesture::nested_scroll::Coordinator` gives each level of a nested
scroll hierarchy (a horizontally-scrolling carousel inside a
vertically-scrolling page) a `dispatch(Delta, allow_overscroll) ->
Consumption` method: it consumes as much of an offered delta as its own
`Extent` allows, and reports the rest as a `remainder`. `dispatch_chain`
routes a delta through a full hierarchy, child first (`chain` ordered
innermost to outermost) -- each level's remainder becomes the next
level's offered delta, exactly matching real touch-scrolling platforms'
pre/post consumption contracts.

This is deliberately the same shape as the real nested-scroll
starvation bug found and fixed in `widget::Scrollable` during Phase 7
(a saturated scrollable kept capturing events via a stale
multi-second "transaction" window even once it had nothing left to do
with them) -- `Coordinator`/`dispatch_chain` is the reusable, tested
protocol that failure mode is avoided by construction: a saturated
level's `dispatch` always returns its *entire* offered delta as
remainder (never a stale partial amount), so a caller chaining several
levels together can never get stuck with a level silently absorbing
input it has no room left to use.

`allow_overscroll` distinguishes an in-progress drag (a saturated level
absorbs the whole delta as rubber-band stretch, reporting zero
remainder so the parent never visibly "scrolls underneath" a child that
appears stopped) from a released fling (no overscroll accumulates that
will never get animated back). `Coordinator::settle_overscroll` snaps a
level back to its valid range once such a drag ends.

## Routing: hit-testing, capture, and the pointer router

### The spatial index

`taim_runtime::hit_test::hit_test` is the correctness oracle from
Phase 3: an unmodified, linear (depth-first, `O(nodes)`) walk that
finds the deepest descendant containing a point, returning a `Path` of
child indices. `Index` (this phase) is the real spatial structure
`phase.md` calls for: a uniform grid over a layout snapshot's leaf
bounds (`Hoverable::bounds`, a new trait method), built once per layout
pass via `Index::build`, queried per pointer event without re-walking
the whole tree.

`Index` is not an independently-trusted implementation checked against
the oracle after the fact -- it performs the *identical* recursive
descent (`descend`) as the oracle, using the grid only to decide
whether a subtree has any candidate leaf at all
(`subtree_has_candidate`) before bothering to enter it. This makes
`Index` provably as correct as the oracle by construction (same
algorithm, pruned branches), not a separately-trusted heuristic that
merely happens to agree in every test case exercised so far. An earlier
design (a single flat "highest-priority leaf across the whole grid"
comparison) was discarded during this phase specifically because it
could not make this guarantee: a leaf's *bounds* containing a point
does not imply every ancestor's real `contains` check also passes, and
priority is decided per-sibling-group by the oracle, not globally
across the whole tree.

`Hoverable::bounds` carries a documented invariant: it must be a
superset of wherever `Hoverable::contains` can ever return `true`. A
widget with a *smaller* effective hit area than its reported bounds (a
circular button, a scrollable clipping its content) satisfies this
trivially; a widget whose `contains` somehow accepted a point *outside*
its own reported bounds would let `Index` unsoundly prune a real hit --
every widget's default `hit_test` (a rectangular bounds check, Phase 2)
satisfies the invariant by construction, since it checks these exact
same bounds.

### The pointer router

`taim_runtime::pointer_router::Router` is the piece that drives an
`Arena` per contact across many frames and many concurrent pointers:

- **Capture table.** Once an entrant's arena resolves
  (`Resolution::Won`), `Router` remembers which `Path` owns that
  `Pointer::id` for the rest of its lifetime. Every subsequent event for
  that pointer routes directly to the capture (`Routing::Captured`),
  skipping `Index::hit_test` and arena competition entirely -- the
  concrete mechanism behind "captured pointers keep routing outside
  original bounds": there is no bounds check left to fail once
  captured, since capture is a table lookup keyed on `Pointer::id`, not
  a fresh hit test at the new position.
- **Arena routing.** `Router::start` begins tracking a new contact at
  a hit-tested `Path` with a caller-supplied `Arena`, called on a `Down`
  with no existing capture.
- **Terminal cleanup.** `Up`/`Cancel` remove the contact from `Router`
  after routing, so a reused `Id` (a mouse's fixed `Id::MOUSE` across
  separate press/release cycles) is treated as a genuinely new contact
  on its next `Down` rather than tripping `Router::start`'s
  already-tracked assertion.
- **Deadline ticking.** `Router::tick(now)` advances every still-
  competing arena via `Arena::tick`, so a `LongPress` resolves without
  a new pointer event, exactly as `Arena::tick` itself is documented to
  require a periodic external caller.

`UserInterface::route_pointer` is the integration point into the
existing per-frame update loop: it converts one `Event` to a `Pointer`
(via `Event::to_pointer`, given the caller's tracked cursor position and
timestamp), routes it through the `UserInterface`'s own `Router`, and --
on an unrecognized `Down` -- calls a caller-supplied closure to hit-test
and build a fresh `Arena` only when one is actually needed. This is
deliberately independent of `UserInterface::update`'s existing, full-
tree `Widget::update` traversal: routing's own cost never depends on
the widget tree's size (an `Index` lookup plus a `Router` map lookup),
which is what satisfies "integrate route without full-tree traversal"
literally, rather than by changing what `update` itself does. Widgets do
not yet expose "give me your recognizers" the way they expose
`hit_test`/`update`; wiring a real widget's registered gestures into the
`Arena` a caller builds on `Down` is a larger, separate `Widget` API
question this phase deliberately leaves open rather than solving
implicitly as a side effect of routing plumbing.

## What Phase 8 does not close

Checked against the phase's own exit gate ("recorded streams are
deterministic; spatial index always agrees with exhaustive hit testing;
nested scroll has no stuck capture; gesture latency improves without
missing critical events; system-edge gestures remain reserved by
policy"):

| Clause | Status | Evidence |
|---|---|---|
| "spatial index always agrees with exhaustive hit testing" | **Met** | `hit_test.rs`'s own unit tests (a several-hundred-query-point stress test over a 20x20 leaf grid) plus `tests/events/spatial_hit_test.rs` (overlapping siblings, dense rows, nested two-level trees, a smaller custom cell size, all checked point-by-point against the oracle over real widget layouts). |
| "recorded streams are deterministic" | **Met** | `tests/gestures/recorded_streams.rs`: every scenario replays a fixed, hand-authored event stream through two independent `Router`s and asserts identical `Routing` sequences, across single-contact, mixed mouse/touch, five-finger multitouch, cancellation, and tick-driven long-press scenarios. |
| "nested scroll has no stuck capture" | **Met** | `tests/gestures/nested_scroll.rs`: a three-level hierarchy, a middle level at its own limit passing the whole delta through, a simulated decaying fling proven to drain to exactly zero remainder and stay there, overscroll proven not to leak into a parent, and a mid-gesture resize proven to reclamp rather than leave a dangling offset. |
| "system-edge gestures remain reserved by policy" | **Met** | `tests/gestures/arena.rs::a_system_priority_entrant_wins_a_same_pass_tie_over_a_normal_one`, proving `Priority::SYSTEM` wins a same-pass tie regardless of registration order -- the mechanism `EdgeSwipe` callers are documented to rely on. |
| "gesture latency improves without missing critical events" | **Partially met** | `Event::coalescing`/`pointer_router::coalesce` implement the "never drop down/up/cancel/boundary events" half exactly, with tests proving critical events survive coalescing unmerged. The "latency improves" half is not independently measured this phase -- no `taim_bench` scene exercises the new `Router`/`Index` path yet (the existing `multitouch` scene, from Phase 3, still measures only raw dispatch cost with no gesture arena involved), so there is no recorded before/after latency number to point to. |

## Follow-ups carried forward

1. A `Widget` API for exposing which recognizers a widget/subtree wants
   registered into an `Arena` on `Down` -- today, `UserInterface::
   route_pointer`'s `start` closure is caller-supplied plumbing, not
   something driven automatically from the widget tree itself.
2. An `taim_bench` scene exercising `Router`/`Index`/`Arena` together
   (rather than only raw dispatch), to get a real, measured before/
   after latency number for the "gesture latency improves" exit-gate
   clause.
3. A real backend (`winit`) source for pen/trackpad `DeviceKind`
   events and non-`Metadata::NONE` pressure/tilt data -- the pointer
   model already has a place for it (`pointer::DeviceKind::{Pen,
   Trackpad}`, `pointer::Metadata`), but no backend constructs one
   today.
4. Prediction (interpolating a pointer's likely next position ahead of
   the next real event) stays out of scope entirely this phase, per
   `phase.md`'s own conditional: it must stay behind an experimental
   feature until wrong-prediction correction is visually proven, and no
   such proof was attempted here.
