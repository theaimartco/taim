# Path map

> Status: Phase 0 output. This is the ground truth for how this checkout's
> directories correspond to the module layout `phase.md` assumes. Update
> this file whenever a phase adds, renames, or removes a top-level crate.

## Actual top-level crates vs. the roadmap's expected modules

`phase.md` describes the target layout in terms of `core/`, `runtime/`,
`graphics/`, `wgpu/`, `widget/`, `tools/`, and `tests/`. This checkout is an
unmodified-in-structure `aura` fork (crate names still carry the `aura_`
prefix; roadmap rule 7 says not to mass-rename yet). The mapping is
one-to-one for every directory the roadmap names:

| Roadmap module | Actual directory | Actual crate name | Notes |
|---|---|---|---|
| `core/` | `core/` | `aura_core` | Matches. |
| `runtime/` | `runtime/` | `aura_runtime` | Matches. |
| `graphics/` | `graphics/` | `aura_graphics` | Matches. |
| `wgpu/` | `wgpu/` | `aura_wgpu` | Matches. |
| `widget/` | `widget/` | `aura_widget` | Matches. |
| `tools/` | *(does not exist yet)* | — | First created in Phase 1 (`tools/aura_bench`). |
| `tests/` | *(does not exist yet)* | — | First created in Phase 0 (`tests/smoke.rs`). |

Additional directories present in this checkout that the roadmap's module
table does not name, kept as-is because they are load-bearing for the
existing `aura` public API and have no roadmap replacement yet:

| Directory | Crate name | Role |
|---|---|---|
| `src/` | `aura` | Root/facade crate; re-exports the others as the public `aura` API. |
| `program/` | `aura_program` | `Program` trait and message plumbing shared by `Application`/`Daemon`. |
| `renderer/` | `aura_renderer` | Renderer-selection glue between `aura_wgpu` and `aura_tiny_skia`. |
| `futures/` | `aura_futures` | Executor/subscription primitives. |
| `debug/` | `aura_debug` | Pluggable debug/telemetry API (span timing, hot-reload hook). |
| `tiny_skia/` | `aura_tiny_skia` | Software rasterizer fallback (kept live per roadmap rule 6). |
| `winit/` | `aura_winit` | `winit`-based windowing/runtime backend (kept live per roadmap rule 6 pending Phase 14). |
| `benches/` | *(part of root `aura` package)* | `cargo bench` targets (`cpu`, `wgpu`), gated on `canvas`. |

## Workspace members removed in Phase 0

`Cargo.toml` listed six workspace members and one glob with no
corresponding directory on disk, which made every `cargo metadata` /
`cargo check` invocation fail before any feature work could start:

| Listed member | Crate name | Directory found on disk? | Action |
|---|---|---|---|
| `beacon` | `aura_beacon` | No | Removed from `[workspace.members]`, `[workspace.dependencies]`, and `debug/Cargo.toml`'s (former) `enable` feature. |
| `devtools` | `aura_devtools` | No | Removed from `[workspace.members]`, `[workspace.dependencies]`, and the root `debug`/`time-travel` features and their call sites in `src/daemon.rs` / `src/application.rs`. |
| `highlighter` | `aura_highlighter` | No | Removed from `[workspace.members]`, `[workspace.dependencies]`, and the root/`widget` `highlighter` features. Call sites in `widget/src/markdown.rs` and `widget/src/text_editor.rs` were already `#[cfg(feature = "highlighter")]`-gated and needed no source changes. |
| `selector` | `aura_selector` | No | Removed from `[workspace.members]`, `[workspace.dependencies]`, and `runtime`'s `selector` feature. Deleted `runtime/src/widget/selector.rs` and its `#[cfg(feature = "selector")] pub mod selector;` declaration in `runtime/src/widget.rs`. |
| `test` | `aura_test` | No | Removed from `[workspace.members]` and `[workspace.dependencies]`. This crate had zero references anywhere else in the tree — it was dead even before the directory went missing. |
| `tester` | `aura_tester` | No | Removed from `[workspace.members]`, `[workspace.dependencies]`, and the root `tester` feature and its call sites in `src/daemon.rs` / `src/application.rs`. |
| `examples/*` | *(glob)* | No (`examples/` absent) | Removed from `[workspace.members]`. Phase 18 recreates a scoped `examples/vyom_shell/`; nothing else in this roadmap needs a general `examples/*` glob before then. |

None of these six crates are needed by any phase before the point where
the roadmap explicitly reintroduces an Aura-owned equivalent (`selector`-like
widget querying is not yet scheduled; syntax highlighting has no scheduled
return; hot-reload/time-travel debugging is superseded by `aura-profiler`
and Aura Lab in Phase 15). Per the roadmap's own instruction ("restore a
deleted capability only when a later phase explicitly needs an Aura-owned
replacement"), Phase 0 did not stub or recreate any of them.

See `docs/architecture/feature-policy.md` for the full list of Cargo
features that were removed alongside these crates, and why `hot` (which
depends on the unrelated, still-present `cargo-hot` crate) was kept.

## Verification

```bash
cargo metadata --format-version 1 --no-deps
```

succeeds from a clean checkout with no "failed to read `.../Cargo.toml`"
errors, confirming every remaining `[workspace.members]` entry and
`path = "..."` dependency resolves to a real directory.

## Phase 3: `graphics/src/primitive.rs` does not exist

Phase 3's file plan lists `graphics/src/primitive.rs` ("Modify: stable
subtree/display-list identity hooks"). No such file exists in this
checkout, and `graphics/src/lib.rs` has no `primitive` module at all.
The closest same-named file is `wgpu/src/primitive.rs`, but it defines a
backend-specific system for embedding raw `wgpu` draw calls inside a
custom widget (`Primitive`/`Pipeline`/`Storage`, used by `Shader`-style
widgets) -- an unrelated concern to "stable subtree/display-list
identity," which is about the renderer-neutral retained-layer system
Phase 4 builds (`graphics/src/display_list.rs`, not yet created).

Per Phase 0's own instruction ("do not create duplicate modules just to
match this document"), Phase 3 does not create a `graphics/src/primitive.rs`
stub or repurpose `wgpu/src/primitive.rs`. The identity hooks a retained
display list will need are Phase 4's responsibility, built alongside the
display list type itself rather than speculatively added a phase early
with nothing yet to attach to. Phase 3 instead delivers its structural
identity work entirely through `core::widget::Id`/`Key` (Phase 2),
`runtime::reconcile` (this phase), and `core::widget::tree::{Revision,
DirtyFlags}` (Phase 2) -- none of which require a `graphics`-crate change.
