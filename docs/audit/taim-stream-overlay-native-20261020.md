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

## Exact Git-tree checkpoint

At commit `9ba087a4f70d595c149764a5fa7e41c180a2d7fd`
(candidate tree `06a2ddd5ddf5889fe45fb99f97ca967370a1d0aa`)
a complete, nontruncated GitHub recursive-tree enumeration was compared
with fork baseline `56b706fcd35aa881baede77a696bbb842773fdc6`.
Among `.rs`, `.wgsl`, and `.toml` files at the **same paths**:

| Item | Result |
| --- | ---: |
| Baseline source/config files | 459 |
| Current source/config files | 997 |
| Byte-identical inherited file paths | **58** |
| Prior identity-branch byte-identical inherited files | 60 |
| Net reduction | **2** |

Changed exact blobs were `futures/src/stream.rs` and
`core/src/mouse/interaction.rs`. The latter still retains Iced-derived
enum variants; an edited blob is NOT automatically original work.
Cross-path matches, copied code fragments, and high token similarity can
exceed the exact same-path count. **Source independence remains NO-GO.**
