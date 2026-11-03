# TAIM production release policy and signoff

**Verdict on 2026-11-02: NO GO.** The automated checks pass locally; no
device, screen-reader or release-signing gate has been run.

**Decision (owner, 2026-11-02): no hosted CI.** Gate 1 is met by the local
release check, `bash scripts/release/check.sh EVIDENCE_DIR`, run on the
candidate commit from a clean checkout, with its report kept as evidence.
No tag or publication follows from that check alone.

## Required gates

1. A passing local release check (`scripts/release/check.sh`) on the
   candidate commit, with no skipped step: pinned toolchain, format,
   desktop and Android lints, workspace tests (visual goldens included),
   the new-project smoke and an Android link with the NDK.
2. **Android:** a passing `scripts/qa/android-device-check.sh` report,
   with its by-hand section completed, on at least three physical
   devices with different GPU vendors (for example Adreno, Mali,
   PowerVR or Xclipse) and Android versions spanning the supported
   range (API 26 and the newest), plus one emulator. A passing
   `scripts/qa/firebase-test-lab.sh` run on three physical models from
   different manufacturers may stand in for the automated half of the
   device check; the by-hand half still needs a person. Supported devices
   in published material may not exceed the tested ones.
   **Desktop and VYOM:** evidence under docs/testing/wayland-vyom-device-acceptance.md
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
| Local release check (replaces hosted CI) | PASSES WITHOUT THE ANDROID LINK STEP (no NDK where it was run) |
| Android device check on physical devices | NOT RUN |
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
