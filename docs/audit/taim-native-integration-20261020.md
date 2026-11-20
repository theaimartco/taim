# TAIM.rs native integration candidate — 2026-10-20

## Scope

**Status: INTEGRATED IN REVIEW BRANCH — NOT PRODUCTION-CERTIFIED.**

This branch combines the native development lineage through Draft PR #17 with current main commit `f2dd3b785e0fe6ec9e28a1792930393a13c07729`. It makes no assertion of full source independence, clean-room authorship, or readiness to publish.

### Preserved from main

- Entire `assets/` tree with the owner's 149 premium SVG icons and root exports, excluding pre-session icons.
- `scripts/assets/build-icon-font.py`, `scripts/assets/validate-premium-icons.py`, `scripts/design/check-icons.sh` matching that asset set.
- Five owner-disabled legacy CI workflow stubs.
- Prior scoped flex-main integration record, with this document superseding its historical warning that wrapping is not yet in the candidate.

### Integrated from the native candidate

- TAIM-native intent-based application/runtime and subscription tracker.
- Native input/window/gesture/scheduler/pointer and overlay correctness changes.
- Graphics/shader/renderer and executor safety work.
- Explicit Row/Column wrapping group planner and bounded weighted flex allocator from PR #17.
- Source audit and manual QA scripts, test additions, release evidence contracts and native architecture docs.
- Additional commented verification.yml: retained, **not activated**, per previous owner instruction.

### Validation blockers — must not be converted to PASS by assertion

1. Rust toolchain and a full private checkout are unavailable in the current editing environment; no code was compiled, run, linted, or device-tested for this integration.
2. Existing GitHub Actions runner-allocation incident issue #2 remains open. Latest main workflows completed in failure without executed Rust job steps. Owner must investigate GitHub Actions permissions/runner/billing.
3. Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, `cargo test --workspace --all-features --locked`, plus golden and feature-specific flex/layout tests.
4. Run `python3 scripts/qa/verify-taim.py` and `python3 scripts/audit/source-lineage-gate.py --strict --output /tmp/taim-source-gate.json` on the exact merge revision in a clean full-history checkout.
5. Perform a human independent provenance audit. Zero Git blob matches do not establish code independence: prior token-overlap samples of widget Row/Column were near 90% and require redesign/review as appropriate.
6. Validate native Wayland/VYOM compositor presentation, text/IME, accessibility, GPU recovery, platform compatibility and same-device Iced/Slint/egui benchmark evidence.
7. Preserve historical licenses, credits and full Git ancestry; do not present TAIM as never having been based on Iced.
8. Repeat verification after any fixes; require human acceptance before merging this candidate into main or tagging production.

### Change-policy

Do not turn on GitHub Actions YAML without an explicit owner request; executable local QA is the current fallback. Do not remove license/credit text to produce an artificial originality score. Existing `main` remains untouched until the native integration passes review.
