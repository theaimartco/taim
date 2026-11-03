# TAIM on Android

**Status: experimental.** Every TAIM crate builds and passes `clippy -D
warnings` for `aarch64-linux-android` (phones) and `x86_64-linux-android`
(the emulator). The touch, back-navigation and scrolling behaviour is
covered by tests that run on any machine. **Nothing here has run on a
physical device or an emulator yet.** Treat it as a first port that still
needs device testing before you ship it.

## Starting a new application

`taim-new` writes a project that builds for the desktop and Android, like
`flutter create`:

```sh
cargo run -p taim-new -- ~/apps/my_notes --name "My Notes" --package com.example.notes
cd ~/apps/my_notes
cargo run                              # on the desktop
cd android && gradle installDebug      # on a phone
```

Use `--taim-path <checkout>` to build against a local TAIM checkout instead
of the Git repository. The starter app is a notes list kept between
launches: a lazy `list`, a `navigator` with a detail page, a text field and
`taim::storage`. It pins TAIM's Rust release and Android targets in
`rust-toolchain.toml`.

## What applications are built from

| Need | TAIM | Flutter equivalent |
|---|---|---|
| Screens and back navigation | `Navigator::new(cx, home)`, `navigator(nav, \|cx, route\| …)`, `nav.push`, `nav.pop` | `Navigator` with routes |
| Page-local state | the `cx` each page is built with; freed when the page is popped | `StatefulWidget` |
| Long lists and feeds | `list(count, \|index\| …)`, which builds only items near the screen | `ListView.builder` |
| Settings that persist | `taim::storage::Store::open("settings")` | `shared_preferences` |
| Open links, share, vibrate | `taim::platform::{open_url, share_text, vibrate}` | `url_launcher`, `share_plus`, `vibration` |
| App files | `taim::platform::data_dir()` | `path_provider` |
| Pinch to zoom | `pinch(content).on_pinch(\|scale, focal\| …)` | `GestureDetector.onScale` |
| Pull to refresh | `pull_to_refresh(content, refreshing)` | `RefreshIndicator` |
| App structure | `scaffold`, `app_bar`, `nav_bar`, `tabs`, `drawer`, `sheet`, `dialog`, `snackbar` | Material scaffold widgets |

## Writing an application

An Android application is a Rust library with an `android_main` entry
point. The views are the same as on the desktop:

```rust
use taim::prelude::*;

pub fn app() -> taim::App {
    taim::app("Counter", |cx| {
        let count = cx.signal(0);
        button(move || format!("Tapped {}", count.get()))
            .on_press(move || count.update(|n| *n += 1))
    })
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android: taim::android::AndroidApp) {
    if let Err(error) = app().run_android(android) {
        log::error!("{error}");
    }
}
```

```toml
[lib]
crate-type = ["rlib", "cdylib"]

[dependencies]
taim = { path = "…", default-features = false, features = ["wgpu", "tiny-skia", "crisp", "hinting"] }

[target.'cfg(target_os = "android")'.dependencies]
taim = { path = "…", default-features = false, features = ["android"] }
log = "0.4"

[target.'cfg(not(target_os = "android"))'.dependencies]
taim = { path = "…", default-features = false, features = ["x11", "wayland", "linux-theme-detection"] }
```

`examples/mobile_shell` is a complete example. It is a desktop binary and
an Android library built from the same code.

## Building and installing

You need the Android SDK and NDK (Android Studio installs both), the Rust
targets and [`cargo-ndk`](https://github.com/bbqsrc/cargo-ndk):

```sh
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk
export ANDROID_HOME=~/Android/Sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/<version>
```

### With Gradle (APK or Play Store bundle)

`examples/mobile_shell/android` is a Gradle project. It runs `cargo ndk`
before each build and packages the library with the platform's
`NativeActivity`, so it has no Java or Kotlin code:

```sh
cd examples/mobile_shell/android
gradle installDebug          # build and install on a connected device
gradle bundleRelease         # an .aab for the Play Store
```

Open the folder in Android Studio to run, debug and profile it like any
other app. Before you publish, replace the debug signing configuration in
`app/build.gradle.kts`.

### With cargo-apk (quick debug builds)

```sh
cargo install cargo-apk
cargo apk run -p mobile_shell
```

The `[package.metadata.android]` section in `examples/mobile_shell/Cargo.toml`
configures it.

### Logs

Log records and panics go to logcat under the `taim` tag:

```sh
adb logcat -s taim
```

## Checking a release

TAIM does not use hosted CI. Before a release, on a clean checkout:

```sh
bash scripts/release/check.sh ~/taim-evidence/check-$(git rev-parse --short HEAD)
```

It runs formatting, desktop and Android lints, every test, a `taim-new`
smoke build and, with `cargo-ndk` and the NDK installed, an Android link.
It exits non-zero on a failure, and with code 3 when a step was skipped
(a check without the NDK has not linked anything for Android).

Then, with a phone connected over USB debugging (or an emulator running):

```sh
bash scripts/qa/android-device-check.sh ~/taim-evidence/device-pixel8
```

It builds and installs `examples/mobile_shell` (or the Android project you
name), then launches it, scrolls and flings, taps, presses back, sends it
to the background and back, rotates the screen and switches dark mode,
taking a screenshot at each step. It fails if the app dies or logcat shows
a panic or a native crash, and writes a report whose by-hand section
(typing, TalkBack, smoothness, copy and paste) a person completes. Release
policy asks for this on at least three physical devices with different GPU
vendors; see `docs/release/production-gates.md`.

## What the Android shell does

| Area | Behaviour |
|---|---|
| Touch | The first finger becomes the pointer (move, press, release), so every view works with touch, as with Flutter's unified pointer events. Views are told when the pointer is a finger. Every finger is also delivered as a raw touch event. |
| Pinch | `pinch(content).on_pinch(\|scale, focal\| …).on_end(\|scale\| …)` reports two-finger scaling. The second finger landing takes the gesture from the child, so nothing under it presses or scrolls. |
| Scrolling | Touch dragging with an 8dp slop. A drag that starts on a button scrolls and cancels the button, like Android's `onInterceptTouchEvent`. Flings decelerate. Touching a moving list stops it without pressing anything. |
| Pull to refresh | Takes a downward drag only when the content is at its top. Otherwise the list scrolls back first, as `SwipeRefreshLayout` does. |
| Back | The back button or gesture closes the topmost dialog, sheet or drawer. When nothing is open it finishes the activity, like the root route of a navigator. |
| Lifecycle | The GPU surface is dropped when the activity goes to the background and recreated when it returns. Application state is kept. |
| Safe area | Views are laid out inside the system bars, display cutouts and the on-screen keyboard, read from the activity's content rectangle. Edge-to-edge (Android 15) is handled. |
| Keyboard | The on-screen keyboard opens while a text field or editor has focus and closes when it loses focus. With the `android-game-activity` feature, the input method's own text buffer is followed, so composing (including Devanagari transliteration), suggestions, autocorrection and swipe typing work. The changes are replayed into the view as typed text and Backspace. |
| Screen reader | TalkBack reads the views' semantic tree through AccessKit, injected as the decor view's accessibility delegate (no Java code needed). Activate, focus, expand/collapse and scroll actions become the same input a finger would give. The tree is built only while an accessibility service is reading it. |
| Dark mode | The system light or dark setting is followed, including a switch while the app runs. |
| Fonts | Roboto and the system Noto families (Devanagari and other scripts) are loaded from `/system/fonts`. |
| Clipboard | Text and HTML through `ClipboardManager`. |
| Rendering | wgpu picks a Vulkan or OpenGL ES adapter. If the GPU path cannot start, the CPU renderer (tiny-skia) takes over. |

## Choosing the activity

| | NativeActivity (`android`) | GameActivity (`android-game-activity`) |
|---|---|---|
| Java code in the package | None | The AndroidX `games-activity` library and a small activity class |
| On-screen keyboard | Key events only: basic Latin typing, no composition or suggestions | Full input connection: composition, suggestions, swipe typing |
| Everything else | Same | Same |

Use GameActivity for any application where people type. To switch
`examples/mobile_shell`:

1. In its `Cargo.toml`, use `features = ["android-game-activity"]` instead of
   `["android"]` for the Android target.
2. In `android/app/build.gradle.kts`, add
   `dependencies { implementation("androidx.games:games-activity:4.4.0") }`.
   That is the version android-activity 0.6 is built against. Do **not**
   enable `prefab`: android-activity compiles its own copy of the C++ glue,
   and linking the upstream one as well breaks the build.
3. Add `app/src/main/java/com/theaimartco/taim/mobileshell/MainActivity.java`:

   ```java
   package com.theaimartco.taim.mobileshell;

   import com.google.androidgamesdk.GameActivity;

   public class MainActivity extends GameActivity {}
   ```

4. In `AndroidManifest.xml`, remove `android:hasCode="false"`, change the
   activity's `android:name` to `.MainActivity`, and use an AppCompat theme
   such as `@style/Theme.AppCompat.NoActionBar` (GameActivity extends
   `AppCompatActivity`).

GameActivity's native glue is C++ that the NDK compiles. It has not been
built in TAIM's own (NDK-less) checks, so the GameActivity keyboard path is
compiled against the same Rust API as NativeActivity but has not been
linked or run.

## Known gaps compared with Flutter

These are not done yet, roughly in order of how much they matter:

1. **Device verification.** Nothing has run on hardware or an emulator,
   and without an NDK in TAIM's checks not even linking is verified. Frame
   pacing, GPU drivers, the lifecycle, input, TalkBack and the keyboard all
   need testing on real phones.
2. **Screen reader depth.** Sliders cannot be adjusted from TalkBack yet
   (increment and decrement are not offered), live regions are not
   announced, and text fields do not expose their caret or selection.
3. **Text editing details.** A selection made in the view is not shown to
   the input method, so replacing selected text from a suggestion strip is
   not supported. The input method also does not see the text before the
   caret after each word, which weakens its next-word suggestions.
4. **Gestures.** Pinching is supported. Two-finger rotation, two-finger
   panning and long-press text selection are not.
5. **Platform services.** Links, sharing, vibration, files and settings
   are built in. There is no plugin system for the rest (camera, location,
   notifications, permissions at run time, in-app purchases); applications
   reach Java through `jni`, as `taim::platform` does.
6. **Networking and images from the web.** There is no HTTP client or
   network image view; use a Rust HTTP crate in `spawn` and show the result
   with `image`.
7. **Tooling.** There is no hot reload and no device-preview tooling.
8. **iOS.** Not started.
