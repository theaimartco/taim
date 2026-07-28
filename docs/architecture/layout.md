# Layout: measure/place contracts, invalidation, and virtualization

> Status: Phase 7 baseline. This document describes the measure/place
> contracts `core::layout` now exposes (constraints hashing, a bounded
> measure cache, baseline/transform/clip metadata on `Node`), which
> production containers actually use them today, and how
> `widget::VirtualList`/`widget::sliver` keep large collections
> proportional to visible content rather than total content.

## The measure/place contracts

Three additions to `core::layout` exist independently of any one
container adopting them, mirroring how Phase 3's `Tree::layout_with_cache`
was introduced as a contract before Phase 7 gave it real production
callers (see `docs/architecture/widget-lifecycle.md`):

1. **`Limits::key() -> LimitsKey`** (`core/src/layout/limits.rs`) --
   a compact, `Hash`-and-`Eq` representation of a `Limits` value. `f32`
   implements neither trait (`NaN != NaN`, and `-0.0`/`0.0` are distinct
   bit patterns that must still compare and hash equal), so this is a
   real normalization step, not a formality: `-0.0` folds to `0.0`,
   `NaN` folds to a single canonical bit pattern. Two `Limits` that are
   `PartialEq` always produce an equal `LimitsKey`, and equal `LimitsKey`s
   are used everywhere a cache needs to key on constraints.

2. **`Node` gained `baseline`, `transform`, and `clip`**
   (`core/src/layout/node.rs`), each `Option<_>`, each independent of
   `Node::bounds()`/`move_to`/`translate` -- a node's baseline/transform/
   clip are defined relative to its own frame and do not shift when the
   node is repositioned within its parent, exactly like `bounds.width`/
   `height` already don't. `flex::resolve` computes a container's
   baseline from its children's own baselines (max, for a row's shared
   text line; only the first child, for a column's first-baseline); nothing
   in this phase computes `transform` automatically (`Stack` attaches a
   `clip` only when it was actually asked to clip -- see "What this
   phase does not close" below for why an identity `transform`
   everywhere was deliberately not added).

3. **`layout::Cache`** (`core/src/layout/cache.rs`) -- a bounded,
   least-recently-used cache of `Node` results, keyed on a `Limits`'s
   `LimitsKey` plus a caller-supplied `cache::Fingerprint` (folding in
   anything content-sensitive the cache cannot see for itself: child
   count, `Length` strategies, spacing). Unlike
   `Tree::layout_with_cache`'s single slot, `Cache` holds several
   entries (default capacity 4), because a real container is not
   measured against one constant `Limits` forever -- a flex solver's own
   multi-pass measurement of the same child, or a resize settling
   between two sizes, both revisit a small, recurring set of `Limits`
   values, and a single-slot cache thrashes to zero reuse under that
   pattern.

## Which containers actually use the cache

`Row`, `Column`, and `Stack` (`widget/src/{row,column,stack}.rs`) each
hold a `layout::Cache` in their own `Tree::state` (via `tag`/`state`) and
call `layout::flex::resolve_cached`/`Cache::get_or_insert_with` directly
from `Widget::layout`. `Grid` (`widget/src/grid.rs`) caches something
narrower -- its `Track` (cells-per-row, cell size, row count), not a full
`Node` -- since every cell is still measured individually regardless
(only the *geometry the cells are placed into* is reused).

**The cache-clearing rule, and why it is conservative:** every one of
these containers clears its cache unconditionally in `diff`, not only
when its child count changes. This is deliberate, not an oversight --
`Widget::diff` has no way, today, to report "this specific child's
content changed in a way that affects its measured size." (`Tree`'s own
single-slot cache has the identical gap; see its module documentation.)
Until a widget's `diff` can report that, the only sound choice is to
treat *any* `diff` pass as a potential invalidation. This still leaves a
real, provable win: `UserInterface::relayout` re-measures the whole tree
on events that don't run `diff` at all (a resize, a redraw with no state
change), and that path hits the cache cleanly --
`tests/layout/incremental.rs`'s
`a_real_row_reuses_its_cache_across_layout_calls_with_no_diff_in_between`
and its `Column`/`Stack`/`Grid` siblings prove exactly this, while
`a_row_recomputes_after_diff_even_under_identical_limits` proves the
conservative side is actually conservative.

## `flex.rs`: what changed, and the license note it required

`core/src/layout/flex.rs` is derived from `druid` (Apache-2.0); this
phase modified it in two ways, both noted in-file per license section
4(b) ("cause any modified files to carry prominent notices"):

1. Per-item bookkeeping (`Meta`, the produced `Node`s) now lives in a
   `SmallVec<[_; 8]>` instead of an unconditional `Vec`, so a container
   with 8 or fewer children -- the overwhelmingly common case --
   allocates nothing for this bookkeeping. The multi-pass
   constraint-solving algorithm itself (the STATIC/CROSS
   FLUID/MIN-MAX/MAIN FLUID/CROSS FLUID DEFERRED passes) is untouched.
2. `resolve_baseline` computes a container's own baseline from its
   already-positioned children, added as a new step after the existing
   ALIGNMENT PASS rather than folded into it.

`resolve_cached` is new, additive API alongside the original `resolve`
(still exported, still used directly by anything that does not want
caching); it is not a breaking change to `resolve`'s own signature or
behavior.

## Virtualization: `VirtualList` and the `Sliver` protocol

**`widget::VirtualList`** (`widget/src/virtual_list.rs`) is a
self-contained scrollable region -- it owns its own scroll offset rather
than expecting to be wrapped in `Scrollable` -- because virtualization's
whole point (bounding live widget count) requires knowing the visible
window *before* any child is built, and `Scrollable` measures its
content at unbounded height. Its `layout` computes a `Positions` table
(closed-form for `ItemHeight::Fixed`, a cumulative `Vec<f32>` for
`ItemHeight::Variable`), derives the visible-plus-`cache_extent` index
range, and only that range's indices are ever passed to the builder
closure -- everything else is neither built, diffed, laid out, nor
painted, which is what
`tests/widgets/virtual_list.rs`'s
`a_hundred_thousand_rows_keeps_a_bounded_live_slot_count` measures
directly (a 100,000-row list's live child count stays proportional to
the viewport, not the row count). Slots are recycled by index across
scroll positions (`State::slots`, matched first by exact index, then by
any available slot) rather than rebuilt from scratch, so a small
back-and-forth scroll does not lose a recycled widget's own state.

**`widget::sliver`** is the protocol layer this list's approach
generalizes to: `SliverConstraints`/`SliverGeometry` let several
scrollable regions (a header, a huge list, a footer) share one scroll
position, each reporting how much space it occupies without needing to
have laid out all of it (`SliverGeometry::scroll_extent` can be
"50,000 pixels" while `paint_extent` covers only the visible window).
`sliver::FixedExtent` is the trivial adapter proving an ordinary
`Widget` can always become a non-virtualized `Sliver`; nothing in this
phase builds a multi-sliver `Scrollable` that actually composes several
`Sliver`s under one shared offset -- that composition is future work
this protocol is designed to support, not something this phase needed to
ship to make `VirtualList` itself correct.

## The nested-scroll fix this phase found

While extending `Scrollable` for a "viewport model" (`Viewport::
visible_bounds`, the content-space rectangle a virtualizing child needs)
and "stable offset" (`operation::scrollable::adjust_offset`, a
non-notifying scroll correction a resizing child can apply on the user's
behalf), a real, pre-existing bug surfaced: a nested `Scrollable` already
at its scroll boundary in the requested direction still called
`shell.capture_event()` for up to 1.5 seconds after its last successful
scroll (the `in_transaction` window), starving its parent of wheel/touch
events it could have used instead. `State::can_scroll` is the fix --
both the `WheelScrolled` and `FingerMoved` handlers now check whether the
requested delta could move anything before capturing, letting an
exhausted nested scrollable release the event to its ancestor. This is
the concrete meaning of "nested-scroll hooks" in this phase's file plan;
it was a correctness fix, not new API surface.

## What this phase does not close

Checked against the phase's own required results:

| Required result | Status | Evidence |
|---|---|---|
| "Common row/column layouts allocate zero heap objects after warm-up for typical child counts" | **Met** | `SmallVec<[_; 8]>` in `flex.rs`; typical child counts (buttons, form fields) stay inline. |
| "Equivalent constraints hash identically" | **Met** | `tests/layout/property.rs`'s `equivalent_limits_hash_identically_across_random_cases`, run across random finite `Limits`. |
| "Layout cache never survives content/font/scale/direction changes incorrectly" | **Partially met** | The cache is conservative by construction (clears on every `diff`, per the rule above) -- it cannot be *wrong*, only less effective than a hypothetical fine-grained invalidation would be. The gap is the same one `Tree::layout_with_cache` already documents: no widget's `diff` reports fine-grained "my measured size changed" yet, so this phase could not build (and did not attempt) a cache that survives a real content change while still correctly invalidating on one. |
| "Virtual list does not instantiate, reconcile, layout, or paint off-window content except the bounded cache extent" | **Met** | `tests/widgets/virtual_list.rs`'s bounded-slot-count and repeated-scroll tests; `VirtualList::layout` never calls `self.builder` outside `Positions::visible_range`'s result. |
| "Focus and semantics can traverse virtualized content through explicit bring-into-view behavior" | **Partially met** | `VirtualList::operate` forwards `Operation`s (including focus-counting/traversal) to every currently-live slot, proven by `tests/widgets/virtual_list.rs`'s `focus_can_traverse_visible_virtualized_rows`. There is no explicit "bring index N into view, building it first if necessary" operation yet -- only already-visible content is reachable; scrolling a specific, currently-off-window index into view programmatically is a follow-up. |

## Known pre-existing gaps this phase's tests surfaced but did not fix

`tests/layout/property.rs` found that `Grid`'s default
`Sizing::AspectRatio` height is computed directly from cell geometry
and routed through neither `Limits::resolve_height` nor any min/max
clamp -- it can legitimately exceed (or fall under) its parent's
`Limits` entirely. This is pre-existing behavior (present before this
phase touched `grid.rs` at all, confirmed via `git log`), consistent
with a grid that is meant to grow with content inside something
scrollable; it is recorded here rather than silently special-cased away,
since fixing `Grid`'s core sizing contract is a larger, separate change
from this phase's actual scope (a track/visible-range cache).

## Follow-ups carried forward

1. Fine-grained per-widget `diff` reporting ("my measured size changed"),
   which would let every container's cache survive real content changes
   without the current conservative clear-on-every-diff rule -- the same
   follow-up `Tree::layout_with_cache` already names.
2. A multi-`Sliver` `Scrollable` that actually composes several
   `Sliver`s (header/list/footer) under one shared offset; today only
   the protocol and a trivial single-widget adapter (`FixedExtent`)
   exist.
3. An explicit "bring off-window index N into view" operation for
   `VirtualList`, building the target row if necessary, for full
   keyboard/assistive-technology navigation through virtualized content.
4. `VirtualList`/`Adaptive` overlay support -- both currently leave
   `Widget::overlay` at its default (`None`) because neither persists
   the `Element`s their builder closures produce across calls, and the
   overlay API requires borrowing from a long-lived `Element`. A future
   revision caching built elements per visible slot could close this.
5. `Grid`'s height-axis sizing contract (see above) -- worth a dedicated
   look, since it affects any consumer relying on `Limits` being
   respected uniformly across containers.
