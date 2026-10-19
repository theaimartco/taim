# TAIM engineering status

**Version:** 0.15.0-dev (experimental).
**Production release verdict:** NO-GO until every evidence gate in
docs/release/production-gates.md is approved on a reviewed release commit.

## Implemented foundations

The repository includes a Rust GUI workspace with wgpu/software renderer
paths, a native AppRunner/task scheduler, Wayland/winit integration,
experimental native VYOM support, widgets, a design-system layer, visual
golden tests, pointer/IME/accessibility modules, profiling and benchmarks.

An implemented module, a passing unit test, a virtual screenshot and
physical hardware acceptance are four different milestones.

## Source origin checkpoint (candidate 9ba087a4)

On the identity continuation branch, a full fork-tree Git SHA comparison
finds **58 byte-identical same-path `.rs/.wgsl/.toml` files** against
the original Iced fork, down from 60 in the prior candidate. This is a
lower bound on inherited source, not a verified original-source percentage.
The new bounded-stream and overlay-cursor implementations have committed
regression tests, but their Rust execution has not been certified.
See docs/audit/taim-stream-overlay-native-20261020.md.

## Native geometry and navigation checkpoint (candidate eddaf3aa)

A complete Git-tree comparison finds **55 byte-identical same-path
`.rs/.wgsl/.toml` files** at the Iced fork baseline, down from 58.
Original screen-placement semantics, exact cardinal layout handling
and centerline pane navigation are now committed with unexecuted tests.
The inherited-code replacement and manual provenance review are unfinished.
See docs/audit/taim-native-layout-20261020.md.

## TAIM-native input and clock checkpoint (2026-10-20)

A fresh, untruncated original-fork Git-tree comparison finds **46**
byte-identical same-path `.rs`, `.wgsl`, and `.toml` files, compared
with 55 before the input/clock/platform pass. This number **is not a
source originality percentage**; much inherited code remains edited, moved,
or token-similar. The 9 changed inherited files now include newly authored
input classification, overflow-safe scheduling units, shadow validation,
window presentation semantics, workspace IDs and Linux application-ID
sanitization. Neither compilation nor production acceptance is verified.
See `docs/audit/taim-remaining-source-20261020.md`.

## Verification lanes

- Hosted CI: .github/workflows/verification.yml on PRs and main.
- Physical presentation, input, screen-reader, device-loss and idle-energy:
  docs/testing/wayland-vyom-device-acceptance.md.
- Same-device Iced / Slint / egui comparison:
  docs/performance/four-framework-comparison.md.
- Packaging, API compatibility, security and signed release:
  docs/release/production-gates.md.

The five historical commented workflow files are not active CI. Hardware
results, stable API compatibility, package publication and tag signatures
are not yet certified. Do not advertise microsecond internal work as
input-to-photon or physical display latency.
