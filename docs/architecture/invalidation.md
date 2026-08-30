# Invalidation

> Status: Phase 2 baseline. This document describes the contracts
> `core::widget::tree::{Revision, DirtyFlags}`, `core::damage`,
> `core::shell::Shell`'s typed requests, and the `Widget` trait's new
> defaulted methods establish. **None of these contracts change observed
> behavior yet** -- every existing widget still gets laid out, painted,
> and hit-tested exactly as it did before this phase, on every frame,
> regardless of what changed. Phase 3 is the first phase that makes any
> caller actually skip work based on what is described here.

## Why introduce contracts before behavior

`phase.md`'s Phase 2 objective is "introduce revisions, identity,
damage, semantics, cache policy, frame context, and diagnostics without
yet changing all behavior." The reasoning, carried over from the
roadmap's own rules: a change that both (a) introduces a new invalidation
contract and (b) makes the runtime rely on it for correctness is two
architectural risks in one commit, and rule 5 asks for one per commit,
independently benchmarked. Phase 2 ships (a) alone. Phase 3 ships (b),
against a contract that is already stable and already has its own tests
(`tests/widgets/identity.rs`, `tests/widgets/dirty_flags.rs`) written
against a full-recompute runtime, so a Phase 3 regression is visible as
"the fast path disagrees with the already-correct slow path," not as
"something about dirty tracking is subtly wrong from day one."

## Ownership

| Type | Owned by | Mutated by |
|---|---|---|
| `Tree::revision` / `Tree::dirty` | The `Tree` node itself | Only `Tree::mark_dirty`/`Tree::clear_dirty`; no other code writes these fields directly (they are private) |
| `Tree::cached_bounds/transform/clip` | The `Tree` node itself | Only `Tree::set_cached_*`; nothing calls these yet |
| `Shell::damage_requests` | The `Shell` for the current event/frame | `Shell::request_damage`; drained by nothing yet |
| `Shell::is_semantics_invalid` | The `Shell` for the current event/frame | `Shell::invalidate_semantics`; read by nothing yet |
| `Widget::semantics`/`paint_bounds`/`hit_test`/`cache_policy` | Each widget's own implementation (or the default) | Never mutated; these are pure queries |

A `Tree` node's `Revision` and `DirtyFlags` describe *that node only*.
Nothing in Phase 2 aggregates a subtree's dirty state into its root; see
[Propagation](#propagation) for why that is a future phase's job, not a
built-in guarantee.

## State transitions

### `Revision`

```text
Tree::new / Tree::empty  ->  Revision::INITIAL
        |
        | Tree::mark_dirty(flags)
        v
  Revision::next()  (wrapping)
```

`Revision` uses **generation replacement**, not a wall-clock timestamp: it
only moves forward via `Tree::mark_dirty`, and it resets to
`Revision::INITIAL` whenever the node itself is recreated (a tag
mismatch in `Tree::diff` replaces the whole node, including its
revision). Comparing two `Revision`s is only meaningful for the *same*
node's lifetime -- comparing revisions from two different nodes, or from
a node before and after it was recreated, is a use-after-reset bug the
type system cannot catch. `Revision` deliberately does not implement
`Ord`/`PartialOrd` so a caller cannot reach for `<`/`>` by accident;
`Revision::wrapping_after` is the only supported comparison, and it
handles one wraparound of the underlying `u64` correctly (chosen because
a plain numeric comparison silently breaks the frame right after a
wraparound, and while a `u64` wrapping in practice requires billions of
`mark_dirty` calls on one node, "practically never" is not the same
invariant as "never," so the wrapping-safe path costs nothing to always
use).

### `DirtyFlags`

```text
Tree::new / Tree::empty  ->  DirtyFlags::all()   (fully dirty; nothing has run yet)
        |
        | Tree::clear_dirty(flags)      (a phase proved its result is reusable)
        v
   flags removed
        |
        | Tree::mark_dirty(flags)       (something changed again)
        v
   flags re-inserted, Revision advances
```

Every fresh node starts fully dirty (`LAYOUT | PAINT | SEMANTICS`): a
node that has never been laid out, painted, or had its semantics
computed cannot be anything but dirty in all three respects. There is no
external code that clears these flags yet -- that is exactly the "without
yet changing all behavior" boundary of this phase. When Phase 3 adds real
incremental layout, it is expected to call `Tree::clear_dirty(LAYOUT)`
only at the point it can prove the cached bounds are still valid (same
constraints, same revision-independent inputs); Phase 13 does the same
for `SEMANTICS`.

## Propagation

Phase 2 does not propagate a child's dirty state to its parent
automatically -- there is no code in `Tree::diff`,
`Tree::diff_children`, or anywhere else that reads a child's
`DirtyFlags` and folds it into the parent's. This is deliberate:
propagation is a policy decision (design constraint: "dirty propagation
stops as early as correctness allows"), and the correct stopping point
depends on *what kind of container* the parent is -- a `Stack` that
paints children back-to-front needs different propagation logic than a
`Scrollable` that can skip re-laying-out off-screen children entirely.
Baking one propagation policy into `Tree` itself in Phase 2 would either
be wrong for some container or would have to be so conservative
("always propagate everything") that it provides no value. Phase 3's
`runtime::invalidation` is where a real, container-aware propagation
policy is expected to live.

`tests/widgets/dirty_flags.rs` demonstrates the *shape* a propagation
rule will take (fold children's `DirtyFlags` with a bitwise OR, call
`mark_dirty` on the parent only if the result is non-empty) without
committing the framework to it being the actual rule Phase 3 ships.

## The compatibility shim

`core::shell::Shell` already had a coarse invalidation API before this
phase: `invalidate_layout`/`invalidate_layout_with`/`invalidate_widgets`/
`request_redraw`/`request_redraw_at`. Phase 2 adds typed,
narrower-grained companions --
`Shell::request_damage`/`Shell::damage_requests`/
`Shell::invalidate_semantics`/`Shell::is_semantics_invalid` -- without
removing or changing the meaning of the coarse ones.

`Shell::request_damage` is the concrete instance of the design
constraint "the compatibility redraw API maps conservatively to full
damage until Phase 3": calling it both records the typed
`damage::Request` (for a future consumer to read) *and* calls the
existing `Shell::request_redraw`, so a widget that starts calling
`request_damage` today gets exactly the same runtime behavior it would
get from `request_redraw` -- a full redraw -- while also leaving a
precise record of what actually changed for Phase 4 to switch over to
once real damage-region tracking exists. No widget in this codebase
calls `request_damage` yet; introducing the method is Phase 2's job,
adopting it widget-by-widget is not.

## Invariants a later phase must preserve

1. A `Tree` node's `Revision` only moves forward (mod wraparound) for
   the lifetime of that node; recreating the node (tag mismatch) is the
   only way it resets.
2. A fresh `Tree` node is always fully dirty. No code may construct a
   `Tree` with a subset of `DirtyFlags` already clear.
3. `Shell::request_damage` must keep triggering at least as much
   invalidation as `Shell::request_redraw` alone would, for as long as
   any consumer of `Shell::damage_requests` remains conservative
   (full-redraw fallback) -- this stops being required only once a real
   damage-region consumer exists and is proven correct (Phase 4's exit
   gate: "damage golden tests match full redraw").
4. `Widget::paint_bounds`, `Widget::hit_test`, `Widget::semantics`, and
   `Widget::cache_policy` are pure queries: none of them may mutate the
   `Tree` they are given, and none may be assumed to be called on every
   frame (a caching consumer may skip calling them when a node's
   relevant `DirtyFlags` bit is already clear).
5. `core::semantics::Node` for a `Role::SecureField` must never have its
   `value` populated by widget code (see `phase.md` Phase 10's security
   requirements); Phase 2 only defines the type, Phase 10/13 are
   responsible for the actual enforcement once widgets produce real
   `Node`s.

## Trade-off report: `Tree` size and P95

`phase.md` section 3 requires an approved trade-off report for any phase
that regresses P95 by more than 3% without one. Phase 2 does.

**What changed.** `core::widget::tree::Tree` grew from 56 bytes to 192
bytes (3.4x) on this platform, from adding `revision: Revision` (8
bytes), `dirty: DirtyFlags` (1 byte, padded), and three
`Option<Rectangle>`/`Option<Transformation>` cached-metadata fields (the
bulk of the growth: an `Option<Rectangle>` cannot use a niche
optimization the way `Option<&T>` can, since `f32` has no spare bit
pattern reserved for `None`, so each costs a full `Rectangle` plus a
discriminant, rounded up for alignment).

**Measured impact.** Comparing `taim_bench`'s `idle_shell` and
`ten_thousand_rows` scenes against the Phase 1 baseline
(`benchmarks/baseline/*.json`), both scenes that build/diff many small
`Tree` nodes per frame, on `dev-workstation-01` (same unpinned-governor
caveats as documented in `docs/performance/methodology.md`):

| Scene | Phase 1 baseline P95 | Phase 2 P95 | Relative change |
|---|---:|---:|---:|
| `idle_shell` | 1.030 ms | 1.106 ms (3-sample average) | +7.3% |
| `ten_thousand_rows` | 1.201 ms | 1.181 ms | -1.7% (within noise) |
| `image_grid` | 1.150 ms | 1.133 ms | -1.5% (within noise) |
| `blur_layers` | 1.140 ms | 1.137 ms | -0.3% (within noise) |
| `animated_cards` | 1.100 ms | 1.094 ms | -0.6% (within noise) |
| `multitouch` | 1.113 ms | 1.095 ms | -1.6% (within noise) |

Only `idle_shell` shows a regression outside this environment's
observed noise floor (the other five scenes, including
`ten_thousand_rows`, land within ±2%, consistent with run-to-run
variance already documented in `docs/performance/methodology.md`).
`idle_shell`'s +7.3% is a repeatable, consistent shift across three
samples (1.091ms, 1.116ms, 1.109ms), not the wide single-run swings this
project has already documented as environmental noise for `text_heavy`
in Phase 1. It is attributed to `Tree`'s size increase (more bytes to
allocate, copy, and walk per node in the still-full-recompute widget
tree) rather than any algorithmic change, since Phase 2 introduces no
new control flow in the hot path -- only new fields.

**Why this is accepted rather than reverted.** Phase 2's whole purpose is
to give Phase 3 onward something to cache into; every one of the fields
responsible for the growth (`cached_bounds`, `cached_transform`,
`cached_clip`, `revision`, `dirty`) exists specifically so a later phase
can *stop* doing the full-recompute work this benchmark is measuring
today. Reverting the fields to avoid a 7.3% regression now would remove
the storage Phase 3 depends on, at the cost of Phase 3 having to
re-introduce this exact struct change anyway, just under a different
phase's name and a different commit's scrutiny. The correct place to
recover this regression is Phase 3's exit gate ("clean frames do no
work"), which is expected to eliminate far more than 7.3% of
`idle_shell`'s frame cost once idle frames stop walking the tree at all
-- not Phase 2, which by its own design does not yet skip any work.

**Follow-up required.** Phase 3's benchmark record must show
`idle_shell` P95 at or below the Phase 1 baseline (not merely below
Phase 2's regressed number), since an incremental-invalidation phase
that only clears its own overhead without producing a net improvement
has not demonstrated the payoff this trade-off assumes. If Phase 3
cannot recover it, that is a signal Phase 2's cached-metadata fields
need to be revisited (e.g. boxing the `Option` trio behind a single
`Box<CachedMetadata>` allocated only once a node actually populates
them, trading an indirection for a smaller
common-case `Tree`), not something to carry forward silently.
