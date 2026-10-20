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

## Rendering and executor safety checkpoint (candidate 706a0575)

The native-render/executor branch now has **39 byte-identical same-path
`.rs/.wgsl/.toml` paths** against the recorded original Iced fork,
down from 46 in the previous checkpoint. This is a narrow SHA-count,
**not an original-source completion percentage**.

TAIM now conservatively skips fully zero-alpha path/rectangle fills at
the geometry Frame dispatch boundary, normalizes invalid default font
text size, rejects a missing async executor during runtime initialization,
and clamps zero-duration timers on Tokio, smol and Wasm backends. The
eight added regression tests and whole-workspace builds are **unexecuted**.
Provenance review and release acceptance remain NO-GO. See
`docs/audit/taim-native-render-executor-20261020.md`.

## Widget event and drag arbitration checkpoint (candidate f849254b)

The latest untruncated historical Iced-fork Git-tree comparison reports
**37 byte-identical same-path .rs/.wgsl/.toml files**, down from 39.
The TAIM widget `Action::publish` now schedules the next frame,
matching its documented contract and live canvas/shader dispatch.
Pane drag handlers now receive only finite, in-bounds cursor coordinates
from the actual pane-grid interaction path. Six unit tests were added
but **not executed**. Historical code provenance, build verification and
production release are still NO-GO. See
`docs/audit/taim-widget-action-drag-20261020.md`.

## Executor compatibility and lifecycle correctness (candidate b76e5ffc)

The current full fork-tree comparison finds **35 byte-identical same-path
`.rs/.wgsl/.toml` files**, down from 37. Original Iced-derived code also
remains in modified/relocated files. This metric is not an independence
percentage. The previous null-backend fail-fast change broke scheduler
tests constructing an explicit `null::Executor`; the explicit constructor
is now restored and startup rejection moved to the **default** backend
when no native async executor is configured. A TAIM-owned native thread-pool
adapter now accounts for tasks submitted, active and terminated through
atomics and handles cancellations before the first poll. Seven added/revised
unit tests are unexecuted, and full Rust build remains blocked. See
`docs/audit/taim-executor-contracts-20261020.md`. NO-GO for main/production.

## Input-delivery, gesture cancellation and Windows shell checkpoint

The current untruncated fork-tree Git SHA comparison finds **33**
byte-identical same-path `.rs/.wgsl/.toml` files, down from 35 in the
previous executor checkpoint. This is NOT a total-source originality
percentage. The TAIM core queue contract now protects wheel scroll
deltas from replacement, filters malformed active pointer positions,
and converts invalid terminal pointer events to safe cancellation.
The actual pointer Router also enforces this policy for direct callers,
including canceling a prepared arena after invalid initial Down.
Windows applies custom undecorated shadows only to undecorated windows
via its real Winit attribute conversion.

Eight new regression cases are committed but **not executed**. Rust,
Cargo, Clippy, real Wayland/VYOM, accessibility, GPU recovery and manual
source lineage verification remain **NO-GO**; see
`docs/audit/taim-input-delivery-window-20261020.md`.

## Bounded window-icon decoder and menu overlay viewport placement

The complete Git-tree exact same-path source comparison now finds **31**
unchanged `.rs/.wgsl/.toml` files relative to historical Iced, down
from 33. This narrow value does not measure original authorship.

TAIM native window-icon loading now bounds compressed input to 8 MiB,
validates header dimensions against 2048 pixels per edge before decode,
and sets additional decoder allocation limits. Dropdown overlays now
clamp invalid anchors and shift menus left to avoid right-edge clipping.
Four added unit tests are committed but **not executed**.

Rust/toolchain validation, previous PRs, real GPU/input/IME/accessibility
tests and independent source-lineage review remain open. Do not merge
to main or describe the framework as production ready.
See `docs/audit/taim-native-icon-overlay-20261020.md`.

## All 31 formerly exact inherited paths addressed — code candidate d8f60030

Against the complete historical Iced Git tree `56b706fc`, the current
TAIM candidate has **zero byte-identical files at the same original path**
with extensions `.rs`, `.wgsl`, `.toml`, down from 31 in the
previous checkpoint. This is **NOT** proof of 100% independent TAIM
implementation: substantial old Iced-derived designs/code, including
changed/moved modules, may remain. Manual/token provenance gate
and substantive replacement of derivative source are still required.

Implementation includes GPU image admission before atlas upload,
native macOS titlebar normalization, full error-source propagation,
checked application setup and startup presets, explicit runtime
diagnostics and optional work submission, bounded CPU screenshot/geometry
APIs, window/input facades, shader vertex/color policy, and local Cargo
verification commands. New Rust/WGSL regression and visual acceptance
remain unexecuted. See
`docs/audit/taim-all-31-source-paths-20261020.md`.

CI is intentionally disabled at owner's request; this is a **release
NO-GO**, not a successful verification result.

## Verification lanes

- Hosted CI: DISABLED at owner request — verification.yml is fully commented;
  historical ci.yml, fuzz.yml, golden.yml, licenses.yml and perf.yml
  also have no active YAML lines. Manual checks remain required.
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
