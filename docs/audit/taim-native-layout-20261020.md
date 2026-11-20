# TAIM native layout: independent semantic migration checkpoint

**Candidate implementation commit:** `fdbe21533011bed8e4b9c8687a04b1704f973f93`
**Baseline:** Iced fork `81afda72f374386036a5b96a55f57a3dcfb0b653`
**Branch:** `feature/taim-native-layout-20261020`
**Release verdict:** NO-GO

## Substantive implementation changes

- `Position::resolve` is a platform-neutral TAIM placement contract.
  It separates absolute virtual-desktop coordinates from monitor-relative
  coordinates; it enforces finite inputs, supports callbacks, and centers
  oversized windows without negative top-left offsets.
- `taim_winit` now consumes this contract in the actual `position` shell
  conversion. The shell retains per-monitor DPI conversion, physical monitor
  origin addition and saturating physical coordinate arithmetic.
- `Rotation::apply` now uses exact cardinal angle branches for common
  static rotations, preventing sine/cosine rounding drift from spuriously
  changing layout bounds at 90, 180, 270 and 360 degrees. Dynamic angles
  retain the original geometric path. No performance claim is made.
- `pane_grid::Direction::probe` moves the navigation hit test to the
  midpoint just beyond an edge. A split neighbor is selected by the
  location adjacent to the focused pane's center instead of its top corner.
  This changes observable pane-keyboard-navigation behavior.

## Test additions

Five core placement unit tests, four rotation tests, one direction probe
test and two pane-grid navigation tests are committed, including a
split-neighbor behavioral regression. They are **unexecuted** in the
assistant's environment, which has no Rust toolchain.

### Required checks in a clean checkout

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
python3 scripts/audit/source-lineage-gate.py --strict --output /tmp/taim-lineage.json
```

The strict source-lineage check is expected to reject promotion, because
inherited source remains. A passing static or synthetic test does not
substitute for real Wayland/VYOM, multi-monitor DPI, GPU reset, IME,
accessibility or same-hardware performance testing.

## Full Git-tree same-path blob comparison

Recursive Git-tree inventories were read for both commits with
`truncated: false`. File comparison counts only byte-identical Git
blobs at the same path with extensions `.rs`, `.wgsl`, `.toml`.

| Candidate | Exact same-path Iced-origin blobs |
| --- | ---: |
| Identity-branch checkpoint | 60 |
| Stream/overlay continuation | 58 |
| New window/layout commit | **55** |

The three removed exact matches at the latest checkpoint are
`core/src/window/position.rs`, `core/src/rotation.rs` and
`widget/src/pane_grid/direction.rs`. Their historical API shapes
remain Iced-derived. These changes add TAIM-specific functionality
but **do not prove 100% independently authored source**.

## Unresolved independent-source and release gates

- **55 same-path exact blobs** remain.
- Edited inherited code, moved fragments, high-similarity tokens and
  licensing provenance require the stricter scanner plus manual audit.
- Major remaining families: core keyboard/input/window, async backends,
  renderer geometry and WGSL shaders, application/runtime facades,
  widgets and platform-specific code.
- GitHub Actions issue #2 blocks executable hosted CI verification.
- Production hardware and accessibility validation are unapproved.
- Preserve Iced MIT LICENSE, CREDITS.md and all historical Git ancestry.

Never advertise 100% TAIM source independence, v1.0 readiness or fastest
GUI framework status from this checkpoint.
