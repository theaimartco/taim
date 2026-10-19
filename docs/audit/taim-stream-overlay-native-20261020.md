# TAIM native-runtime migration: bounded streams and overlay arbitration

Candidate branch: `feature/taim-stream-overlay-native-20261020`.
Parent: `3c35c7711607cfaab7f11be3d5b887797fb39ee3`.

## This patch actually changes runtime behavior

1. `taim_futures::stream::{channel,try_channel}` now use a TAIM-owned,
   pull-driven producer/receiver driver. The stream owns the producer future,
   so dropping it cancels active work. The bounded MPSC queue still enforces
   producer backpressure. The error channel drains successful buffered items
   before yielding exactly one terminal error.
2. The overlay compositor resolves cursor requests by actual painting
   order (last painted non-`None` layer) rather than enum discriminant
   ordering. It avoids evaluating covered layers once the visible cursor
   is determined, retains the existing enum and cursor mappings, and handles
   an intentionally `Hidden` top-layer cursor.
3. Regression tests were added for producer ordering/backpressure,
   single terminal error, successful completion, producer cancellation,
   foreground cursor precedence and hidden/transparent overlay behavior.

## Evidence and limits

- Changes are committed as an **unverified implementation proposal**, not
  a successful build or passing test. Rust/Cargo are unavailable in this
  execution environment and GitHub Actions issue #2 currently prevents
  hosted verification.
- A fresh checkout must run:
  `cargo fmt --all -- --check`
  `cargo check --workspace --all-targets --all-features`
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  `cargo test --workspace --all-features`
  `python3 scripts/audit/source-lineage-gate.py --strict --output /tmp/taim-audit.json`
- This **does not establish 100% TAIM-origin code**: original Iced-derived
  modules (including unchanged exact blobs and edited lineage) remain.
  Updating an inherited enum with additional behavior does not magically
  make its inherited portion independently authored.
- Real VYOM/Wayland, IME/AT-SPI, renderer recovery, and four-engine
  physical performance acceptance remain mandatory.
- Preserve complete original Git history, Iced MIT LICENSE and CREDITS.md.

## Further native replacements

Prioritize widget-tree/input dispatch, editing/selection, renderer/text
preparation and platform integration using separately reviewed behavior
specifications. Each replacement needs source provenance review, API
compatibility, ownership tests and comparative functionality/performance
evidence before integration into main.
