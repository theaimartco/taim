# Feature policy

> Status: Phase 0 baseline. Revisit whenever a later phase adds a real
> `profiling` or `recovery` capability (Phase 15 and Phase 17 respectively).

This document records which Cargo features are part of the VYOM Aura
default profile, which are opt-in, and why. It is the source of truth for
the feature combinations `scripts/ci/check-workspace.sh` and
`.github/workflows/ci.yml` must verify.

## Profiles

### `default`

```text
wgpu, tiny-skia, crisp, hinting, thread-pool,
linux-theme-detection, x11, wayland, fira-sans
```

This is the VYOM production profile: Vulkan-capable `wgpu` renderer with a
`tiny-skia` software fallback kept live per roadmap rule 6 (fallback paths
stay until replacement parity), pixel snapping, metrics hinting, the
thread-pool executor, Linux system theme detection, and both X11 and
Wayland windowing backends enabled during the transition window before the
native VYOM/Smithay backend (Phase 14) lands.

**`web-colors` was removed from this list.** It disables linear-light
compositing, so while it was a default the shipping renderer blended
gamma-encoded components — Cargo.toml's own description of the feature calls
that behaviour broken. It is now opt-in legacy compatibility; see
`docs/architecture/color-pipeline.md` §7.

The removal was paired with a second fix, because the feature was not merely
wrong but *unobservable*: the `aura` crate lists itself in its own
`[dev-dependencies]`, so every integration test inherited the full default set
and `cargo test --no-default-features` was a no-op. The workspace dependency
now sets `default-features = false`, which Cargo requires to be declared there
rather than at the dev-dependency. `tests/color/contract.rs` fails if either
half regresses.

### `minimal-headless`

No default features. Compiles `aura` with only the renderer-neutral core,
runtime, and widget crates reachable — no GPU backend, no windowing
backend, no software rasterizer. This is the profile CI uses to catch
accidental default-feature bleed into `core`/`runtime`/`widget` and is
exercised by `tests/smoke/workspace.rs`.

This command means what it says only because of the `default-features = false`
noted above. Before that, the `aura` dev-dependency re-enabled every default
behind it.

```bash
cargo check -p aura --no-default-features
```

### `profiling`

**Not yet defined as a standalone feature.** Phase 0 inherited two
observability-adjacent flags from the pre-fork feature set:

- `hot` (`aura_debug/hot`) — hot-patch/reload support via `cargo-hot`.
- `sysinfo` — system information queries used for basic diagnostics.

Phase 0 leaves these as-is rather than inventing an empty `profiling`
feature with nothing behind it (roadmap rule 9: no LOC padding, prefer
small APIs). A real `profiling` feature should be introduced by whichever
phase adds `aura-profiler` (Phase 15) and should gate that crate's
instrumentation hooks plus `runtime/src/diagnostics.rs` (Phase 2). Until
then, `hot` and `sysinfo` are the closest verified equivalents and are
included in the CI feature matrix under that name.

### `recovery`

**Not yet defined as a standalone feature**, for the same reason as
`profiling`. The pre-fork tree already keeps `tiny-skia` as a fallback
renderer behind its own feature flag (see roadmap rule 6), which is the
Phase 0-era stand-in for "recovery path." A dedicated `recovery` feature
gating GPU-loss recovery, quality-tier degradation, and the safe-shell
escalation path belongs to Phase 17 (`runtime/src/recovery.rs`,
`runtime/src/memory_pressure.rs`). Introducing the flag before that code
exists would fail roadmap rule 10 ("code compiles" is not completion).

## Removed in Phase 0

The following features referenced workspace members that do not exist in
this checkout (see `docs/architecture/path-map.md`) and were removed
rather than stubbed out, per roadmap rule 7 (no mass-renaming) and the
instruction to "restore a deleted capability only when a later phase
explicitly needs an Aura-owned replacement":

| Removed feature | Depended on missing crate | Notes |
|---|---|---|
| `debug` (root `aura` crate) | `aura_devtools` | `aura_winit/debug` and `aura_debug` themselves are untouched; only the root re-export/attach wiring is gone. |
| `time-travel` | `aura_devtools` | Time-travel debugging lived entirely in `aura_devtools`. |
| `tester` | `aura_tester` | Recording/playback developer tool. |
| `selector` | `aura_selector` | Gated `aura_runtime::widget::selector`; module deleted with the feature. |

`highlighter` (in both the root `aura` crate and `aura_widget`) is a
partial exception: `aura_highlighter` is missing the same way as the four
crates above, but its call sites in `widget/src/markdown.rs` (11 sites) and
`widget/src/text_editor.rs` (1 site) are load-bearing, working widget
code, not dead scaffolding — deleting them is out of Phase 0's scope
(repairing the workspace/CI, not editing widget internals). Both manifests
keep a `highlighter = []` feature declared with **no backing dependency**,
purely so the existing `#[cfg(feature = "highlighter")]` gates compile
without an `unexpected_cfgs` warning under `-D warnings`. This feature
must never be enabled — doing so would try to resolve `aura_highlighter`,
which does not exist in this checkout, and fail to build. It becomes real
again, with no source changes, the moment a later phase adds an
Aura-owned highlighter crate and points the feature at it.

`aura_debug`'s `enable` feature (telemetry client for `aura_beacon`) was
also removed for the same reason; `hot` was kept because `cargo-hot` is a
real crates.io dependency with no relationship to the deleted `beacon`
crate.

## Platform features

`x11` and `wayland` stay in `default` until the native VYOM/Smithay
backend (Phase 14) reaches parity, per roadmap rule 6. `webgl` remains
opt-in and Wasm-oriented; it is not part of any VYOM profile.

## Adding a new feature

1. State which crate(s) it gates and why the capability cannot live behind
   an existing feature.
2. Add it to the relevant profile table above.
3. Add its combination to `scripts/ci/check-workspace.sh` and the CI
   feature matrix in `.github/workflows/ci.yml`.
4. If it introduces a new optional dependency, confirm `deny.toml` allows
   its license and source.
