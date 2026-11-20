# TAIM native Row and Column wrapping placement

Date: 2026-10-20
Branch: `feature/taim-native-wrapping-layout-20261020`
Latest implementation checkpoint: `08a3f432dfa784060480e662481628ccbc90d9a0`
Historical Iced baseline: `81afda72f374386036a5b96a55f57a3dcfb0b653`
Release decision: **NO-GO**

## Substantive implementation

Old inherited Row/Column wrapping implementations determined the end
of a wrapped group by inspecting the next laid-out child's X/Y position
against zero. With nonzero padding, this test was not a reliable group
boundary. It could also miss the final group or shift alignment on the
wrong range when child dimensions varied.

The new TAIM-owned arrangement code lives in
`widget/src/column/placement.rs`. Both
`widget/src/column.rs` and `widget/src/row.rs` call it directly
from their actual `Wrapping::layout` methods.

The planner measures children once, then constructs explicit
contiguous RowGroup/Group ranges, calculates per-group main-axis extent,
cross-axis extent and offset, and finally positions all nodes. It
handles:
- Existing widget order and one measurement call per child.
- Separate within-group and cross-group alignment.
- Nonzero padding without altering wrap thresholds.
- Oversized first child without producing a ghost/empty group.
- Zero-child and unbounded-axis cases.
- NaN/Infinity spacing and adversarial f32 extents, with saturating
  positive position arithmetic to avoid infinite coordinates.

Five Column and four Row/numeric-boundary regression functions were
added. Tests are source additions, not passing test results; Rust/Cargo
were unavailable in this runtime.

## Source integrity finding

The earlier full Git SHA audit has 0 exact Iced source blobs at any path.
That alone does not prove independent implementation or justify
removing origin attribution. A bounded same-path token-multiset spot
comparison **after this wrapping refactor** still finds:

| Source file | Approximate shared-token multiset overlap |
| --- | ---: |
| `widget/src/column.rs` | 90.0% |
| `widget/src/row.rs` | 90.4% |

The overlap is high because regular layout and widget lifecycle methods
are still similar to inherited code. The shared multi-pass
`core/src/layout/flex.rs` remains documented as originally inspired
by druid (Apache 2.0 lineage). Shared functionality is not by itself
a copying verdict, but must be reviewed. Distinct algorithms and
actual complete source-lineage audit are required for a credible
100% TAIM-native implementation claim. Original source licenses,
CREDITS.md and provenance must be retained.

## Required verification (currently UNEXECUTED)

```bash
python3 scripts/qa/verify-taim.py
cargo test -p taim_widget --all-features --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
python3 scripts/audit/source-lineage-gate.py --strict --output /tmp/taim-source-gate.json
```

Plus real screenshot golden tests comparing Row/Column with no children,
oversized children, mixed fixed/fill sizing, multi-row/group layouts,
nonzero padding, large content, IME, a11y, and VYOM/Wayland behavior.
Do not assert speed improvement without benchmark evidence. The
nine new unit tests are not a replacement for a full native
system/device/release suite. CI remains commented at owner's request.

**Status: independently implemented wrapping logic, NOT 100% TAIM
framework, NOT compiled/certified, do not merge main or release.**
