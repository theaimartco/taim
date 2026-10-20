# TAIM native Flex Engine: scoped main integration

Date: 2026-10-20
Origin: https://github.com/theaimartco/taim/pull/17
Integration: direct, scoped port of the core allocator onto main.

Included:
- core/src/layout/flex.rs
- core/src/layout/flex/allocator.rs
- core/src/layout/flex/engine.rs

Compatibility: The pre-change versions of core/src/layout/flex.rs, core/src/layout/flex/engine.rs and core/src/length.rs on main matched the PR's parent branch by Git blob SHA.

Not included:
- widget/src/column/placement.rs wrapping fix (its wrapping implementation is absent on main)
- PR development-branch audit/status document changes
- any of the unrelated 131 ahead-of-main commits from the PR source branch

No CI runs were available for the PR; Cargo tests, rustfmt, Clippy and device acceptance were not independently executed as part of this integration. Treat this as DEVELOPMENT CODE, NOT PRODUCTION-VERIFIED. Review layout behavior before any stable release. The inherited Apache 2.0 notices are preserved.
