# {{name}}

A [TAIM](https://github.com/theaimartco/taim) application for the desktop
and Android, created by `taim-new`.

## Desktop

```sh
cargo run
```

## Android

You need the Android SDK and NDK (Android Studio installs both), the Rust
targets and `cargo-ndk`:

```sh
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk
export ANDROID_HOME=~/Android/Sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/<version>
```

Then, with a phone connected (USB debugging on) or an emulator running:

```sh
cd android
./gradlew installDebug    # build and install
./gradlew bundleRelease   # an .aab for the Play Store
adb logcat -s taim        # logs and panics
```

Or open `android/` in Android Studio. Before publishing, replace the debug
signing configuration in `android/app/build.gradle.kts`.

The package is `{{package}}`. For text input with composition, suggestions
and swipe typing, switch to GameActivity as described in TAIM's
`docs/platform/android.md`.
