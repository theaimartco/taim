# TAIM declarative linear lifecycle — review candidate

Date: 2026-10-21. Current status: **CODE IMPLEMENTED, UNVERIFIED**.

This change creates a TAIM-owned shared `widget/src/linear.rs` sizing-intent module used by the live Row and Column widget diff paths. Previously, each diff mutated `self.width` / `self.height` while using those same derived fields as the starting point for subsequent child-size aggregation. Once a removed child forced `Fill` or `Fluid`, that inferred strategy could outlive the child if the same widget object was diffed again. It also duplicated container fingerprint construction.

Now each Row/Column stores immutable builder sizing intent (updated only by builder setters), computes a fresh effective size from current children for every diff, and shares a consistent native fingerprint covering children count, resolved size, spacing, padding and alignment. Explicit fixed width/height remains authoritative. Dynamic growth when a child uses Fill/Fluid is preserved where a declared axis is Fit.

Six source-level regression tests: stale Row/Column Fill removals, explicit fixed size preservation, vertical fluid minima, history independence and fingerprint input changes. They are **not passing-test evidence** until Cargo runs on a clean checkout.

Additional release blockers remain: full-source provenance/attribution review, Rust build/Clippy/tests and feature matrix, visual goldens, real Wayland/VYOM/IME/accessibility/GPU recovery and hardware benchmarks. Historical Iced/MIT and Druid/Apache lineage and notices remain preserved. No 100% completion claim, no stable release or main merge.
