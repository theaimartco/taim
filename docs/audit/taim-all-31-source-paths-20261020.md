# TAIM.rs — original fork exact-source completion checkpoint

Date: 2026-10-20
Candidate commit: `65886c11d6fb5eaef0fe15014740f6be3f4d6a37`
Candidate branch: `feature/taim-native-contracts-batch-20261020`
Previous branch / CI-disabled base: `feature/taim-image-culling-ci-disabled-20261020`
Historical original Iced fork: `81afda72f374386036a5b96a55f57a3dcfb0b653`

## Finding

**0 byte-identical same-path original-fork `.rs/.wgsl/.toml` blobs remain**.

Method: Compare every blob at identical paths in complete GitHub
recursive trees of the original fork and this candidate. Both
inventories were `truncated:false`. Previous state was 31 matching
paths. The current code branch changes all 31 formerly matching
paths with implementation, API, shader or policy changes.

**IMPORTANT:** Changed blob identity is NOT the same as independent
authorship. No clean-room audit has been completed. Original Iced-derived
source, algorithms, structure, licenses and history can remain in modified
files, relocated modules and related dependencies. The strict source
lineage gate still requires independent execution and detailed review.
Preserve Iced MIT LICENSE, CREDITS.md and Git ancestry. No claim of
100% independent, fastest-in-world, or production readiness is made.

## Implementation coverage by layer

| Layer | Candidate change | Real integration / caveat |
| --- | --- | --- |
| GPU image batching | Zero opacity, empty clips, NaN/Inf coordinates cull raster/SVG before upload | Actual `wgpu/src/image/batch.rs` hot path; visual and performance runs pending |
| Mac native window | Normalize transparent titlebar/full-size flags, ignore native titlebar flags on undecorated windows | Winit conversion calls macOS normalization; macOS device test pending |
| Window/error contracts | Expose window stacking/attention methods and full source error chains | Winit attention adapter uses TAIM policy; platform checks pending |
| App preset | Checked user-facing preset names, no behavioral change to existing `new` | Name validation API + tests |
| Runtime | CPU/memory telemetry, no-op renderer reconfiguration filtering, multi-image allocation, disabled-image suppression accounting | Async/device tests pending |
| Executor | Optional work submission and selected native executor reporting | No fully independent scheduler; running still uses external pools |
| Geometry/CPU | Typed frame validation and bounded software screenshot memory | Frame checked API and screenshot capture admission integrated |
| Input/window facade | Keyboard classification, checked touch pointer conversion, constrained window setup | Direct public APIs; native integrations retain previous behavior |
| Lazy cache | Overlay-free retained element constructor | Existing ouroboros self-ref design remains derivative; borrow-safety build tests pending |
| WebGPU shaders | Immutable guarded blit/vertex LUT, normalized coverage alpha and bounded gradient mix | **No Naga/WGSL compile or golden-image equivalence test yet** |
| Build policy | Remove two RUSTSEC ignore waivers; add manual cargo aliases; tighten Clippy thresholds | Advisory and lint commands may now correctly fail until issues are fixed |
| Platform abstractions | Distinguish generic window effects; wasm local executor and native selected backend | Target compilation unverified |
| Program tracing | Non-wrapping message sequence IDs | Optional instrumentation API, not automatically wired into every message |

## CI status — explicitly disabled

- `.github/workflows/verification.yml` is **entirely commented out**
  on the development lineage at the owner's request; a source check
  found zero active YAML lines.
- Five legacy workflows (`ci.yml`, `fuzz.yml`, `golden.yml`,
  `licenses.yml`, `perf.yml`) on the historical main branch
  were already fully commented.
- The development branch retains only `verification.yml` under the
  workflow directory; no runnable `on:` or `jobs:` records remain.
- This prevents normal hosted CI coverage. The GitHub Actions
  runner allocation blocker #2 has not been resolved by commenting
  definitions and is **not** closed.
- None of the new regressions has been executed. CodeRabbit or GitHub
  status checks do not substitute for real Rust compilation.

## Required manual acceptance — release NO-GO

Run on a real development device with Rust and required native libraries:

```bash
cargo fmt --all -- --check
cargo taim-check
cargo taim-lint
cargo taim-test
cargo test -p taim --features image --locked
cargo test -p taim_wgpu --all-features --locked
cargo audit
python3 scripts/audit/source-lineage-gate.py --strict --output /tmp/taim-lineage.json
```

- Run Naga/WGSL shader compilation and golden-image comparisons against
  the parent branch, including alpha, gradients, blits and vertex layouts.
- Run native Wayland/VYOM frame presentation, input/IME, accessibility,
  GPU device loss/recovery and software screenshot memory regression.
- Check macOS Windows Linux and wasm feature permutations, crate doc
  warnings, semver and user app compilation.
- Audit upstream-derived source at token and algorithm levels; retain
  required attribution and do not mistake zero exact hashes for 100%
  clean-room authorship.
- Repeat performance comparisons on the SAME physical hardware and
  capture p50/p95/p99 along with CPU/GPU, memory and power data.

**Final decision: NOT PRODUCTION READY. Do not merge into main or
publish a stable/v1.0 package without independent passing evidence.**
