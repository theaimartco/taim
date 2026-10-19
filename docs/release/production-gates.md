# TAIM production release policy and signoff

**Verdict on 2026-10-19: NO GO.**
Branch hardening work is a proposal, not a 1.0 release. No tags or package
publications should be created solely because CI passes.

## Required gates

1. Green active GitHub Actions verification on the proposed commit:
   pinned toolchain, format, lint, workspace tests, visual goldens,
   AT-SPI bus and harness contract tests. Previous five workflows
   were commented out and did not constitute active CI.
2. Evidence under docs/testing/wayland-vyom-device-acceptance.md
   collected on the claimed target hardware. Native VYOM support cannot be
   claimed from generic Wayland/winit tests.
3. Same-hardware four-framework comparisons under
   docs/performance/four-framework-comparison.md. Internal scene-cost
   measurements may be published separately with their actual scope.
4. Independent API surface review and a **fresh external consumer**
   build against an immutable packaged release artifact. Verify every
   documented example and all advertised feature combinations.
5. Review dependency licenses and Iced-origin attribution, unsafe code,
   security privacy boundaries, resource budgets and fault recovery.
6. Artifact provenance: clean SHA, locked toolchain, feature flags, SBOM,
   SHA-256 checksums and independent clean-machine rebuild.
7. Obtain written release-owner signoff; only then sign an annotated tag
   and publish release artifacts and a real changelog.

## Public compatibility contract

Version 0.15.0-dev is experimental, with no stability guarantee.
Before 1.0, publish the stable public API and target matrix, freeze
the API for an independent consumer test, and include a migration guide.
After 1.0, use major versions for breaking changes, minor versions for
compatible additions, and patches for compatible fixes. Provide
security disclosure and deprecation policies without inventing an SLA.

## Current target matrix (not deployment guarantees)

| Target | Status |
|---|---|
| Linux x86-64 via winit/Wayland and X11 | Implemented; physical QA not signed |
| Native VYOM/Smithay Wayland | Experimental; physical presentation not accepted |
| Windows/macOS | Platform code present; physical QA not accepted |
| Android/iOS | Unsupported until native device verification |
| Web/WASM | Compile support only; functional device/browser QA pending |

## Latest gate ledger

| Gate | Status |
|---|---|
| Hosted CI for proposed hardening branch | UNVERIFIED UNTIL RUNS COMPLETE |
| Real Wayland frame presentation | NOT RUN |
| Real VYOM-native display path | NOT RUN |
| Input, multilingual IME, screen-reader acceptance | NOT RUN |
| GPU recovery and thermal/idle memory/energy | NOT RUN |
| Iced/Slint/egui benchmark proof | NOT RUN |
| Clean third-party package consumer and API freeze | NOT RUN |
| Security assessment and signed reproducible release | NOT RUN |

For each PASS supply device, exact source SHA, test command, raw trace hash,
independent reviewer and date. Do not substitute self-reported completeness
percentages for evidence. Supported device promises must never exceed tested
coverage.

## Hardware smoke entrypoint

Run `bash scripts/qa/wayland-smoke.sh <new-evidence-dir>` on actual hardware.
It is an automatic compositor-protocol/process sanity check, not physical
input-to-photon or screen-reader acceptance. Never certify hardware from
that single script. Unverified VYOM-native paths remain BLOCKED.
