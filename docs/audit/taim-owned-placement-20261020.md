# TAIM owned measured-node placement

Date 2026-10-20. Candidate: PR #19 successor. Status: implementation committed, NOT COMPILED.

The live flex resolver now dispatches measured child nodes to a separate TAIM-owned positional prefix-scan stage. The new axis-neutral Frame computes positions with f64 intermediate arithmetic, saturating to finite f32, and distinguishes intrinsic measurement, resolved cross size, cross-axis alignment and baseline. Unlike the old inlined positioning pass, a very large extent or gutter cannot overflow the next sibling's coordinate to infinity. Nonfinite child extents do not poison the parent's intrinsic span. No widgets are remeasured.

Six new source tests cover horizontal and vertical positioning, alignment, huge finite extents, NaN handling, first baseline and empty groups. All still need executable Rust verification. This changes unbounded/NaN overflow behavior intentionally and must be visually compared against existing Row/Column layouts before production.

The Druid Apache notice in core/src/layout/flex.rs, historical Iced licensing/credits, and full Git ancestry remain intact. This does not certify independently authored lifecycle/rendering/platform code, whole-source provenance, production safety, performance or device compatibility.

Required: cargo fmt, cargo test -p taim_core, full workspace tests and Clippy, Row/Column golden tests, strict full-history source-lineage gate, independent code-origin review, GPU/Wayland/VYOM/IME/a11y device acceptance and same-device comparison. GitHub Actions issue #2 remains a blocker; disabled workflows were not enabled here.
