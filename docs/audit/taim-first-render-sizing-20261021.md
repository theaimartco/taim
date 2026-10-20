# TAIM first-render sizing hardening (candidate)

Date: 2026-10-21. Status: **uncompiled, NO-GO**.

A new Row/Column could previously report its declared Fit size until Widget::diff ran, even though current children demanded Fill/Fluid. `width`/`height` fields stored inferred results and created state-lifecycle ambiguity. This patch removes those redundant mutable fields. Row and Column resolve sizes directly from current children plus declared builder intent on every `size()` and `layout()` call; Wrapping Row/Column now resolve from the same source in initial wrapping layout, not from stale mutable fields. Existing diff conservatively clears the cache.

Three source-level tests added for first-render row/column Fill and stale Fluid removal. **These tests were not executed.** No independent source attribution or release claim. Must pass Rust fmt/Clippy, all widget/core integration tests, incremental layout reflow, wrapping goldens, Wayland/VYOM actual rendering. Inherited licensing and code origin require separate review.
