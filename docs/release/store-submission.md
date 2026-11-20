# Store submission

What it takes to put a Taim application in each store: the command that
builds the upload, what the owner supplies, and what has and has not been
checked. Every packager writes into a new directory, reads credentials only
from the environment, and never uploads. A store accepting an upload is not
evidence that the application works on users' devices; that is
[production-gates.md](production-gates.md).

| Store | Upload | Packager | Run on real hardware |
|---|---|---|---|
| Google Play | `.aab` | Gradle `bundleRelease` | Emulator only |
| App Store (iOS) | `.ipa` | `scripts/ios/package.py` | Simulator only |
| Mac App Store | `.pkg` | `scripts/macos/package.py` | One arm64 Mac |
| Microsoft Store | `.msix` | `scripts/windows/package.py` | Never |
| Snap Store (Ubuntu) | `.snap` | `scripts/linux/package.py` | Never |

## Google Play

```sh
cd examples/mobile_shell/android   # or your taim-new project's android/
./gradlew bundleRelease
```

You supply: an upload key (Play App Signing holds the app signing key), the
release `signingConfig` in place of the debug one, the application ID, the
Play Console listing, the Data safety form and content rating. New projects
target SDK 36. Before submitting: `scripts/qa/android-device-check.sh` on
physical phones, or `scripts/qa/firebase-test-lab.sh` on three physical
models plus the by-hand checks. See [android.md](../platform/android.md).

## App Store (iOS)

```sh
python3 scripts/ios/package.py --package counter --binary counter --name "Taim Counter" \
  --bundle-id com.example.counter --target aarch64-apple-ios --mode app-store --release \
  --team TEAMID --identity "Apple Distribution: ..." --profile app.mobileprovision \
  --icon icon.png --privacy-manifest PrivacyInfo.xcprivacy --build-number 1 --output target/ios/counter
```

You supply: an Apple Developer membership, an explicit App ID, an
Apple Distribution certificate and App Store provisioning profile, the app
icon, usage descriptions for every service used, the privacy manifest, and
the App Store Connect record. Upload the `.ipa` with Transporter; raise the
build number each time. Not yet done by anyone: a physical iPhone or iPad,
VoiceOver, and a TestFlight upload. See [ios.md](../platform/ios.md).

## Mac App Store

```sh
python3 scripts/macos/package.py build --config scripts/macos/counter.toml \
  --package counter --binary counter --output target/mac/counter --mode app-store
```

You supply: `TAIM_MACOS_APP_IDENTITY`, `TAIM_MACOS_INSTALLER_IDENTITY`,
`TAIM_MACOS_TEAM_ID` and `TAIM_MACOS_PROFILE` (a Mac App Store profile).
The app is sandboxed; add entitlements only for what it uses. Upload the
`.pkg` with Transporter. Intel and universal builds have not been run. See
[macos.md](../platform/macos.md).

## Microsoft Store

```sh
# On Windows with the Windows SDK (makeappx) and Pillow:
python scripts\windows\package.py pack --config scripts\windows\counter.toml ^
  --package counter --binary counter --output target\msix\counter --arch x64
```

You supply: a Partner Center developer account and a reserved app name.
Copy `identity`, `publisher` and `publisher_display_name` from Product
identity into the config; the fourth version part stays `0`. Upload the
unsigned `.msix`: the Store signs it. `--sign` (with
`TAIM_WINDOWS_CERTIFICATE` and `TAIM_WINDOWS_CERTIFICATE_PASSWORD`) is for
sideloading outside the Store.

The package is a full-trust desktop app (`runFullTrust`, which the Store
allows for Win32 apps) with a statically linked C runtime, for Windows 10
1809 and later. The packager refuses an executable that would open a console
window; release builds of `taim-new` projects and the examples set the GUI
subsystem. `layout` writes the package folder without packing, on any OS,
given `--executable`.

Not yet verified: running any Taim app on Windows, `makeappx` and Windows
App Certification Kit results, keyboard/IME and Narrator.

## Snap Store (Ubuntu)

```sh
python3 scripts/linux/package.py snap --config scripts/linux/counter.toml \
  --package counter --binary counter --output target/snap/counter \
  --source https://github.com/OWNER/REPO.git
cd target/snap/counter && snapcraft pack        # on Ubuntu, with LXD
sudo snap install --dangerous ./*.snap && taim-counter
snapcraft register taim-counter && snapcraft upload --release=edge ./*.snap
```

You supply: a Snapcraft account and a registered snap name. The project is
strictly confined on core24, builds from source with the Rust plugin (the
toolchain from `rust-toolchain.toml`), stages the libraries winit opens at
run time and takes GPU drivers from the `mesa-2404` content snap. Ask for
`network`, `home` or `audio-playback` in `app.plugs` only if the app uses
them.

Not yet verified: `snapcraft pack`, the confined app running on Ubuntu under
Wayland and X11, and store review. Flathub needs an offline build with
vendored crates (`flatpak-cargo-generator`) and is not packaged yet.
