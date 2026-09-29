# Virgo IDE — product and implementation plan

## Goal

Build Virgo as a dependable, native desktop IDE on Taim. “More mature than VS Code” is a product target, not a current claim: it requires feature coverage, reliability, accessibility, ecosystem compatibility, and measured performance on real projects. The existing `examples/ide` application is a visual prototype with many simulated workflows. Each capability below is complete only when it operates on a real workspace and passes its acceptance checks.

## Product principles

- Never report a successful save, build, commit, debug session, or extension install unless the underlying operation succeeded.
- Keep user data safe: dirty tabs, external file changes, atomic saves, recovery, and clear errors take precedence over visual polish.
- Keyboard first, accessible controls, predictable layouts, and usable behavior on small and large projects.
- Trust is explicit for workspaces, commands, extensions, and remote code.

## Feature coverage and acceptance gates

| Area | Required capability | Acceptance gate |
| --- | --- | --- |
| Workspace | Open folder/file, multi-root workspaces, recent projects, tree refresh, excludes, file create/rename/delete, drag and drop | Changes appear on disk; errors and permissions are surfaced |
| Editing | Tabs, split panes, UTF-8 and other encodings, line endings, large files, undo/redo, selections, find/replace, autosave, format on save, recovery | Round trip preserves data; dirty tabs cannot be discarded silently |
| Navigation | Quick open, command palette, symbols, references, definitions, breadcrumbs, outline, go to line | Real project locations open correctly |
| Search | Whole workspace, case/word/regex, include/exclude, preview and replace with undo | Unsaved buffers take precedence; replacement is reviewable |
| Languages | Highlighting, LSP lifecycle, completion, hover, diagnostics, rename, code actions, semantic tokens | Tested against real language servers and malformed responses |
| Terminal and tasks | Real PTY, multiple sessions, shell integration, build/test/run tasks, cancellation | Output, exit status, environment and working directory match process |
| Source control | Git status, diff, stage/unstage, commit, branches, history, conflicts | State agrees with Git CLI; no fabricated commits |
| Debugging | DAP adapters, launch configs, breakpoints, stepping, variables, call stack, console | Real process stops and resumes at requested locations |
| Extensions | Signed or verified packages, permissions, sandboxing, updates, compatibility, searchable catalog | Installed extension runs and can be disabled/uninstalled safely |
| Remote and collaboration | SSH/containers, remote FS/process, shared sessions, conflict handling | Edits and tasks target the intended host; disconnect recovers cleanly |
| Settings and UX | Themes, fonts, keymaps, layouts, command discovery, localization, screen readers | Settings persist and keyboard/screen-reader flows work end to end |
| Platform | macOS, Windows, Linux packaging, updates, crash reports, telemetry opt-in | Clean install, update and rollback pass on each OS |
| Quality | Unit/integration/UI tests, benchmark suite, fuzzing, security review, migration tests | Release gates publish reproducible results, not marketing estimates |

## Delivery sequence

1. **Truthful workspace foundation:** rename the app Virgo; open a chosen folder; load files from disk; save to disk with visible errors; search real files and unsaved buffers. Verify with temporary workspaces and UI state tests.
2. **Data-safe editing:** tab lifecycle, Save As, atomic writes, external-change detection, recovery journal, create/rename/delete, refresh, large-file limits. Verify crash and concurrent-edit cases.
3. **Execution and Git:** replace simulated build/terminal/Git with actual process and Git integrations. Add cancellation, streaming output, status, diff and commit tests.
4. **Language and debug:** LSP client, diagnostics/completion/navigation, DAP client and adapters, project-specific settings. Verify against real servers/adapters.
5. **Extensions, remote and release:** permissioned extension host, marketplace, remote transport, packaging, accessibility audit, cross-platform CI, performance baselines.

## Current implementation boundary

`examples/ide` already provides a visual shell, editor widget, tabs, panes and many demo-only panels. Demo content and simulated actions must not be represented as production integrations. This plan is the coverage checklist; passing one milestone does not imply the whole IDE or VS Code parity is complete.

Run the current Virgo workspace build with `cargo run -p taim-ide -- /path/to/project` (or omit the path to open the current directory). The current build has a graphite dark palette, monochrome syntax highlighting, bundled SVG workbench icons, a compact workspace shell, disk-backed file open/create/save, Save As without overwriting an existing path, file and directory rename, confirmed recoverable delete and in-app trash restore, automatic explorer refresh, file quick open, workspace text search with case/word/regex flags and include/exclude globs, reviewed workspace replace with conflict validation and guarded undo, an editor line-number gutter, draggable vertical scrollbar and clickable content minimap, actual Cargo check/build, real Git status/stage/unstage/diff/commit in repositories, and a PTY-backed terminal with multiple independent sessions, raw keyboard input, scrollback and VT alternate-screen support. Save detects external file changes and uses a temporary file plus rename to replace existing content. Dirty buffers now have private recovery snapshots with explicit Restore/Discard at next launch; external edits remain protected. External workspace changes reload clean files while preserving dirty buffers. The initial tree scans 16 levels and search reads UTF-8 files up to 1 MB. Force-kill recovery verification, horizontal scrolling, per-match replace, terminal cell styling/copy/links, advanced Git history/conflict workflows, LSP, debugging, extensions, remote workflows and platform installers remain on the checklist.

The desktop build currently selects Taim's software renderer by default while a black GPU-presented window is investigated. Set `TAIM_BACKEND=wgpu` to opt into the hardware path. A headless first-frame test checks that the Virgo workspace draws visible pixels; a windowed visual check is still required on the affected machine.

The software compositor is optimized in the normal development profile. On a 1200×800 headless workspace render, a local debug-build probe measured a median frame time of ~5.7 ms after optimization, compared with ~45 ms before; actual desktop frame rates depend on display and project size. Search over larger workspaces is debounced and scanned on a blocking worker so typing stays responsive; older searches cannot replace newer results, and the sidebar displays at most 200 hits while retaining the full count. Search results navigate to the matched file and position, and workspace tabs can be closed down to an empty editor without losing dirty buffers.
