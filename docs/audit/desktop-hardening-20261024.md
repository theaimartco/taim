# Desktop hardening evidence — 2026-10-24

Requested targets are Linux, Windows and macOS. Work remains uncommitted;
these results describe a working tree based on `13121da90`, not a signed
release candidate. No stable tag, publication or production approval exists.

## Functional verification (updated 2026-10-25)

Current local commands pass:

```sh
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo fmt --all -- --check
cargo check -p taim -p taim-ide -p taim_lab --target x86_64-pc-windows-msvc --locked
cargo check -p taim -p taim-ide -p taim_lab --target aarch64-apple-darwin --locked
python3 scripts/audit/check-upstream-boundary.py
python3 scripts/audit/exact-fork-blobs.py --strict --output /tmp/taim-final-exact-blobs.json
```

The cross-target checks run on Linux and do not certify OS runtime behavior,
linking, signing or accessibility. Earlier native-driver full tests recorded
2,334 passes with all features and 2,417 default-feature passes, five ignored
cases each. The default repeat excluded six benchmark-schema cases that
were included in the all-feature run. A fresh restricted-sandbox full replay
terminated with SIGSEGV in the GPU executable; do not record it as passing.
Earlier unrestricted native GPU tests passed all 89 cases. Sharing native
renderer instances avoids repeated loader setup, but is not proof that every
restricted-driver crash is resolved.

The core native API now includes window-scoped tasks, bounded window-thread
channels, visible and password-masked IME preedit, validated layout inputs,
scroll edge chaining and fatal presentation error propagation. Calendar/time
arithmetic now handles extreme integer offsets without overflow, and public
calendar fields are normalized before indexing month labels.

No GitHub Actions were used. No Iced Cargo dependency or exact inherited
source blob was detected; source provenance still requires semantic review.

## Visual baseline review

The existing HEAD commit `13121da90` intentionally changed built-in light
and dark palette seeds to TAIM blue/ink. It did not update the widget
snapshots. All 34 gallery cases consequently differed in both themes.
The harness now collects failures/artifacts for both themes instead of
stopping after light mode. Representative before/after renders were
inspected for enabled buttons, text inputs, slider/progress, calendar,
text sizes and app bars in both themes. The color/surface changes match
the palette commit; visible content and placement remain intact.

[Before/after comparison](taim-palette-review-20261024.png).

Only the 68 gallery baselines were adopted. Golden tolerances were not
changed; the full production/default golden suite then passed, including
non-gallery image tests. The gallery review run took 0.12 seconds before
adoption. Suite wall time is recorded for reproducibility, not advertised
as application performance or evidence of a speed improvement. Updating
baseline image files does not change rendering work at runtime.

## Remaining blockers

IDE and Lab now build on the native shell. Lab uses native views; IDE retains
its editor models through a local retained-widget adapter using owning crates.
The removed public application host was not restored. Further effort is
prioritized on core UI reliability rather than extra tooling polish.

The advisory audit flags `paste` (RUSTSEC-2024-0436), `zbus_names`
(RUSTSEC-2026-0371), `zvariant` (RUSTSEC-2026-0370), and `zvariant_derive`
(RUSTSEC-2026-0372). These are maintenance findings, not new claims of
exploitable vulnerabilities. The audit remains failing; dependencies were
not silently replaced or excepted to obtain a pass.

Only Linux executed here. Windows/macOS type checks passed; native OS builds, composition,
screen-reader behavior, device-loss/recovery, signing/packaging and
physical performance remain unverified. All release-owner, provenance
and independent device-evidence gates still apply.

## Core native UI boundary checks — 2026-10-25

Date/month arithmetic saturates at supported year boundaries, public malformed
calendar fields are normalized, and time offsets use wide arithmetic. Sliders
and progress bars normalize reversed/non-finite ranges, preserve finite drawing
fractions over the full f32 range and reject non-finite pointer coordinates.
Infinite steps are rejected. Four boundary regression tests were added.
`cargo test -p taim-view --locked`: **37 passed**, zero failures.
Whole-workspace strict all-feature Clippy passed after these changes.
These checks do not certify physical platform acceptance or source independence.
