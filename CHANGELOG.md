# Changelog

All notable changes to this project are recorded here. Versions follow
[Semantic Versioning](https://semver.org); before 1.0 a minor version may
break the public API.

## [Unreleased]

### Added

- Web: applications run in the browser (`scripts/web/build.sh`,
  `docs/platform/web.md`). The canvas mounts into `#taim` or the page body,
  the GPU device is created asynchronously, Fira Sans is embedded, panics
  reach the console, and WebGL2 works (glass and effects are drawn flat
  there, since a WebGL canvas cannot be read back).
- `nav_bar` takes an `Icon`
  (`nav_bar([(Icon::HOME, "Home")], tab)`); it is drawn in the built-in
  icon font, so it shows on every platform.
- `Icon::name`; `icon_button` announces it to screen readers (it announced
  nothing before).
- `Container::background_themed(light, dark)` and `TextField::plain`.
- The release check lints the web, Windows, macOS and iOS targets.

### Changed

- Text fields are as tall as buttons (9 px vertical padding, was 6) and
  share their 8 px corner radius, so a field and its button line up.
- Checkboxes show a tick instead of a filled square; an open accordion
  section's chevron points down; the date picker's arrows are icon
  chevrons.
- The chosen item in the floating navigation bar is a gentler lift on dark
  themes (3% white, was 10%: blending in linear light made it a bright grey).

### Fixed

- Gradients were never drawn on the web: the pipeline was compiled out for
  wasm.
- The search field kept a light background on dark themes; its text field
  drew a second frame inside it.
- The search, clear and time-picker arrow glyphs, and the examples'
  navigation glyphs, were symbols missing from the bundled font and showed
  as boxes where the system had no fallback (Android, the web).
- iOS and Windows builds: keyboard conversion, the Unix display check,
  clipboard and a lint failed to compile or pass.
- `std::time::Instant` panicked on the web; time comes from `web-time`.

## [0.15.0-rc.1] - 2026-11-03

Release candidate. Automated checks pass on Linux x86-64 and the Android
targets build; the Android device gate and release sign-off in
`docs/release/production-gates.md` are pending. See
`docs/release/0.15.0-rc.1.md` for what that means for adopters.

### Breaking

- Applications are built from views that own their state (`taim::launch`,
  `taim::app`, `taim::prelude`). The Elm-style `application` builder, the
  `program` crate and the `taim::widget` / `taim::advanced` facade are gone;
  the engine crates (`taim_core`, `taim_widget`, `taim_runtime`) remain for
  custom widgets.

### Added

- Android: the `android` (NativeActivity) and `android-game-activity`
  features, `App::run_android`, GPU surface handling across the activity
  lifecycle, touch, back navigation, safe-area insets, the on-screen
  keyboard (with input-method composition under GameActivity), dark mode,
  system fonts, clipboard, TalkBack through AccessKit, multi-touch.
- Views: `navigator` (route stack with page-local state and transitions),
  lazy `list`, `pinch`, `glass`, plus an intercept phase so touch drags
  scroll and cancel presses, flings, and back handling in overlays.
- Liquid glass: frosted through a mip chain of the backdrop, a lensing rim
  with dispersion, vibrance, sheen and shadow; `Material::LIQUID` and
  `LIQUID_CLEAR`. The navigation capsule, dialogs and sheets use it.
- `taim::storage` (persistent settings, atomic writes) and `taim::platform`
  (open URL, share, vibrate, data directory).
- `taim-new`, a project generator for desktop and Android apps, with a
  pinned Gradle wrapper.
- Release tooling without hosted CI: `scripts/release/check.sh`,
  `scripts/qa/android-device-check.sh`, `scripts/qa/firebase-test-lab.sh`,
  `scripts/cloud/setup-android.sh`.

### Changed

- Secondary text and quiet icons meet WCAG AA contrast (4.5:1 text, 3:1
  icons) in every theme; they measured 1.4–2:1 before.
- Segmented controls use a raised thumb; the navigation bar is a floating
  capsule.
- A GPU out-of-memory on present recreates the surface (three tries)
  instead of aborting.

### Fixed

- The workspace, its tests, benches and tools build again after the move to
  views; `taim_lab` and `examples/trivial` are ported.
- Buttons did not respond to touch.
- Android had no text: system fonts were not loaded.
- Routing an event was quadratic in a parent's children (3.36 ms per
  pointer move over a 1,000-row list, now 86 µs); rows scrolled out of view
  were painted (338 µs per frame, now 27 µs).
- `taim_wgpu` with `svg` but without `image` did not compile.
- A finger resting before it lifted still flung a list.

### Known limitations

- Not yet run on an Android device or emulator.
- `examples/ide` is not ported to views and is excluded from the workspace.
- No plugin system for camera, location, notifications or runtime
  permissions; no HTTP client or network image view; no hot reload; no iOS.
