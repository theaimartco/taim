# TAIM.rs native flex allocator — engineering and provenance audit

Date: 2026-10-20
Branch: `feature/taim-native-flex-allocation-20261020`
Parent: `feature/taim-native-wrapping-layout-20261020`
Original Iced fork checkpoint: `6db551046abd02eadee9f6bbcfa103bc4a9fadd5`
Status: **source implemented / uncompiled / release NO-GO**

## What changed in live code

1. Added `core/src/layout/flex/allocator.rs`: a pure, deterministic
   f64-intermediate weighted water-fill allocator with f32 outputs.
   The planner computes per-child minimums, weighted surplus and
   finite maximum caps in O(n log n) (cap sorting) with O(n) memory.
   Children do not execute layout inside this arithmetic kernel.
2. Modified the actual `core/src/layout/flex/engine.rs` resolver.
   Static and cross-axis measurement remains compatible with prior
   staging, but the old iterative max/min and fluid-distribution
   passes are replaced. `Length::Fluid(Constraint::Max)` children
   receive one intrinsic-capped measurement and release unused budget
   to the remaining fill children.
3. Updated `core/src/layout/flex.rs` with an accurate derivative
   source notice. **Kept** xi-editor/druid Apache 2.0 license and
   historical provenance; the remaining static/cross-axis solver is
   not claimed to be clean-room authored.
4. In `widget/src/column/placement.rs`, corrected wrapping row and
   column extent accumulation: only add gutters between groups.
   Before, subtracting a terminal gutter after f32 saturation could
   reduce the reported extent to zero.

## Regression cases committed, NOT executed

Eleven allocator unit cases cover deterministic weighted sharing,
maximum saturation, minimum floor, oversubscribed minimums, mixed
bounds, zero weight, zero/NaN budgets, unbounded requests, intrinsic
cap classification, malformed constraints, child-order permutation
invariance and very large finite budgets.

One additional wrapping regression covers enormous intergroup spacing
without overflowing or canceling intrinsic extents.

## Compatibility and remaining blockers

- Full standard `Row`/`Column` resolution now uses the new
  allocation stage, but the parent widget lifecycle, cache, static
  and cross-axis measurement stages remain inherited or partially
  derivative. No complete clean-room claim.
- Minimum-space overcommit intentionally allocates proportional
  shortages rather than privileging whichever child appeared first;
  review semantic compatibility with existing applications.
- This change is on a **development branch**, not main. The 149
  premium vector icon assets previously pushed to main are untouched.
- Exact Git blob inequality is not proof of original code; the
  semantic/token source-lineage gate remains a manual release blocker.
- CI workflow files remain commented (owner request). No compiler,
  rustfmt, Clippy, Naga or test runner was executed in this environment.

## Manual acceptance / NO-GO

```bash
cargo fmt --all -- --check
cargo test -p taim_core layout::flex::allocator::tests --locked
cargo test -p taim_widget --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
python3 scripts/qa/verify-taim.py
```

Require additional Row/Column golden snapshots for fill/fill-portion,
fit, bounded min/max, minimum overcommit, content-limited fluid,
nonzero padding, RTL, DPI scaling, long text and nested scroll regions.
Run Wayland/VYOM, macOS, Windows and WebAssembly device/target tests
and reproduce p50/p95/p99 frame times, latency, CPU and memory using
the same physical hardware against relevant alternatives.

**Release decision: NOT PRODUCTION READY. 100% independently
engineered TAIM.rs has NOT been demonstrated.**
