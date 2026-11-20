# macOS packaging and sandbox acceptance — 2026-11-15

Scope: local macOS application distribution tooling, not framework-wide
production clearance or Apple App Store approval. The user explicitly requested
macOS only, no Android emulator, and no upload.

Host: Apple Silicon arm64, macOS 26.6.2 (25G83), Xcode macOS SDK 26.2,
rustc 1.95.0 (59807616e 2026-04-14). Source is a dirty working tree based on
`e0aaf631e8e6b4a021cd6fc58c259ed81ce0c910`; this is not an immutable release.
The accompanying [JSON record](macos-store-acceptance-20261115.json) preserves
source/artifact hashes and the actual runtime report.

## Accepted locally

| Check | Result |
| --- | --- |
| Native arm64 counter development and optimized builds | PASS |
| Real `.app` layout, icon conversion, metadata and deployment-target checks | PASS |
| Ad-hoc signature and actual signed App Sandbox entitlement | PASS |
| Final optimized fixture writes/reads its private container | PASS |
| Two fresh fixture processes retain settings and increment persisted launch count | PASS |
| Explicit wgpu backend presents frames in the sandbox | PASS |
| Visible fixture, typing and button interaction | User confirmed on the earlier fixture run with the same view implementation |
| Fresh generated notes project builds its own optimized sandboxed `.app` | PASS; standalone Cargo workspace with a newly resolved lockfile |
| Installer container construction via actual `productbuild` | PASS; deliberately unsigned QA package, not uploadable |
| Store validation rejects an ad-hoc app | PASS: expected rejection |
| Store build without owner credentials | PASS: expected early rejection, no store product produced |
| Native host/project-generator Rust tests | 89 + 4 passed |
| Packaging identity, profile, platform, entitlement and stale-artifact regressions | 11 passed |
| Targeted all-targets strict Clippy, rustfmt and whitespace checks | PASS |

Final fixture executable SHA-256:
`4fa399c738d8ddf7ea87f770ef562e858e0d81fbc9caeaf73a5d14127f0c2589`.
Fresh standalone notes executable SHA-256:
`8828d15150b4d808db6f8fb9eb4616bbc0b73c1ccac95090a838971e9b1f83a7`.

Raw local artifacts, retained under ignored `target/`:

- `target/macos/store-probe-release-final/` — optimized `.app` and build report.
- `target/macos/store-probe-smoke-final/` — two launch logs, container reports and runtime report.
- `target/macos/generated-notes-release-01/` — standalone generated consumer `.app` and report.
- `target/macos/store-probe-unsigned-test.pkg` — installer structure QA only.

The fixture's native storage path is inside
`~/Library/Containers/com.theaimartco.taim.storeprobe/Data/Library/Application Support/com.theaimartco.taim.storeprobe`.
The runner requires a fresh per-launch token and frame marker, so existing files
or an older running process cannot satisfy a new run. Screenshots were not
captured because the OS did not permit window capture; presentation markers
alone are not claimed as pixel correctness evidence.

## Changes under acceptance

- `scripts/macos/package.py`: Cargo-owned executable selection, deployment and
  dylib checks, icon conversion, bundle metadata/privacy manifest, sandbox
  signing, profile/certificate validation, and signed installer construction.
- macOS `platform::data_dir`: Foundation's Application Support container and
  stable bundle-ID namespace; Objective-C autorelease pools cover worker calls.
- macOS `platform::open_url`: native NSWorkspace rather than a shell helper.
  Actual URL dispatch was not exercised in this pass.
- `taim-new`: self-contained macOS configuration, packager and assets; Android
  Rust targets are opt-in rather than downloaded automatically by the template.
- Dedicated macOS fixture and a bounded two-process runtime smoke runner.

## Owner/distribution checks still pending

There were zero valid code-signing identities in the queried Keychain. The
owner has an Apple Developer account and requested no upload. No certificates,
profiles, account records or store content were created or changed.

Actual owner-certificate signing, provisioning acceptance, signed installer
creation and Apple server validation remain unexecuted. The signed branch is
implemented and its identity/profile rejection rules are covered by synthetic
tests; those tests do not establish Apple acceptance. An unsigned installer
construction check is not a signed installer acceptance result.

The universal/Intel option is implemented but was not built or run in this pass.
Only this Mac/OS was exercised; the configured macOS 13.0 minimum is checked
against Mach-O metadata, not runtime-certified. VoiceOver, complex IME/language
coverage, real app flows, older OS versions, long sessions, sleep/wake and
distribution/install on a different Mac remain app-specific acceptance work.

No Android emulator or Android runtime tooling was installed. No upload,
publication, signing-key import, tag or commit was performed.
