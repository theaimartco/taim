# Shared production hardening — 2026-11-15

Scope: shared Rust correctness and release tooling across the advertised
platforms. Production verdict remains **NO-GO** until the remaining release and
device gates are accepted. Work is on an uncommitted tree based on
`cc5f9a47d73b98028caeb8bc3ff70c443b40bbe5`; it is not an immutable release.
The earlier macOS acceptance record describes its own source/artifacts and
does not certify this newer code.
The accompanying [JSON inventory](production-hardening-20261115.json) records
source, log and artifact hashes, with `production_clearance: false`.

## Correctness changes

- `storage::Store`: a failed insertion, update or removal restores its previous
  in-memory snapshot, so retrying the same value really retries persistence.
  Writers reserve separate adjacent temporary files with exclusive creation,
  sync before rename, and remove temporary files on failure. Unix files start
  with owner-only permissions. Existing temporary symlinks cannot redirect a
  write. Stores remain whole-file snapshots: concurrent read-modify-write
  operations need application coordination; this is not a database transaction.
- Atlas allocation rejects empty and oversized extents before padding arithmetic;
  placement checks use subtraction to avoid wrapping at `u32::MAX`. Boundary
  tests also fill and recycle an atlas at the maximum representable extent.
- Navigation requests a final layout when a pop animation completes. The shell
  otherwise paints animation frames without layout, retaining the outgoing
  page's scope/tasks until some unrelated layout. A regression was observed
  failing before this fix and passing afterwards.
- Motion preference is applied in each root's layout, paint and event context,
  with restoration on unwind/nested roots. The former global atomic flag let
  windows and UI threads change each other's preference.
- The three empty stress tests now perform real assertions: 1,000 page push/pop
  cycles release scoped state, 10,000 retained-layer insertions stay within a
  byte budget and stale resources are purged, and 128 headless editor roots
  survive resize/focus loss and release their state. Additional regressions
  cover final-pop cleanup and windows with different motion settings.

## Release gate changes

`scripts/release/check.sh` now checks tracked and untracked source cleanliness
before building, detects source changes during verification, locks workspace
Cargo commands, runs the QA and release tooling suites, checks external
consumers and runs dependency auditing when installed. Missing auditing is
reported as incomplete. `--host-only` and dirty diagnostics never pass the
release gate. The report includes SHA-256 checksums of step logs.

The web build also uses `--locked`. The release evidence validator rejects a
self-review identity disguised with whitespace/case differences. An isolated
no-renderer debug build of `taim-view` is strict-Clippy clean; its unit paragraph
type requires a scoped `unit_arg` allowance in IME composition.

## Verification on this machine

Host: Apple Silicon macOS, Rust 1.95.0; Python 3.14 with Pillow 12.1.1 for the
complete Python QA suite. The default Python 3.15 lacks Pillow.

| Check | Result |
| --- | --- |
| Native default-feature `taim` library check | PASS |
| Native strict Clippy for the library | PASS |
| Isolated view/atlas strict Clippy including their tests | PASS |
| View, native-host and atlas unit tests | 50 + 89 + 9 PASS |
| Gesture, signal and generator unit tests | 96 + 16 + 4 PASS |
| Actual storage source unit tests | 7 PASS |
| Actual app suite / stress suite | 9 + 5 PASS |
| Python QA / release / audit tooling | 29 / 16 / 29 PASS |
| Rust formatting, shell parsing, whitespace | PASS |
| Optimized native counter binary | PASS |
| Current-source optimized arm64 sandboxed fixture `.app` | PASS: ad-hoc signature and local bundle validation; runtime not rerun |
| Full workspace/all-targets release gate | BLOCKED: locked `rustix 1.1.6` absent from local cache; crates.io DNS unavailable |

The storage/app/stress suites ran from a temporary Cargo harness under
`target/production-storage-tests`, referencing the actual repository sources
and native dependencies. The harness retained versions from the workspace
lockfile, while omitting unrelated workspace dev dependencies that cannot be
fetched here. This establishes those contracts, not a complete workspace PASS.
The native unit suites ran through their actual workspace packages.

Raw logs are retained under ignored `target/production-*.log`; the real
diagnostic gate report is `target/production-readiness-20261115/report.md`.
That report correctly returns failure at the uncached dependency; earlier
green Linux results are not substituted for this candidate's full verification.

The optimized counter is `target/release/counter`. The new sandboxed fixture is
`target/macos/production-hardening-20261115-icns/Taim Store Probe.app`; its bundle
report records executable SHA-256
`e4bb3f42444e84c91584a2146e7067f2b30a71c0cecc21f61e6161c65363921b`.
The PNG/iconset build failed in `iconutil` despite correct generated PNG sizes.
The successful build used the previously validated ICNS file through the
packager's existing ICNS input path; the original PNG conversion failure is
retained in `target/production-macos-package.log`. This does not establish
owner distribution signing, Apple approval or current-source runtime acceptance.

With complete dependencies, reproduce through the normal workspace entrypoints:

```sh
cargo test --locked -p taim --lib storage::tests
cargo test --locked --test apps --test stress
cargo test --locked -p taim-view -p taim_winit -p taim_atlas --lib
cargo clippy --locked --workspace --all-targets -- -D warnings
TAIM_QA_PYTHON=/path/to/python-with-pillow \
  bash scripts/release/check.sh target/new-release-evidence
```

## Remaining acceptance

| Platform / gate | Required before claiming production support |
| --- | --- |
| All targets | Complete no-skip code gate on a reviewed clean commit; fresh external package consumer; security/license review; independent rebuild and signed provenance |
| Android | Native linking with NDK; device/vendor/OS coverage and manual TalkBack/IME/lifecycle/recovery checks in the release policy |
| Linux / VYOM | Actual compositor, input, IME, screen reader, device-loss, idle/memory/power acceptance; VYOM needs its own native path evidence |
| Windows | Native builds and runtime/device acceptance for each advertised architecture |
| macOS | Current-source packaged runtime acceptance, owner distribution signing/validation, Intel/universal and supported OS testing, VoiceOver, complex IME and sleep/wake |
| iOS | App bundle/toolchain integration and native device acceptance, including VoiceOver, text input and lifecycle |
| Web | Current-source browser smoke and WebGPU/WebGL coverage on the advertised browsers/devices |

No hosted CI was enabled. No owner key, store submission, published package,
stable tag or production clearance was created.
