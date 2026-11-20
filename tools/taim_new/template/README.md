# {{name}}

A [TAIM](https://github.com/theaimartco/taim) application for the desktop
and Android, created by `taim-new`.

## Desktop

```sh
cargo run
```

## macOS app bundle and Mac App Store

Requires macOS, Xcode and Python 3.11+. From this project's root:

```sh
python3 macos/package.py build --config macos/app.toml \
  --package {{crate}} --binary {{crate}} --output target/macos/local
open "target/macos/local/{{name}}.app"
```

This builds an optimized, ad-hoc signed `.app` with App Sandbox enabled.
The local build is for this Mac; it is not signed for distribution.
Use a new output directory for each build. `--debug` is available for local smoke.

For a Mac App Store package, first replace the starter icon, review the privacy
manifest and configure `macos/app.toml` with your registered bundle ID, category,
copyright and increasing build number. Install your Apple distribution app and
installer certificates in Keychain, and download the explicit Mac App Store
provisioning profile for your app. Set:

```sh
export TAIM_MACOS_TEAM_ID=YOURTEAMID
export TAIM_MACOS_APP_IDENTITY='Apple Distribution: Your Company (YOURTEAMID)'
export TAIM_MACOS_INSTALLER_IDENTITY='3rd Party Mac Developer Installer: Your Company (YOURTEAMID)'
export TAIM_MACOS_PROFILE=/absolute/path/YourApp.provisionprofile
rustup target add aarch64-apple-darwin x86_64-apple-darwin
python3 macos/package.py build --config macos/app.toml \
  --package {{crate}} --binary {{crate}} --arch universal \
  --mode app-store --output target/macos/store-build-1
```

App Store mode requires `Cargo.lock` (created by the first local build), validates
certificate/profile/team/bundle identity, verifies signatures and produces a signed
`.pkg` plus `report.json`. Upload that `.pkg` using Apple's Transporter app.
App Store Connect registration, privacy/store metadata and Apple review are
application-owner steps. The packager does not upload or publish anything.

Sandboxed settings use the app's native Application Support container. External
files require a native user-selected file grant and the corresponding entitlement;
adding an entitlement alone does not grant arbitrary filesystem access.
Update `macos/PrivacyInfo.xcprivacy` for your application's actual data and API use.
The starter declares no collection/tracking and elapsed-time measurement for UI timers.
See Taim's `docs/platform/macos.md` for validation and distribution details.

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
