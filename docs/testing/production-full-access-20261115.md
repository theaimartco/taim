# Production hardening with full local access — 2026-11-15

The automated code release gate passes. This is a release candidate, version
0.15.0-rc.1; all-platform production acceptance remains pending the physical
and independent gates below. No release was uploaded, published or tagged.

The reviewed local candidate is isolated on branch
`codex/production-readiness-20261115`, in
`target/release-candidate-20261115`. The user's main working tree is preserved.
The final [release report](../../target/production-release-final-20261115/report.md)
records the exact clean commit, each command's result and raw-log SHA-256.
The [artifact inventory](../../target/production-readiness-20261115/report.json)
records candidate/source hashes and runtime artifact hashes. These ignored
local artifacts must be retained alongside any release review.

## Changes exercised

- Failed settings writes roll back memory; private exclusive temporary files
  prevent collision/symlink clobbering. Concurrent stores remain whole-file
  snapshots requiring application coordination.
- Atlas allocation rejects zero/oversized extents without integer overflow.
- Completing a navigation pop releases outgoing page state/tasks. Motion
  preference is scoped to each view root, including nested/threaded use.
- Real stress tests exercise 1,000 navigation cycles, 10,000 cache operations
  and 128 editor/window lifetimes, rather than placeholder assertions.
- Browsers use the portable clock; the previous first frame panicked.
- iOS gets native window bounds and an embedded default font; the previous
  Simulator bundle clipped the view and rendered no text. Hardware keyboard
  command shortcuts follow Apple conventions.
- Android editor synchronization carries an acknowledged revision. A queued
  Rust snapshot cannot overwrite newer InputConnection text or composition,
  and snapshots from earlier focus owners cannot restore stale focus. The
  regression reproduces mixed Arabic/Japanese input across frame boundaries.
- Android APK packaging supports macOS NDK tools and verifies JNI exports,
  packaged dependencies, 16 KiB ELF/APK alignment and development signatures.
- iOS Simulator packaging verifies Mach-O platform/architecture and signs a
  native bundle locally. macOS sandbox packaging and owner-signing validation
  remain available; production credentials were not used.
- Release checks require clean tracked/untracked source, lockfiles, strict
  platform lints, workspace/tooling tests, generated/downstream consumers,
  dependency audit, three browser engines and both Android ABI links. A
  dirty/skipped/host-only run cannot pass. Evidence mutation is rejected.
- clipboard-win and wasmtimer updates remove a yanked locked dependency and
  unmaintained pin-utils; no advisory was suppressed to obtain this result.

## Verification and limits

| Coverage | Evidence and scope |
|---|---|
| Complete local code gate | Clean candidate report linked above; no skipped stage. |
| Cross-target compilation/lints | Windows MSVC x64/ARM64; macOS x64/arm64; iOS arm64/device and arm64/Simulator; Linux GNU x64/ARM64; Android arm64/x86_64; WASM with WebGPU and WebGL paths. Portable test targets included. Cross compilation does not prove runtime behavior. |
| Native feature coverage | Workspace default tests, all-feature strict Clippy and all-feature tests; glass keeps transmission checks by default and checks documented opaque fallback with legacy web-colors. |
| Web runtime | Headless Chromium, Firefox and WebKit render and respond to two counter clicks using WebGL. Actual Safari, WebGPU runtime and mobile browser devices remain unaccepted. |
| macOS runtime | Optimized native sandboxed fixture passes two launches, private storage/restart and GPU first-present. [Report](../../target/macos/full-access-current-runtime-20261115/report.json). One arm64 Mac; development/ad-hoc packaging. |
| iOS Simulator runtime | Packaged iPhone counter presents readable text within correct bounds. [Screenshot](../../target/ios/full-access-simulator-20261115/counter-fixed.png). Physical devices/VoiceOver/complex native IME remain unaccepted. |
| Android runtime | API 35 arm64 emulator, SwiftShader Vulkan, 4 KiB guest pages. Touch counts, three background/resume cycles and OS background process death restore state; a new process accepts another increment. [Final report](../../target/android/full-access-runtime-final-20261115/report.json). |
| Android editor protocol | Test-only IME drives real InputConnection Unicode commits/codepoint deletion, batches, Arabic/Japanese composition, repeated asynchronous composition cycles, stale connections, redacted secure context and multiline edits. [Final report](../../target/android/full-access-ime-final-20261115/report.json). Restores selected IME in finally; test IME is never bundled with apps. |
| Source provenance | Two source bundles produced from one clean candidate must match byte-for-byte; SPDX 2.3 all-feature Cargo inventory and fresh downstream builds from the extracted bundle. This is source reproducibility, not independent binary reproducibility or license approval. |

The final inventory distinguishes dirty-root native fixture builds (by source
and binary hashes) from the clean candidate's automated gate. Tests were run
by this agent on one local machine, without an independent reviewer. Historical
failed logs/screenshots remain as diagnosis evidence and are not counted as
passing results. The earlier restricted-network report is historical.

## Repeat the checks

Use Rust 1.95.0 and the locked dependency graph. Install the declared Rust
cross targets, an r28+ Android NDK, cargo-ndk, cargo-deny, matching
wasm-bindgen CLI, Node/Playwright's Chromium/Firefox/WebKit and Python 3.11+
with Pillow. On this Mac, Python 3.14 and Java 17 are explicitly selected.
Set ANDROID_HOME, ANDROID_NDK_HOME, CARGO_TARGET_DIR, NODE_PATH and PATH to
the actual installed tools, then run from the clean candidate:

```sh
TAIM_QA_PYTHON=/opt/homebrew/bin/python3.14 CARGO_BUILD_JOBS=2 \
  bash scripts/release/check.sh /absolute/path/to/new-evidence-directory
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo deny --all-features check advisories licenses
python3 scripts/release/build-source-bundle.py --output /tmp/candidate-A.tar.gz
python3 scripts/release/build-source-bundle.py --output /tmp/candidate-B.tar.gz
cmp /tmp/candidate-A.tar.gz /tmp/candidate-B.tar.gz
python3 scripts/release/sbom.py --all-features --output /tmp/candidate.spdx.json
```

On an explicitly chosen test device, build/install the Android counter with
`scripts/android/counter.py`. Use `runtime-smoke.py` and `ime-probe.py` with
coordinates measured from that device's screenshot. The final IME run uses
20 composition regression cycles, not just a successful commit return value.

## Remaining production acceptance

The existing owner release policy remains in effect:

1. Physical Android devices covering three GPU vendors and API range;
   physical Windows/Linux and claimed VYOM hardware; supported Apple devices.
2. Human screen-reader, keyboard, complex IME, recovery, thermal/idle resource
   and accessibility acceptance on those targets. Android protocol tests do
   not establish TalkBack usability.
3. Same-hardware benchmark comparisons before publishing comparative claims.
4. Independent API, security/privacy, unsafe-code and attribution/license
   review, clean-machine rebuild and written release-owner signoff.
5. Owner distribution identities/profiles and platform validation for any
   store release. Locally generated test keys/ad-hoc signatures are not owner
   distribution signing. No remote publication was performed.

On iOS, system theme detection is unavailable through the current winit
backend and `platform::share_text` is unsupported. These are documented in
[the iOS target notes](../platform/ios.md); they are not marked as passes.

These gaps require target hardware, platform work and independent acceptance.
Full filesystem access alone cannot supply those results. Hosted CI remains
disabled as previously requested.
