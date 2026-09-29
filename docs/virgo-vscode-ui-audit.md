# Virgo vs VS Code: UI and workflow audit

Updated: 2026-09-21. Baseline: current Virgo workspace mode in `examples/ide`, compared with Microsoft's current [workbench UI](https://code.visualstudio.com/docs/editing/getting-started/userinterface), [core editor features](https://code.visualstudio.com/docs/editing/getting-started/overview), [source control](https://code.visualstudio.com/docs/sourcecontrol/overview), [terminal](https://code.visualstudio.com/docs/terminal/basics), [themes and icons](https://code.visualstudio.com/docs/configure/themes), and [accessibility](https://code.visualstudio.com/docs/configure/accessibility/accessibility) documentation. A visual shell alone does not establish feature parity.

| Surface | VS Code baseline | Current Virgo | Gap / acceptance gate | Priority |
| --- | --- | --- | --- | --- |
| App identity and hierarchy | Stable workbench regions, discoverable commands | Graphite shell with title, activity rail, explorer, editor, panel, status bar | Test clear active states and avoid controls that imply unfinished features work | P0 |
| Icons | Consistent product and file icon systems with theme support | Mixed Unicode glyphs with different weight, alignment and font fallback | Bundle consistent monochrome vector icons; check small sizes on software renderer | P0 |
| Integrated terminal | Workspace-root shell, terminal tabs, command interaction, shell profiles | Workspace-root PTY sessions with keyboard input, scrollback and VT screen; separate Build output | Add cell styling, selection/copy, links, dynamic width, shell integration and terminal search | P1 |
| Search | Project-wide search, replace and navigation | UTF-8 files up to 1 MB, dirty buffers, case/word/regex flags, include/exclude globs, first 200 rendered hits, reviewed replace with guarded undo | Add large project indexing and per-match replace controls | P1 |
| Editor | Tabs, groups, line-number gutter, scrolling, minimap, find/replace, rich language editing | Disk-backed UTF-8 editor, two-pane split, protected tabs, Save As, line numbers, interactive vertical scrollbar, clickable code minimap and draft recovery | Add horizontal scrolling, wrapping controls, session/layout persistence, encodings, more groups, navigation history | P1 |
| Explorer | CRUD, drag/drop, filters, open in terminal | Open/create, file and directory rename, recoverable delete and restore, folder expand, manual and automatic refresh | Add context actions, configurable ignore rules and drag/drop | P1 |
| Panel and layout | Movable/resizable panels and secondary sidebar | Fixed-width sidebar and two terminal heights | Free resizing, panel placement, layout persistence | P1 |
| Status and feedback | Live file/language/status context | Basic status bar and notifications | Accurate task/process indicators, progress, error navigation | P1 |
| Keyboard and accessibility | Keyboard navigation, screen reader and high contrast | Common workbench shortcuts, focused palette/search/find fields and theme modes | Custom keymap, full focus order, semantic labels, visible focus, screen reader verification | P0 |
| Source control | Live Git status/diff/stage/commit | Real repository status, stage/unstage, diff and commit in workspace mode | Branch switching, history graph, conflict resolution, blame, multi-repository workflows | P1 |
| Languages/debug/extensions | LSP, DAP, extension ecosystem | Demo-only UI hidden in workspace mode | Real protocol clients and lifecycle testing before surfacing controls | P2 |
| Performance | Large-workspace responsiveness | Software renderer optimized; search scanning background for large trees | Measure startup, typing and memory across large projects on actual displays | P0 |

## Next implementation order

1. Finish keyboard focus, screen-reader labels and native performance measurements.
2. Add remaining editor navigation controls and large-workspace indexing.
3. Build and test real language-server and debugger protocol clients before exposing their controls.
4. Verify recovery behavior with force-kill tests, concurrent windows and large dirty buffers.

## Visual review of the 1200 × 800 workspace frame

The captured Virgo frame has a clear editor-first layout and a restrained grayscale palette. Its workbench regions are recognisable at a glance. The initial capture exposed weak text contrast in the top search/control buttons, active explorer row and terminal CTA; custom graphite styles now make these labels visibly brighter. The bottom panel still uses roughly a quarter of the window even before a shell opens. Long file names lack a tested truncation/tooltip strategy. Active tabs rely mainly on a gray fill and the button groups offer little separation. The editor now has a line-number gutter with the current line highlighted, an interactive vertical scrollbar and a clickable minimap drawn from document content. It still lacks a current-line background and a discoverable per-file language/encoding menu.

The previous rail mixed unrelated Unicode symbols; font fallback changed their stroke widths and vertical alignment. The new rail and explorer use locally bundled, single-stroke SVGs with one grayscale ink value. SVG handles are cached so the icon bytes are not hashed or allocated on each frame. The remaining command/action glyphs still need the same treatment, plus accessible names for icon-only controls.

## Interaction and reliability review

- **Discoverability:** The workspace title bar exposes Save, Check, Build, Split and Panel, but lacks a menu structure, shortcut hints and consistent hover labels. Terminal entry is now in the panel and command palette. Audit every command against the palette and actual handler.
- **Feedback:** Cargo tasks report completion but the panel has no running/progress indicator or cancellation control. Errors currently appear in the log and toast; clicking a compiler location does not yet reliably navigate to the source. Git errors are reported from the CLI and the Source Control view can refresh its actual status.
- **Editing safety:** Disk snapshots prevent accidental overwrite after external changes and dirty tabs cannot close silently. Save As creates a new path without replacing an existing file; file rename keeps dirty buffers attached to the new path. Both workflows keep the open document consistent when the destination is outside Explorer's scan depth or filtered from its tree. Dirty buffers now get private, atomic recovery snapshots after 250 ms without further edits and on normal shutdown. Explorer offers explicit Restore and Discard actions at next launch. Restored drafts remain dirty, and external disk edits still block normal save. Recovery is not yet validated by force-kill, concurrent-window or large-file tests; a crash before the debounce completes can lose the most recent edit. Read-only files, encoding detection and line ending round-trip remain gaps.
- **Workspace changes:** A two-second metadata fingerprint detects external additions, changes and removals. Clean open files reload, removed clean tabs close and dirty buffers remain untouched. File and directory rename reconcile open child tabs. Confirmed delete moves the entry to hidden `.virgo-trash`; dirty files block the operation. Explorer can restore entries to their original path and refuses to overwrite a recreated path. A replaced parent symlink cannot make polling read outside the workspace. Native event watching remains open.
- **Terminal depth:** The panel owns a persistent PTY process per tab, starts at the workspace root, forwards focused keyboard input including control keys, supports scrollback and two panel heights, and uses Alacritty's VT screen for cursor addressing and alternate-screen programs. Its current text widget does not render ANSI cell colors or attributes. Dynamic width, selection/copy, links, split terminals, shell integration and terminal search remain open before claiming terminal parity with VS Code.
- **Scalability:** The current renderer stays in single-digit milliseconds in a 1200 × 800 headless frame probe, but actual display latency and large-file behavior need measurement. Search scans large trees off the UI thread; explorer startup still recursively walks up to 16 levels.
- **Accessibility:** Small monochrome icons need meaningful accessible names and focus states. Current high-contrast theme and zoom controls alone do not satisfy keyboard-only or screen-reader acceptance. Test activity rail, command palette, tab close and terminal input in a fixed focus-order walkthrough.

## Release gates before a superiority claim

1. Recruit representative users to complete open/edit/search/build/terminal/Git tasks in both editors; record success, time, errors and recovery rather than subjective impressions alone.
2. Run native visual checks at compact laptop size, 1200 × 800, and high-density large display; check hover/focus/disabled/empty/error/loading states.
3. Verify screen reader labels and keyboard-only task completion on each supported OS.
4. Benchmark cold start, typing latency, 100k-file tree load, 10 MB edit, terminal flood and memory with repeatable environments.
5. Publish only capabilities that work against a real workspace and remove fabricated metrics or example data from production surfaces.

## Quality bar

The goal is to surpass VS Code in the workflows users actually rely on, measured with real projects and task completion, not claimed from mock panels or synthetic comparisons. Any missing capability above remains a product gap until its acceptance test passes.
