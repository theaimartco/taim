# TAIM production release policy and signoff

**Verdict on 2026-10-19: NO GO.**
Branch hardening work is a proposal, not a 1.0 release. No tags or package
publications should be created solely because CI passes.

## Required gates

1. Recorded local verification on the proposed immutable commit:
   pinned toolchain, format, lint, workspace tests, visual goldens,
   AT-SPI bus and harness contract tests. Run the manual verifier and retain
   hashed logs. GitHub Actions is disabled at owner request and is not a
   release prerequisite; the same executable checks remain required.
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
| Android | Experimental ARM64/x86_64 builds; API 35/4 KiB emulator smoke passes; tested 16 KiB GLES smoke passes; Vulkan corruption remains guarded; physical QA pending ([evidence](../android/native-host-20261029.md)) |
| iOS | Unsupported until native device verification |
| Web/WASM | Compile support only; functional device/browser QA pending |

## Latest gate ledger

| Gate | Status |
|---|---|
| Local verification on immutable release candidate | COMPLETE EVIDENCE REQUIRED |
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


## Programmatic pre-main release evidence contract

Execute the evidence integrity validator only from a clean candidate checkout:

    python3 scripts/release/verify-readiness.py \
      --manifest /path/to/acceptance/manifest.json \
      --commit "$(git rev-parse HEAD)" \
      --repo-root . \
      --output /path/to/acceptance/validated-contract.json

Example JSON manifest structure (all placeholders must be replaced):

    {
      "schema_version": 1,
      "candidate_commit": "<full 40-character candidate SHA>",
      "operator": "<test operator identity>",
      "supported_targets": ["VYOM-native-Wayland"],
      "gates": {
        "<each of the 12 names in REQUIRED_GATES>": {
          "status": "PASS",
          "reviewer": "<independent reviewer, different from operator>",
          "reviewed_at": "2026-10-19T00:00:00Z",
          "evidence": [
            {"path": "physical-test-log.json", "sha256": "<sha256 hex>"}
          ]
        }
      }
    }

ALL 12 required gate names must occur, with PASS, a different reviewer,
a timezone-aware review date and at least one SHA-256-matching artifact.
Exact candidate SHA and clean tree are enforced, and missing files are
rejected. The validator runs synthetic contract tests in CI; those
synthetic files are NOT actual hardware measurements.

This checks declared evidence integrity only. It cannot independently
prove physical measurements, real-world reviewer identity, security or
compositor quality. An independent release reviewer must verify the
underlying device, trace and acceptance evidence before allowing a merge.

Live GitHub Actions runner allocation issue:
https://github.com/theaimartco/taim/issues/2.
Job failures with no executed steps are never Rust test results.

Do NOT automatically merge into main, sign a stable tag, publish an
unverified package, or claim production certification solely because
this manifest validator passes.
