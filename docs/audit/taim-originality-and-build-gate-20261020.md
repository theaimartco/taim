# TAIM.rs independent-source and build gate (not an originality certificate)

Date: 2026-10-20
Candidate branch: `feature/taim-release-hardening-20261020`
Baseline historical Iced fork: `6db551046abd02eadee9f6bbcfa103bc4a9fadd5`
Parent candidate: `feature/taim-native-contracts-batch-20261020`
Decision: **NO-GO for production and 100% independent authorship claims**

## What is actually verified

Complete GitHub recursive tree comparisons of the historical Iced fork
(459 .rs/.wgsl/.toml source/config blobs) and the current TAIM
implementation (997 such blobs at the earlier PR #13 checkpoint) were
both untruncated. They had **zero SHA-identical source/config blobs
at the same or relocated path**.

This is **file identity only**. File renaming, adding a method, changing
formatting or wrapping a previously inherited implementation breaks SHA
identity without establishing independent original implementation.

A deliberately bounded 10-file spot comparison additionally counted
matching normalized Rust token occurrences using multiset Sorensen-Dice.
It is NOT a whole-repository originality metric:

| Same-path candidate | Original lines | TAIM lines | Approx token-multiset overlap |
| --- | ---: | ---: | ---: |
| `widget/src/column.rs` | 566 | 677 | **90.6%** |
| `runtime/src/user_interface.rs` | 662 | 1,302 | 60.3% |
| `tiny_skia/src/lib.rs` | 415 | 76 | 23.1% |
| `core/src/widget.rs` | 182 | 18 | 15.8% |
| `widget/src/button.rs` | 727 | 44 | 9.6% |
| `widget/src/text_input.rs` | 1,715 | 72 | 5.6% |
| `widget/src/scrollable.rs` | 2,224 | 55 | 4.3% |

The file-by-file comparison intentionally does not compare moved
implementations with dissimilar paths. The `check-independence.py`
scanner, original history, and a full human review are needed to
investigate origins of implementation, algorithms and layout structures.
High shared token counts alone cannot determine copyright ownership,
and legitimate platform dependencies like winit and futures do not
invalidate source originality by themselves.

## Engineering corrections in current candidate

1. Document public enum variants for native executor selection, generic
   platform capabilities, and preset-name validation to address
   `missing_docs = "deny"` risk. Publicly export
   `PresetNameError` rather than leaving a public method's typed error
   in a private module.
2. Explicitly consume the bounded icon input read result to respect
   `unused_results = "deny"`. The image 0.25 decoder provides
   independent strict dimension bounds and a best-effort allocation
   budget.
3. Reject nonfinite touch-move replacement in the input coalescing
   classification and add a regression test.
4. Normalize invalid popup viewport and desired menu widths before
   creating layout constraints.
5. Introduce `scripts/qa/verify-taim.py` and its five source-only
   test cases. It performs strict source scan, audit and gate unit
   tests, rustfmt, workspace Cargo check, Clippy, tests, and package
   inventory. All commands write logs and a JSON summary.
   Missing tools yield `NO_GO`. Even if all automated steps pass, the
   status is `CODE_CHECKS_PASSED_HUMAN_ORIGIN_AND_DEVICE_REVIEW_REQUIRED`.
   The script never claims 100% independent authorship or production.

All new Rust code, Python runner tests, and associated scripts are
**committed but unexecuted** here; toolchain and private local checkout
are absent. This status must be updated from actual machine evidence,
not from the existence of tests.

## CI and review findings

The owner requested GitHub Actions YAML files be commented out.
The active development lineage preserves
`.github/workflows/verification.yml` fully commented with zero
uncommented YAML lines. Historical main also contains five fully
commented legacy workflow files. **Do not re-enable these without
explicit approval.** GitHub Actions runner-allocation issue #2 is open.

PR #13 is a draft; its CodeRabbit bot comment explicitly says draft
PRs are not automatically reviewed. A successful connection or status
check is not a substitute for line-level code review.

## Required to reach a credible full-TAIM milestone

- Manually run `python3 scripts/qa/verify-taim.py` in a **clean,
  full-history checkout** with the pinned compatible Rust toolchain;
  investigate every failing step and preserve all result artifacts.
- Run a complete token/semantic similarity comparison, with relocation
  detection. Focus high-overlap widget layout modules such as
  `widget/src/column.rs`. Record independently designed algorithms,
  functional differences and tests rather than artificially rewriting
  text to change hashes.
- Prove macOS, Linux Wayland/X11, Windows, and wasm compile matrices.
- Validate Naga/WGSL shaders, alpha blending and color-space behavior
  against trustworthy visual baselines on real GPU hardware.
- Verify physical Wayland/VYOM presentation, IME, accessibility,
  hotplug/device loss/recovery, capture teardown and bounded memory.
- Collect reproducible same-device p50/p95/p99, CPU/RAM/GPU/power and
  input-to-photon timing against Iced, Slint and egui.
- Retain historical MIT LICENSE, CREDITS.md and Git ancestry. Complete
  a manual independent source/attribution review and external release
  sign-off. Never infer it from zero SHA matches.

Current state: **NOT 100% VERIFIED; NO-GO FOR MAIN OR v1.0.**
