# Core production hardening — 2026-10-26

This is working-tree verification. It is not an immutable release artifact,
OS hardware acceptance, stable API promise or proof of independent authorship.
No GitHub Actions workflows were enabled or used.

## Functional changes

- Migrate the accessibility transport to zbus 6.1 with current object-path,
  variant and deferred-builder APIs. Remove mundy and the old standalone
  zvariant/zbus_names dependency stack.
- Implement a Taim-owned Linux color preference watcher using the desktop
  portal. Subscribe before the initial read, ignore unrelated/invalid values,
  keep bus work off the UI thread, and abort on shell drop. Failure leaves
  the window-system theme available rather than preventing window creation.
- Replace yanked Windows clipboard 5.4.1 with 5.4.3.
- Normalize public malformed date fields, saturate month shifts at supported
  year boundaries and use wide arithmetic for the full integer offset range.
- Normalize slider/progress endpoints and use finite wide-arithmetic fractions
  even over the complete f32 range. Reject invalid pointer coordinates/steps.
- Treat focus loss and touch cancellation as lifecycle notifications delivered
  to every view. Clear pending button/drag state without activating selection,
  refresh, drawer dismissal or carousel navigation from cancelled input.
- Record private-bus accessibility/theme regressions in the local verifier.
  Release readiness now accepts recorded local verification rather than
  requiring GitHub Actions, matching the owner's instruction.

## Verification

| Command / check | Result |
|---|---|
| Workspace all-target/all-feature check | PASS |
| Workspace all-target/all-feature strict Clippy | PASS |
| Complete default-feature workspace suite | 2,431 passed; 0 failed; 6 ignored |
| Final complete all-feature workspace suite | 2,342 passed; 0 failed; 6 ignored |
| Native view unit suite | 38 passed |
| CPU-rendered native UI and IME integration | 7 passed |
| Private-bus accessibility transport | 4 passed |
| Explicitly executed private-bus theme race regression | 1 passed |
| External minimal and reactive-counter consumers | Both compiled |
| Windows MSVC and Apple Silicon macOS facade type checks | PASS |
| Python audit / QA / release contracts | 29 + 7 + 11 passed |
| Formatting and whitespace checks | PASS |
| Dependency licenses | PASS |
| Dependency advisory policy | PASS with the documented, guarded paste maintenance exception |

The full native GPU suite runs outside the restricted sandbox. An earlier
restricted replay terminated with SIGSEGV; no assertion is made that all driver
or sandbox crashes are resolved. Cross-target checks on Linux do not certify
native Windows/macOS execution, OS IME, assistive technology, signing or linking.
The theme race test is ignored in ordinary test runs because it requires a
private D-Bus service name; the verifier explicitly runs it under dbus-run-session.
The six ignored cases in ordinary workspace output are not counted as passes.

## Open evidence

`paste` remains through pulp/exr and rav1e/ravif/image. The audit marks it
unmaintained (RUSTSEC-2024-0436), not a newly established exploitable defect.
No codec was disabled. A later bounded engineering risk disposition now permits
this maintenance-only macro temporarily, with exact source/checksum/consumer
and expiry guards; this is not a dependency replacement. See
[the exception and its limits](dependency-maintenance-20261026.md). A policy
requiring zero unmaintained dependencies without exceptions still fails.

The exact Git-blob audit finds zero inherited blobs, and the Cargo boundary
check finds no Iced dependency. The strict source-lineage heuristic still
reports eight token-identical files, 52 high-similarity files, seven disguised-copy
flags and 6,353 inherited-token LOC. These metrics require manual review;
they do not establish an originality percentage or legal conclusion. Historical
origin and license attribution remain intact.

Physical device acceptance, independent provenance/API review, source and
artifact checksums on a reviewed clean SHA, and packaging/signing remain open.

Raw final logs and a hashed evidence index are retained locally in
`target/taim-production-evidence-20261026/`.

## Source artifact and current code checks

The source bundle builder preserves the locked workspace, pinned toolchain,
license/credits and binary assets, adds a per-file SHA-256 inventory, normalizes
archive timestamps/ownership, excludes local state and secrets, rejects source
symlinks, and refuses a source change during packaging. It defaults to requiring
a clean checkout. `--allow-dirty` produces an explicitly labeled, unapproved
working-tree candidate. Five artifact tests cover determinism, exact inventory,
attribution, source boundaries and preserving an existing artifact on failure.

An extracted candidate passed the all-feature facade check and both external
minimal/reactive consumers with network access disabled. This verifies source
completeness using cached third-party packages; it is not a dependency-vendored
offline installer or an independent-machine rebuild. The public facade API docs
also built successfully. The local verifier now includes strict rustdoc and
source bundle creation as well as its prior code checks.

Reproduce the local candidate with Python 3.11+ and the pinned Rust toolchain:

```sh
python3 scripts/release/build-source-bundle.py --allow-dirty
python3 scripts/release/provenance.py --allow-dirty \
  --output target/source-dist/provenance.json \
  target/source-dist/taim-0.15.0-dev.tar.gz
```

The tarball, checksum and provenance are under `target/source-dist/`.
Packaging and the maintenance-risk disposition do not mark physical OS
acceptance, independent origin review or publication/signing as complete.

## Optimized native window smoke

`cargo build --release -p counter --features taim/measurement --locked --offline`
passed. The optimized reference application then ran on a private Xvfb display
with the tiny-skia backend and wrote a fresh successful-present marker. A
second five-second run recorded four frames in total, including startup/expose
work; it did not show a sustained render loop. Both runs ended with the
harness's expected timeout (124), rather than an application error.

This checks an actual native window and optimized Linux link, but uses a
virtual X11 display. It is not Wayland, physical latency/power evidence,
interactive device acceptance or Windows/macOS runtime certification.
