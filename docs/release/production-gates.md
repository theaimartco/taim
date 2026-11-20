# TAIM production release policy and signoff

**Current verdict (2026-11-15): automated code gate PASS; broad production
release remains NO GO.** A clean all-platform release check now runs every
step, including Android linking and all three browser engines. Native macOS,
iOS Simulator and Android emulator evidence is available. Physical target,
screen-reader, independent review and distribution-signing gates remain open.
See [the full-access follow-up](../testing/production-full-access-20261115.md).

**macOS follow-up, 2026-11-15:** a real arm64 Mac now passes the local sandboxed
release fixture and generated-consumer packaging checks. This is limited local
evidence; owner signing and Apple distribution validation remain pending. See
[the scoped acceptance record](../testing/macos-store-acceptance-20261115.md).

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

Version 0.15.0-rc.1 is a release candidate, with no stable API guarantee.
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
| Windows x64/ARM64 | Lint clean in the release check; not run, physical QA not accepted |
| macOS | Native arm64 sandbox/storage/restart/input smoke accepted on one Mac; Intel/universal, broad physical QA and owner distribution signing pending |
| Android arm64/x86_64 | Both optimized libraries link; arm64 development APK passes emulator lifecycle/process-death and multilingual/secure/multiline IME probes; physical GPU/API-range acceptance pending |
| iOS arm64 and simulator | Strict cross checks pass; service layer (share, haptics, biometrics, keychain, notifications, location, photo picker, camera), safe area and appearance verified on an iOS 26.2 Simulator, interactive prompts and notification taps included (keychain biometric prompt and camera capture excepted); device/App Store packaging implemented but not run with owner credentials; physical devices, VoiceOver, complex input and owner distribution pending; see platform/ios.md |
| Web/WASM | Counter rendering/repeated clicks pass in headless Chromium, Firefox and WebKit (WebGL2); glass is drawn flat on WebGL; WebGPU, actual Safari and physical devices are not accepted |

## Latest gate ledger

| Gate | Status |
|---|---|
| Local release check (replaces hosted CI) | PASS on clean candidate; no skipped step, including Android link |
| Android device check on physical devices | NOT RUN |
| Real Wayland frame presentation | NOT RUN |
| Real VYOM-native display path | NOT RUN |
| Input, multilingual IME, screen-reader acceptance | Android emulator protocol checks pass; physical IME and human screen-reader acceptance pending |
| GPU recovery and thermal/idle memory/energy | NOT RUN |
| Iced/Slint/egui benchmark proof | NOT RUN |
| Clean third-party package consumer and API freeze | Fresh external consumer builds from extracted immutable source bundle; independent API review/freeze pending |
| Security assessment and signed reproducible release | Dependency audits pass; source bundles reproduce byte-for-byte; SBOM generated; independent security/rebuild and owner distribution signing pending |

For each PASS supply device, exact source SHA, test command, raw trace hash,
independent reviewer and date. Do not substitute self-reported completeness
percentages for evidence. Supported device promises must never exceed tested
coverage.

## Running the local code gate

Use the pinned Rust toolchain and Python 3.11+ with Pillow for the Android
pixel-evidence tests. `TAIM_QA_PYTHON` selects the interpreter/virtual environment
for the Python checks (default: `python3`). The complete run also requires
the cross-target Rust standard libraries, Android NDK/cargo-ndk, cargo-deny,
wasm-bindgen at the locked version, Node and Playwright with Chromium, Firefox
and WebKit installed.

Run from a clean candidate, writing evidence under ignored `target/` or outside
the checkout:

```sh
bash scripts/release/check.sh target/release-check-candidate
```

On a Mac with the tools in their usual places, the full run is:

```sh
python3 -m venv ~/.taim-qa && ~/.taim-qa/bin/pip install Pillow
cargo install --locked cargo-deny cargo-ndk
cargo install --locked wasm-bindgen-cli --version 0.2.108  # as in Cargo.lock
npm install -g playwright@1.56.1 && npx playwright install chromium firefox webkit
ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/<version> \
NODE_PATH="$(npm root -g)" TAIM_QA_PYTHON=~/.taim-qa/bin/python \
CARGO_TARGET_DIR="$PWD/target/release-$(git rev-parse --short HEAD)" \
  bash scripts/release/check.sh target/release-check-$(git rev-parse --short HEAD)
```

Use a fresh `CARGO_TARGET_DIR` for evidence: cross-target caches in a
long-lived `target/` have produced errors from stale `taim_core` metadata.
With Node 26, Playwright 1.56's browser install can hang while unpacking;
download each archive from
`https://playwright.download.prss.microsoft.com/dbazure/download/playwright/`
(the paths are in `playwright-core/lib/server/registry/index.js`), unpack it
with `ditto -x -k` into `~/Library/Caches/ms-playwright/<name>-<revision>`
and create an empty `INSTALLATION_COMPLETE` file there.

For development diagnostics on the host:

```sh
TAIM_QA_PYTHON=/path/to/venv/bin/python \
  bash scripts/release/check.sh --allow-dirty --host-only target/host-check-candidate
```

Exit 0 means the complete automated code gate passed; it does not grant device
acceptance or release approval. Exit 1 is a failed check, 2 a preflight/usage
error, and 3 incomplete coverage. Dirty or host-only runs cannot return a
release PASS. Untracked source is included in cleanliness checks; changing the
source while checks run invalidates the result. Reports include log checksums.
Use a new evidence directory for each run.

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
