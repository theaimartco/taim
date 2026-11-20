# Changelog

All notable changes to this project are recorded here. Versions follow
[Semantic Versioning](https://semver.org); before 1.0 a minor version may
break the public API.

## [Unreleased]

### Added

- iOS services (`taim::apple`, `docs/platform/ios.md`): the share sheet
  (`platform::share_text`, which returned `Unsupported`), Taptic Engine
  haptics, Face ID/Touch ID status and prompts, keychain items with
  this-device-only and biometric access, local notifications with
  permission, scheduling, badges and open/dismiss/foreground events (the
  launch tap included), when-in-use location (one-shot and watch streams),
  the system photo picker and camera capture with camera permission.
- iOS: the root view is laid out inside the safe area, and the shell follows
  the system light/dark appearance and Reduce Motion.
- `scripts/ios/package.py` builds signed device apps and `.ipa` files
  (`--target aarch64-apple-ios --mode development|ad-hoc|app-store`),
  checking the provisioning profile against the mode, embedding it and
  deriving entitlements from it; adds Info.plist usage descriptions
  (`--usage`), an asset-catalog app icon (`--icon`), version and build
  numbers, and links Simulator entitlements so the keychain works there.
- `examples/apple_services_probe`, a Simulator/device/Mac fixture for the
  Apple services; the release check lints it on every target.
- macOS services under the same `taim::apple` functions
  (`docs/platform/macos.md`): the `NSSharingServicePicker` share picker
  (`platform::share_text`, which returned `Unsupported`), Force Touch trackpad
  haptics, Touch ID status and prompts, the keychain (data-protection
  keychain when signed with an access group, else the login keychain), local
  notifications with events, location, the open panel as the media picker,
  and camera permission. Reduce Motion is followed. `capture_media` is
  `Unsupported` on macOS.
- `--size` for the macOS and iOS packagers builds with `release-small`.
  `scripts/macos/apple-services-probe.toml` packages the services probe.

- Motion across the views (`docs/design/motion-language.md`). Buttons and chips
  squish under the finger and spring back. Switches slide their knob and
  stretch it while held. Checkbox ticks and radio dots pop in. The segmented
  control's indicator stretches towards its target, catches up and bounces,
  and it now follows outside changes to the selection. A slider thumb swells
  while dragged. `scroll` and `list` rubber-band past their ends and bounce
  when a fling hits one. New `animated(view)` glides content to wherever
  layout moves it, and `appear(view)` fades and rises it in, with `.delay`
  for cascades. Every one of these respects Reduce Motion.
- `View::visual(&VisualCx)` lets a view move, scale or fade itself and its
  subtree when drawn. `VisualCx` gives the frame time, the view's frame in
  its parent and its window bounds.
- Navigator transitions. `navigator(..).transition(PageTransition::…)`
  offers `Slide` (the default), `Zoom`, `Fade`, `Rise` and `None`:
  - `Slide` is the iOS push: the page underneath drifts with parallax and
    darkens, and the moving page casts a shadow. A swipe from the left edge
    drags the top page back with the finger. Let go past 40% of the width,
    or flick it, and it goes back at the finger's speed; otherwise it
    returns.
  - `Zoom` grows the new page in while it fades in. `Rise` brings the new
    page up from the bottom while the old one recedes.
  - With Reduce Motion, pages change at once, as before.
- Shared elements: `hero(tag, view)`. While the navigator moves between two
  pages that both have a hero with the same tag, the hero flies from one
  place to the other, growing or shrinking on the way, at full opacity even
  over a fading page. It also flies during an edge swipe, following the
  finger. `Renderer::start_opacity_reset` (wgpu, tiny-skia) lets a subtree
  ignore the opacity it inherits.

- Live liquid glass (`docs/design/liquid-glass.md`): a press lights the
  rim nearest the finger, magnifies what is under it and swells the pane on
  a spring; `.interactive()` panes also stretch after a drag and wobble
  back. `glass_group` melts nearby panes into one another through a smooth
  signed-distance union. The scene light leans towards the pointer on
  desktops and with the device's tilt on iOS. Glass keeps text on it
  readable over any backdrop. Reduce Motion stops the movement.
  `SceneLight::leaned`, `material::Liquid` and `PaintCx::fill_liquid` are
  new, and `renderer::Quad` has a `liquid` field.
- `examples/liquid` has glass transport controls that melt together when
  pressed.

### Changed

- `[profile.release]` uses fat LTO instead of thin: 13% smaller executables
  (counter on macOS 6.05 → 5.26 MiB, iOS 6.47 → 5.77 MiB) at the same frame
  cost. New `taim-new` projects and generated Android projects do the same.
- `[profile.release-small]` keeps the rasterizers at `opt-level = 3`. Before,
  everything was built for size, which made software rendering 5x slower;
  it is now about 25% slower than `release`, for 41% smaller executables
  (counter on macOS 3.58 MiB, iOS 4.00 MiB). New `taim-new` projects get it.
- Store packaging (`docs/release/store-submission.md`):
  `scripts/windows/package.py` writes Microsoft Store MSIX packages
  (full-trust, static C runtime, logos from one PNG) and
  `scripts/linux/package.py` writes strictly confined core24 Snapcraft
  projects for the Snap Store. Neither has been run on Windows or Ubuntu.
- Release builds of `taim-new` projects and the examples no longer open a
  console window on Windows.
- **Breaking:** `graphics::Compositor::create_surface` returns
  `Result<Surface, backend::Error>`. A window the platform will not give a
  surface now fails with `Error::GraphicsCreationFailed` instead of
  panicking.

### Fixed

- The getting-started guide taught the removed `Message`/`update` API and
  `cargo add taim`, which fails; it is rewritten for the view API and its
  examples are now compiled as doctests. The README's performance table
  claimed zero allocations per frame, and listed latency, startup and idle
  figures that were never measured; it now shows only measured numbers,
  each linked to `docs/performance/numbers.md`. The accessibility guide no
  longer claims VoiceOver and Narrator support, which does not exist yet.
- Settings saves now fsync the directory after the rename, so power loss
  right after a save cannot bring back the previous file.
- Keychain: replacing an item deleted the old secret before adding the
  new one, so a failure (no passcode, missing entitlement, unsigned Mac)
  lost the stored credential. The new item is now tried first.
- macOS: bundled apps moved their data directory from the executable's
  name to the bundle ID without moving the data, so an update looked like
  a reset. The old directory is moved across once.
- Android: a keyboard snapshot that Rust rejected (too long, or a secure
  field) left Java one revision ahead, after which every programmatic
  change to that field was ignored.
- `appear()` content not yet drawn (rows below the fold) kept the window
  redrawing every frame while idle.
- An edge swipe that the system cancelled (a call, the notification
  shade) popped the page if it was past 40%; it now springs back.
- Heroes built by `each`, `list`, `when` or `rebuild` never flew between
  pages.
- Glass kept its hover light after the pointer left the window.
- Settled springs stopped a hair short of their target, so animated views
  kept an extra layer every frame.
- Web: the counter crashed in every browser. The liquid glass pass needs
  more vertex attributes than WebGL2 has, its shader failed, and the
  software fallback panicked on the canvas. Where the pass does not fit,
  liquid glass is drawn as plain glass.
- Android builds failed `-D warnings`: a misplaced `#[cfg]` left
  `lean_light` unused there.
- Quad and text pipelines were compiled twice at startup.
- Font data with no usable face is logged instead of dropped silently.
- iOS apps ran at 60 Hz on ProMotion iPhones: `scripts/ios/package.py` now
  sets `CADisableMinimumFrameDurationOnPhone`, so they can reach 120 Hz.
  `TAIM_FRAME_TRACE` lines now include the gap since the previous frame.

- Notification scheduling and badges report `ServiceError::Denied`, not
  `Failed`, when the user has not allowed notifications.

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
- Web services: the clipboard (`navigator.clipboard`), `taim::storage` in
  `localStorage`, `open_url`, `share_text` (Web Share) and `vibrate`.
- iOS services: the clipboard (`UIPasteboard`), `open_url` (UIKit),
  `vibrate` and the sandbox data directory; the desktop implementation,
  which spawned `xdg-open` and wrote to `~/.local/share`, no longer applies.
- The release check lints the web, Windows, macOS, iOS and iOS-simulator
  targets, the portable crates' tests included, and runs a browser smoke
  test (`scripts/web/smoke.js`).

### Changed

- Text fields are as tall as buttons (9 px vertical padding, was 6) and
  share their 8 px corner radius, so a field and its button line up.
- Checkboxes show a tick instead of a filled square; an open accordion
  section's chevron points down; the date picker's arrows are icon
  chevrons.
- The chosen item in the floating navigation bar is a gentler lift on dark
  themes (3% white, was 10%: blending in linear light made it a bright grey).

### Fixed

- A panic inside `Signal::update` or a `Memo` computation poisoned the
  signal's lock, and every later read or write of it panicked; the signal
  now stays usable and a memo recomputes on its next read.
- A panic in the task scheduler's notifier poisoned its lock, after which
  every wake of every task panicked; later wakes now enqueue and notify.
- The software renderer aborted the process when the platform refused a
  presentation context, even though it is the last fallback when no GPU
  works; `Compositor::new` now returns that as an error, and
  `taim_tiny_skia::window::compositor::try_new` is the fallible
  constructor.
- Windows: `open_url` went through `cmd /C start`, which cut a URL at `&`
  and ran the rest as a command; it now uses the URL protocol handler,
  with no shell.
- Each crate lints clean on its own (`taim_winit` and `taim_wgpu` relied on
  features turned on elsewhere in the workspace).
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
