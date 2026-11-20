# TAIM engineering status

**Version:** 0.15.0-rc.1 (release candidate).
**Production release verdict:** NO-GO until every evidence gate in
docs/release/production-gates.md is approved on a reviewed release commit.

## iOS services checkpoint (2026-11-20)

iOS now has a service layer comparable to the Android one's core: share
sheet, haptics, Face ID/Touch ID, keychain, local notifications (with launch
tap events), location, photo picker and camera, plus safe-area layout and
live light/dark/Reduce Motion following. `scripts/ios/package.py` builds
signed device `.app`/`.ipa` products for development, ad-hoc and App Store
profiles. On an iPhone 17 Simulator (iOS 26.2), with taps driven
through the Simulator window, the `apple_services_probe` fixture passed every
service including the Face ID prompt (match, mismatch, cancel), notification
and location permission prompts, foreground, background and cold-launch
notification taps, and photo-picker selection and cancel; a release
`aarch64-apple-ios` binary links. The keychain biometric read prompt, camera
capture (no Simulator frames), physical devices and device-signed
installation remain unverified. See [iOS](platform/ios.md).

## Full-access verification follow-up (2026-11-15)

The complete local automated release gate now passes from an isolated clean
candidate, including Android linking and Chromium/Firefox/WebKit interaction.
Strict cross-target checks cover Windows x64/ARM64, macOS Intel/arm64,
iOS device/Simulator, Linux x64/ARM64, Android arm64/x86_64 and Web/WASM.
Fresh default/all-feature dependency audits pass after clipboard-win/wasmtimer
updates remove a yanked version and the unmaintained pin-utils dependency.

Runtime tests exposed and fixed the browser clock panic, iOS clipped layout
and missing fonts, and an Android race that let stale app snapshots discard
newer IME composition. Native Android now acknowledges editor revisions only
after the view consumes the edit. The emulator verifies Unicode/codepoint
editing, composition, secure surrounding-text privacy, stale connections,
multiline input, background/resume and OS process-death restoration. These
are emulator results, not physical-device or human screen-reader acceptance.

macOS optimized sandbox/storage/restart/GPU-present checks and an iOS Simulator
counter also pass. Reproducible source bundles, an SPDX inventory and an
external consumer of the extracted source artifact are available locally.
See [full-access evidence and remaining gates](testing/production-full-access-20261115.md)
and [iOS limitations](platform/ios.md). Owner distribution signing, physical
platform QA and independent review remain open. Earlier dated checkpoints
below are historical; their environment blockers have been superseded.

## Shared production hardening (2026-11-15)

Settings mutations now roll back on failed persistence, use exclusively created
temporary files per writer, clean up failures, and create private Unix files.
The atlas rejects empty/oversized extents without overflow. A completed pop
requests layout to release its outgoing page; reduced-motion settings are
scoped to each root instead of shared globally. Empty stress tests now exercise
navigation/state release, editor lifetimes and cache pressure. Release checks
require a clean checkout, locked workspace commands, QA/release tooling tests,
external consumers and dependency auditing; diagnostic/skipped runs cannot pass.

At that earlier checkpoint, an uncached dependency and unavailable network
blocked the full workspace gate. The full-access follow-up above supersedes
those environment blockers; physical acceptance remains open. See [the scoped hardening report](testing/production-hardening-20261115.md)
for results, reproduction commands and outstanding platform work.

## macOS packaging checkpoint (2026-11-15)

Native arm64 app builds and a sandboxed optimized fixture have now run on a real
Mac. Private settings survive process restart, wgpu presents frames, and the
user confirmed visible UI, typing and button input. A fresh generated notes
project also builds its own sandboxed `.app`. `taim-new` includes macOS packaging;
owner certificate/profile validation and signed installer construction are
implemented. Actual owner signing and Apple upload remain unexecuted; no store
upload was requested. See [macOS acceptance](testing/macos-store-acceptance-20261115.md)
and [packaging instructions](platform/macos.md). The older unrun macOS status
below is historical; broad production gates remain open.

## Page transitions and heroes (2026-11-20)

The navigator has five transitions: slide (the iOS push, with parallax,
dimming, a shadow and an edge swipe back that follows the finger), zoom,
fade, rise and none. `hero(tag, view)` flies shared content between pages
while they move. Tested headlessly: swipe commit, swipe cancel, flick, every
style settling, and heroes paired across pushes and pops. A GPU test on Metal
checks the hero's path, that it grows, and that it stays opaque over a
fading page. Limits:
- A flight scales the hero uniformly; it is not laid out again at each size.
- The flight stays inside the clips of any scroll view it sits in.
- The flight draws the top page's hero in both directions.
- Not yet tried by hand on a phone.

## Motion across the views (2026-11-20)

The views move like physical objects. They run on one shared closed-form
spring, so a retarget mid-motion keeps the speed it had. Buttons and chips
squish. Switches, checkboxes, radios and sliders spring. The segmented
indicator stretches and bounces. Scrolling rubber-bands and bounces at its
ends. `animated` glides content when layout moves it and `appear` brings it
in. Unit-tested, and the transform/opacity path is rendered and tested on
Metal. Not yet felt on a physical phone. The rubber band at the top shows
only once a drag is already scrolling, because a drag that starts at the top
goes to `pull_to_refresh`.

## Live liquid glass (2026-11-20)

Glass answers touch and light: a press lights the rim nearest the finger,
magnifies what is under it and swells the pane on a spring; interactive
panes stretch after a drag and wobble back; `glass_group` melts nearby
panes together through a smooth SDF union drawn once per point; the light
leans towards the pointer (desktop) or with the device's tilt (iOS); and
glass keeps its text readable over any backdrop. Rendered and tested on
Metal on macOS, and run in the iOS Simulator. Not yet rendered on WebGL,
Vulkan or DirectX, and tilt is untried on a physical iPhone.

## Liquid glass and visual checkpoint (2026-11-03)

Glass now frosts through a mip chain of the backdrop (smooth, nine samples
per pixel) with vibrance, lenses at its rim scaled to screen density, and
has a lit rim, sheen and shadow. A `glass` view, `Material::LIQUID` and
`LIQUID_CLEAR`, and glass navigation capsules, dialogs and sheets are new;
segmented controls use a raised thumb. Secondary text meets WCAG AA in
every theme; it measured 1.4–2:1 before. GPU tests had been skipping in
the build environment for lack of an adapter; with Mesa's software Vulkan
driver they run, and all pass. See `docs/design/liquid-glass.md`.

## Application-building checkpoint (2026-11-02)

Added for writing applications: `navigator` (a route stack with page-local
state, slide transitions and back), a lazy `list` that builds only items
near the screen (100,000 items tested), `taim::storage` (persistent
settings, atomic writes), `taim::platform` (open URL, share, vibrate, data
directory; Android over JNI, desktop through system tools) and `taim-new`,
a project generator. A generated project was built on the desktop, checked
for `aarch64-linux-android`, and driven under Xvfb: notes added, saved,
restored after a restart, opened on their own page and closed with back.
The Android side of `taim::platform` has not run on a device.

## View-tree performance checkpoint (2026-11-02)

`cargo bench --bench views` measures a list of buttons inside `scroll`
with the CPU renderer (criterion medians, one Linux x86-64 container,
debug assertions off; indicative only):

| Benchmark | Before | After |
|---|---:|---:|
| pointer move, 100 rows | 40.3 µs | 7.3 µs |
| pointer move, 1000 rows | 3.36 ms | 86 µs |
| paint, 100 rows | 27.4 µs | 8.2 µs |
| paint, 1000 rows | 338 µs | 27 µs |

Two causes. Routing an event visited a parent's children in reverse by
walking all of them once per child, which is quadratic for a long list;
`View::children_rev` makes it linear. Painting drew every row of a
scrolled list; views entirely outside a clipping ancestor (with a 32px
margin for shadows and focus rings) are now skipped. Layout and
`is_animating` are unchanged.

## Android checkpoint (2026-11-02)

TAIM builds and passes `clippy -D warnings` for `aarch64-linux-android`
and `x86_64-linux-android` (`android` feature, NativeActivity). Added: an
`android_main` entry (`App::run_android`), GPU surface drop/recreate across
the activity lifecycle, touch delivered as pointer events, an intercept
phase so touch drags scroll (with flings) and cancel the press underneath,
back navigation that closes overlays then finishes the activity, safe-area
insets, on-screen keyboard for text views, dark mode, Android system fonts
and a JNI clipboard. View-level touch behaviour is covered by
`tests/touch`. Since then: TalkBack through AccessKit (tree conversion
unit tested), input-method text through GameActivity's buffer (diffing
unit tested, including Devanagari composition), every finger delivered as
raw touch, and a `pinch` view (integration tested). **None of it has run
on a device or emulator**: there is no Android NDK in the build
environment, so not even linking is verified, and the GameActivity
backend's C++ glue has never been compiled here. See
`docs/platform/android.md` for the gaps compared with Flutter.

## Build and test checkpoint (2026-11-02)

After the move to the view API (the `program` crate, the Elm-style
`application` builder and the widget facade were removed), the workspace
builds again and its automated checks are green on Linux x86-64:

- `cargo check --workspace --all-targets`, `cargo clippy --workspace
  --all-targets -- -D warnings` and `cargo fmt --all --check` pass.
- Integration tests and benches exercise the engine crates
  (`taim_core`, `taim_widget`, `taim_runtime`, `taim_futures`) directly;
  the `taim` facade exposes only the view API.
- Tests of removed code (`AppRunner`/`Program`/`FrameGate`) were deleted
  with it: `tests/runtime/phase3_inversion.rs`, `benches/phase3_production.rs`.
- Widget-gallery goldens were re-adopted for the TAIM palette introduced in
  13121da. Before that, 25 of them had also drifted by sub-perceptual
  border antialiasing (max channel delta 12); the source of that drift was
  not identified.
- `taim_wgpu` built with `svg` but without `image` failed to compile; fixed.
- `tools/taim_lab` and `examples/trivial` are ported to the view API.
- **`examples/ide` is not ported** and is excluded from the workspace.

None of this changes the release verdict: automated checks passing on a
headless machine is not hardware, accessibility, performance or security
acceptance. Hosted CI remains disabled at the owner's request.

## Implemented foundations

The repository includes a Rust GUI workspace with wgpu/software renderer
paths, a native AppRunner/task scheduler, Wayland/winit integration,
experimental native VYOM support, widgets, a design-system layer, visual
golden tests, pointer/IME/accessibility modules, profiling and benchmarks.

An implemented module, a passing unit test, a virtual screenshot and
physical hardware acceptance are four different milestones.

## Source origin checkpoint (candidate 7cf498d3)

On the identity continuation branch, a full fork-tree Git SHA comparison
finds **58 byte-identical same-path `.rs/.wgsl/.toml` files** against
the original Iced fork, down from 60 in the prior candidate. This is a
lower bound on inherited source, not a verified original-source percentage.
The new bounded-stream and overlay-cursor implementations have committed
regression tests, but their Rust execution has not been certified.
See docs/audit/taim-stream-overlay-native-20261020.md.

## Native geometry and navigation checkpoint (candidate fdbe2153)

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

## Rendering and executor safety checkpoint (candidate 2d0061a6)

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

## Widget event and drag arbitration checkpoint (candidate c7b51661)

The latest untruncated historical Iced-fork Git-tree comparison reports
**37 byte-identical same-path .rs/.wgsl/.toml files**, down from 39.
The TAIM widget `Action::publish` now schedules the next frame,
matching its documented contract and live canvas/shader dispatch.
Pane drag handlers now receive only finite, in-bounds cursor coordinates
from the actual pane-grid interaction path. Six unit tests were added
but **not executed**. Historical code provenance, build verification and
production release are still NO-GO. See
`docs/audit/taim-widget-action-drag-20261020.md`.

## Executor compatibility and lifecycle correctness (candidate 8b59fd9c)

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

## All 31 formerly exact inherited paths addressed — code candidate 65886c11

Against the complete historical Iced Git tree `81afda72`, the current
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

## TAIM independent-source and executable verification checkpoint

Current development branch `feature/taim-release-hardening-20261020`
corrects documentation and optional API/strict lint issues, invalid touch
coalescing, and non-finite popup-width layouts; it adds the **manual**
`python3 scripts/qa/verify-taim.py` gate. The owner-requested GitHub
Actions workflow remains fully commented out.

Complete Git-tree hashes against historical Iced show **0 identical
source blobs even when comparing across different paths**; however a
sampled normalized token-multiset comparison identifies approximately
**90.6% overlap for `widget/src/column.rs`** with the historical fork.
Such overlap is a source-lineage **review flag, not proof of clean-room
authorship or infringement**. Full strict heuristic scan is still
unexecuted here because the private checkout/Rust toolchain is not
available in this execution environment.

NO-GO: CodeRabbit reports PR #13 draft is not automatically reviewed.
No passing Rust build, Clippy, tests, shader validation, physical
Wayland/VYOM acceptance, or independent manual code-origin review.
Further substantive framework architecture and widget implementation
rewrites are needed to establish TAIM's distinct implementation.
See `docs/audit/taim-originality-and-build-gate-20261020.md`.

## TAIM-native wrapping Row/Column engines (candidate 08a3f432)

The interactive `Column::wrap` and `Row::wrap` layout paths now
use a TAIM-specific explicit group planner, rather than detecting
wrapping boundaries from already-positioned X/Y coordinates. This
repairs nonzero-padding, per-group cross alignment, oversized-child,
malformed-spacing and enormous numeric extent cases. Both planners
measure each widget exactly once before arranging nodes; the placement
pass is linear in child count. Nine source regression tests were
committed, **not executed** in this environment.

This is **not** full framework independence: normal Row and Column
still use inherited multi-pass `layout::flex::resolve_cached`, and
same-path Rust token-multiset spot audit still finds approximately
90% historical overlap in both widgets. Replacing all derivative
implementations, then compiling, testing and reviewing source origin
remains an open production gate. User-disabled GitHub CI remains
fully commented. See `docs/audit/taim-native-wrapping-20261020.md`.

## TAIM-native weighted flex allocator — 2026-10-20

The actual non-wrapping Row/Column flex engine now delegates weighted
main-axis space allocation to an independently implemented TAIM
water-filling kernel at `core/src/layout/flex/allocator.rs`.
It replaces the inherited recursive min/max/main-fluid allocation
passes (not the whole legacy constraint engine). The new allocator
reserves feasible minimums, shares shortage proportionally, and sorts
maximum-saturation breakpoints for deterministic O(n log n) weighted
distribution. Intrinsic-content-capped fluid widgets are measured
once, with unused space redistributed to remaining flexible children.

The wrapping Row/Column group planner was also corrected to avoid
subtracting an overflow-saturated trailing gutter (which could
incorrectly produce zero intrinsic width/height). 12 new Rust tests
were committed in this branch — **not executed**. The flex allocator
is not benchmarked; O(n log n) is algorithmic analysis, not a measured
frame-rate improvement.

**Not 100% TAIM:** static/cross-axis flex measurement retains
inherited druid-inspired structure under Apache 2.0, and widgets,
platform adapters and other framework subsystems remain subject to
complete semantic source-origin audit. Rust workspace compilation,
cross-target rendering and actual device verification remain NO-GO
until run. CI workflows are still fully commented at owner request.
See `docs/audit/taim-native-flex-20261020.md`.

## TAIM-native winit shell — 2026-10-22

`winit/` is rewritten around a TAIM host/engine split (see
`winit/src/shell.rs`): the host is the only code touching the `winit`
event loop; the engine owns the application, windows and runtime as one
cooperative task; they exchange `Signal`s and `Request`s. Keyboard,
pointer, event and window-attribute conversions, window state, the IME
overlay, the window manager and the proxy were rewritten with it.

Line-level overlap with the recorded Iced fork point (normalized lines of
25+ characters that also occur anywhere in the fork tree) fell from
1,675 / 2,855 (58.7%) to 390 / 2,732 (14.3%). What remains is mostly
shared vocabulary that any `winit` adapter contains: `cfg` attributes,
enum variant names, trait method signatures, `arboard` and `winit` calls.
It is a similarity measure, not an originality percentage.

Verified in this checkpoint: `cargo check --workspace --all-targets
--all-features`, `cargo test --workspace --all-features` (2,279 passed,
0 failed), clippy on `taim_winit` with no findings, and the `trivial` and
`mobile_shell` examples running under Xvfb with the tiny-skia backend
(render, tab/chip/bottom-nav clicks; pixel-identical before and after the
window-layer rewrite). Typing into `mobile_shell`'s search field also
works once the test harness gives the window X keyboard focus (Xvfb runs
without a window manager, so `xdotool windowfocus` is required).
Physical-device, Wayland, Windows and macOS runs remain unverified. Web
runs in headless Chromium through WebGL2 (`scripts/web/build.sh`, see
`docs/platform/web.md`); Windows, macOS and iOS are lint-clean in the
release check but have not been run.

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
