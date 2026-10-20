# TAIM native flex measurement scheduling (review-only)

20 October 2026: **Implemented on the candidate branch; not compiled or production certified.**

This patch introduces `core/src/layout/flex/plan.rs`, an independent upfront measurement schedule shared by Row and Column through the live non-wrapping flex resolver. Unlike the historical inherited multi-pass classifier, the schedule reserves fixed cross-dependent main-axis extents *before* measuring any sibling, so free room is not order-sensitive. It uses a wide f64 budgeting calculation for spacing and fixed reservations, with bounded f32 output and explicit nonfinite handling. The live engine now dispatches by the schedule's Intrinsic → CrossProbe → Weighted → CrossDeferred phases. The existing TAIM water-fill allocator handles weighted sizing.

Five unit tests are added for scheduling/spacing/NaN, weighted category semantics, reservation and sibling order. **None is recorded as executed**. Main-axis spacing and explicit overflow constraints still need golden and physical validation. This is a genuine semantic change and could alter sizes under cross-axis compression; compare visuals on nested layouts before merging.

Not addressed: inherited widget lifecycles/cache/rendering/platform origin, whole-tree similarity, WGPU/Wayland/IME/accessibility behavior, independent human source audit, or production gates. Historical druid/Apache, Iced/MIT attribution and Git ancestry must remain. No percentage of original source is inferred.

Required checks: `cargo fmt --all -- --check`, `cargo test -p taim_core --locked`, `cargo test -p taim_widget --all-features --locked`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, full workspace suite, strict source audit and physical compositor golden comparisons. No stable tag, no main promotion until verified.
