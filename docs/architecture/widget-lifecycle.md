# Widget lifecycle

> Status: Phase 3 baseline. This document describes the state machine
> `core::widget::tree::{Tree, DirtyFlags, Revision}` now supports and the
> invalidation transitions each Phase 3 mechanism (`layout_with_cache`,
> `hit_test`, `semantics::build`, `invalidation::propagate`,
> `invalidation::Aggregator`) permits. **Every mechanism described here is
> opt-in infrastructure, proven correct in isolation against synthetic or
> directly-constructed trees.** None of it is yet called from
> `runtime::user_interface::UserInterface`'s real per-frame path -- see
> [What Phase 3 does not close](#what-phase-3-does-not-close) for the
> exact, honest gap this leaves against the phase's own exit gate.

## The state machine

A `Tree` node's lifecycle, from creation through however many
diff/layout/paint/semantics passes it survives, until it is either
recreated (tag mismatch) or dropped (removed from its parent):

```text
                    Tree::new / Tree::empty
                             |
                             v
              +---------------------------+
              | DirtyFlags::all()          |
              | (LAYOUT | PAINT | SEMANTICS)|
              | Revision::INITIAL           |
              +---------------------------+
                             |
        +--------------------+--------------------+
        |                    |                     |
        v                    v                     v
  layout_with_cache()  semantics_with_cache()  (nothing yet clears
  clears LAYOUT if      clears SEMANTICS if     PAINT -- see below)
  limits match           computed
        |                    |
        v                    v
  DirtyFlags minus     DirtyFlags minus
  LAYOUT                SEMANTICS
        |                    |
        +--------------------+
                    |
                    v
         Tree::mark_dirty(flags)
        (content/interaction/resize
         changed -- widget or container
         calls this directly)
                    |
                    v
        flags re-inserted, Revision
        advances (wrapping)
                    |
                    v
        invalidation::propagate(root, path)
        marks every ancestor along `path`
        PAINT-dirty too (upward-only;
        never LAYOUT or SEMANTICS)
                    |
                    +--> (loop: cache calls above
                          clear flags again as each
                          is recomputed)
                    |
                    v
        tag mismatch in Tree::diff
                    |
                    v
              Tree::new (fresh node;
              old subtree dropped)
```

Three independent flags, three independent caches, one shared
`Revision` counter that advances on any `mark_dirty` call regardless of
which flags it carries.

## Permitted transitions

| From | To | Trigger | Enforced by |
|---|---|---|---|
| `DirtyFlags::LAYOUT` set | cleared | `Tree::layout_with_cache` computes a fresh `layout::Node` for the current `limits` | `core/src/widget/tree.rs` (`layout_cache_tests`) |
| `DirtyFlags::LAYOUT` clear | still clear | `Tree::layout_with_cache` called again with **identical** `limits` | `layout_cache_tests::a_clean_node_with_identical_limits_reuses_the_cached_result` |
| `DirtyFlags::LAYOUT` clear | set | `Tree::mark_dirty(LAYOUT)`, or different `limits` passed to `layout_with_cache` (implicit: a changed-limits call always recomputes, but does not itself call `mark_dirty` -- it simply does not consult the flag at all when limits differ) | `layout_cache_tests::different_limits_always_recompute_even_when_clean` |
| `DirtyFlags::SEMANTICS` set | cleared | `Tree::semantics_with_cache` computes (possibly to `None`) | `semantics_cache_tests` (5 tests, including the `None`-caching case) |
| `DirtyFlags::SEMANTICS` clear | still clear | `Tree::semantics_with_cache` called again on an unchanged node | `semantics_cache_tests::a_clean_node_reuses_the_cached_result` |
| Any flag clear | set (that flag only) | `Tree::mark_dirty(flags)` | `revision_tests`, `invalidation::propagate_tests` |
| `DirtyFlags::PAINT` clear (an ancestor) | set | `invalidation::propagate(root, path)`, called for every ancestor along `path` | `invalidation::propagate_tests`, `tests/widgets/invalidation.rs` |
| A whole `Tree` node | replaced with `Revision::INITIAL`, `DirtyFlags::all()` | `Tree::diff` observes a tag mismatch | Pre-existing `Tree::diff` behavior, unchanged by Phase 3 |

### Invariants no later phase may violate

1. **A cache never returns a result computed under a different input.**
   `layout_with_cache` checks `limits` equality *and* `DirtyFlags::LAYOUT`;
   satisfying only one is not sufficient for reuse. `semantics_with_cache`
   has no second input to check (semantics do not depend on layout
   constraints), so `DirtyFlags::SEMANTICS` alone gates it -- this is why
   the doubly-optional `cached_semantics: Option<Option<Node>>`
   distinction matters (see below): "never computed" and "computed, no
   semantics" must not collapse into the same state, or a semantics-less
   widget (the common case today) would never actually benefit from the
   cache.
2. **A node's own flag never implies an ancestor's flag.** `propagate`
   only ever inserts `DirtyFlags::PAINT` on ancestors; it is not correct
   to fold `LAYOUT` or `SEMANTICS` upward automatically, because a
   child's resize does not, by itself, prove a parent's own measured
   size or reported semantics changed -- that is each container
   widget's own `Widget::layout`/`Widget::semantics` implementation's
   job to decide (unchanged from Phase 2's `invalidation.md`).
3. **`hit_test::hit_test` and `semantics::build` are oracles, not
   caches.** Neither function memoizes anything itself; correctness
   depends only on `Hoverable`/`Semantic` adapters and the underlying
   `Tree` state being asked honestly on every call. A future cache
   built on top of either (Phase 8's spatial index; Phase 13's
   accessibility tree) must be validated against calling these
   functions directly, not assumed correct because it agrees with a
   previous cached answer.
4. **`invalidation::Aggregator` degrades to `Region::Full`, never to
   under-reporting damage.** Exceeding `Aggregator::MAX_REGIONS` or
   recording an explicit `Region::Full` request must always widen the
   result, never narrow it -- an aggregator that silently dropped
   regions past its bound would under-report what needs to be redrawn,
   which is a correctness bug, not merely a performance one.

## A concrete state trace

For a single leaf widget inside a container, across three consecutive
frames:

```text
Frame 1 (first build):
  DirtyFlags::all() on both nodes
  -> layout_with_cache computes for real (LAYOUT cleared)
  -> semantics_with_cache computes for real (SEMANTICS cleared)
  -> PAINT is still set on both (nothing clears it yet -- see below)

Frame 2 (nothing changed):
  -> layout_with_cache: limits unchanged, LAYOUT clear -> reuse, no widget call
  -> semantics_with_cache: SEMANTICS clear -> reuse, no widget call
  -> PAINT: still set; nothing in this phase clears it (see gap below)

Frame 3 (the leaf's content changes):
  -> widget/container calls tree.mark_dirty(DirtyFlags::PAINT) directly
     on the leaf's own Tree (Phase 3 does not do this on the widget's
     behalf; adoption is the caller's responsibility, same as
     Tree::layout_with_cache and Tree::semantics_with_cache themselves)
  -> invalidation::propagate(root, path_to_leaf) marks every ancestor
     PAINT-dirty too
  -> layout_with_cache / semantics_with_cache: unaffected, still clear
     (a paint-only change must not force either to recompute -- proven
     by tests/widgets/invalidation.rs)
```

## What Phase 3 does not close

Phase 3's own exit gate reads: *"changing one leaf in a 10,000-node
synthetic tree lays out/paints only the required path/subtree; clean
frames do no work; dynamic lists preserve state; no hit-test or
semantics mismatch against the linear/full-tree oracle."*

Honestly assessed against the five commits actually landed:

| Clause | Status | Evidence |
|---|---|---|
| "...lays out only the required path/subtree" | **Met** | `tests/layout/incremental.rs`: a 10,000-leaf synthetic tree re-lays-out exactly one leaf after one `mark_dirty(LAYOUT)` call. |
| "...paints only the required path/subtree" | **Not met** | `invalidation::propagate`/`Aggregator` exist and are unit-tested, but nothing in `runtime::user_interface::UserInterface`'s real draw path calls either. There is no test proving a real frame paints less than the whole tree. |
| "clean frames do no work" | **Partially met** | True for the synthetic layout tree's own `layout_with_cache` calls. Not demonstrated for a real `UserInterface::draw`/`update` cycle, since none of `layout_with_cache`, `semantics_with_cache`, `hit_test`, or `invalidation` is called from `user_interface.rs` -- that file was never modified by any of the five commits, despite `phase.md`'s file plan listing it as "Modify in four commits: split reconcile/event/layout/semantics/paint phases; reuse clean results; report counters." |
| "dynamic lists preserve state" | **Met** | `tests/widgets/identity.rs` and `tests/widgets/keyed_reconcile.rs` prove this at both the keyed slot's own level and one level of nesting behind it, across reorder/insert/remove/replace. |
| "no hit-test or semantics mismatch against the linear/full-tree oracle" | **Met, by construction** | `hit_test::hit_test` and `semantics::build` *are* the linear/full-tree oracles this clause refers to; there is nothing faster yet to disagree with them. This clause becomes meaningful again once Phase 8/13 build a real cache to check against these oracles. |

**Why this gap exists, not just that it does:** each of the five
required commits built one *mechanism* (`reconcile`, `layout_with_cache`,
`hit_test`, `semantics::build`, `invalidation::{propagate, Aggregator}`)
and proved it correct against either a synthetic tree or directly
against real widgets bypassed around `UserInterface`. None of the five
commits touched `user_interface.rs` itself. This was a scope decision
made explicitly commit-by-commit (see each commit's message under
"Scope and invariant changed"), following the same "introduce contracts
before behavior" precedent Phase 2 set for its own contracts
(`docs/architecture/invalidation.md`) -- retrofitting `UserInterface`'s
actual `build`/`update`/`draw` methods to call into all five new
mechanisms simultaneously would have been a sixth architectural risk on
top of the five already taken, violating the "one architectural risk
per commit" rule this phase was already spending its full budget on.

**What this means for Phase 2's own follow-up requirement.**
`docs/architecture/invalidation.md`'s trade-off report required "Phase
3's benchmark record must show `idle_shell` P95 at or below the Phase 1
baseline." Since no Phase 3 commit wires its mechanisms into the real
frame path, `idle_shell` has not recovered -- every benchmark record in
commits 1 through 5 shows `idle_shell` still elevated relative to the
Phase 1 baseline (in the same +0.9%-to-+12% range Phase 2's own report
found, modulo this machine's shared-desktop noise). That recovery is
now explicitly **carried forward as a Phase 3 follow-up**, not silently
dropped: a future commit (whether folded into Phase 4's damage-tracking
work or a dedicated `user_interface.rs` change) must wire
`layout_with_cache`/`semantics_with_cache`/`invalidation` into
`UserInterface`'s real `build`/`update`/`draw` before `idle_shell`'s P95
can actually improve, since improvement requires skipping work on clean
frames -- which nothing yet does outside the synthetic test tree.

## Ownership (extending Phase 2's table)

| Type | Owned by | Mutated by |
|---|---|---|
| `Tree::cached_layout` | The `Tree` node itself | Only `Tree::layout_with_cache`; private field |
| `Tree::cached_semantics` | The `Tree` node itself | Only `Tree::semantics_with_cache`; private field |
| `diagnostics::Counters` | Whoever constructs one (nothing in production yet) | `Counters::increment`, called by `invalidation::record_paint` and `Aggregator::record_with_counters` -- both currently only exercised by tests |

Every other row in Phase 2's ownership table (`Tree::revision`/`dirty`,
`Tree::cached_bounds/transform/clip`, `Shell::damage_requests`,
`Shell::is_semantics_invalid`) is unchanged by Phase 3: none of those
fields gained a new mutator this phase, only new *readers*
(`invalidation::Aggregator` reads `damage::Request`s the same shape
`Shell::request_damage` already produced).
