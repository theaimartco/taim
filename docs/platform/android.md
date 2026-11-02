# TAIM on Android

**Status: experimental.** Every TAIM crate builds and passes `clippy -D
warnings` for `aarch64-linux-android` (phones) and `x86_64-linux-android`
(the emulator). The touch, back-navigation and scrolling behaviour is
covered by tests that run on any machine. **Nothing here has run on a
physical device or an emulator yet.** Treat it as a first port that still
needs device testing before you ship it.

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

## What the Android shell does

| Area | Behaviour |
|---|---|
| Touch | The first finger becomes the pointer (move, press, release), so every view works with touch, as with Flutter's unified pointer events. Views are told when the pointer is a finger. |
| Scrolling | Touch dragging with an 8dp slop. A drag that starts on a button scrolls and cancels the button, like Android's `onInterceptTouchEvent`. Flings decelerate. Touching a moving list stops it without pressing anything. |
| Pull to refresh | Takes a downward drag only when the content is at its top. Otherwise the list scrolls back first, as `SwipeRefreshLayout` does. |
| Back | The back button or gesture closes the topmost dialog, sheet or drawer. When nothing is open it finishes the activity, like the root route of a navigator. |
| Lifecycle | The GPU surface is dropped when the activity goes to the background and recreated when it returns. Application state is kept. |
| Safe area | Views are laid out inside the system bars, display cutouts and the on-screen keyboard, read from the activity's content rectangle. Edge-to-edge (Android 15) is handled. |
| Keyboard | The on-screen keyboard opens while a text field or editor has focus and closes when it loses focus. |
| Dark mode | The system light or dark setting is followed, including a switch while the app runs. |
| Fonts | Roboto and the system Noto families (Devanagari and other scripts) are loaded from `/system/fonts`. |
| Clipboard | Text and HTML through `ClipboardManager`. |
| Rendering | wgpu picks a Vulkan or OpenGL ES adapter. If the GPU path cannot start, the CPU renderer (tiny-skia) takes over. |

## Known gaps compared with Flutter

These are not done yet, roughly in order of how much they matter:

1. **Device verification.** Nothing has run on hardware: frame pacing,
   GPU drivers, the lifecycle and input need testing on real phones.
2. **Screen readers.** TalkBack cannot see TAIM views yet. TAIM builds a
   semantic tree (`taim-accessibility`), but nothing connects it to Android
   accessibility (AccessKit's Android adapter is the likely route).
3. **Text input from the keyboard.** With `NativeActivity` the on-screen
   keyboard sends key events. That covers basic typing but not
   composition, suggestions or swipe typing. The `android-game-activity`
   feature uses AndroidX GameActivity, which has a real input connection,
   but TAIM does not handle its composition events yet.
4. **Multi-touch gestures.** Only the first finger is delivered, so there
   is no pinch-to-zoom or two-finger rotation yet.
5. **Platform services.** There is no plugin or method-channel system
   for Android APIs (camera, sensors, notifications, sharing). Applications
   reach Java through `jni`, as the clipboard does.
6. **Tooling.** There is no hot reload, and no `flutter create`-style
   project generator beyond copying `examples/mobile_shell`.
7. **iOS.** Not started.
