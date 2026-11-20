# iOS applications, services and packaging

TAIM's native iOS shell uses winit/UIKit and the Metal renderer. The root window
uses the actual device screen bounds; desktop window dimensions are ignored on
iOS. A baseline Fira Sans font is embedded automatically because fontdb's
desktop font discovery does not expose iOS system fonts. Applications needing
additional language coverage must supply and test suitable fonts.

## Build a Simulator application

Use macOS, Xcode with an installed iOS Simulator runtime, the pinned Rust
toolchain and `aarch64-apple-ios-sim` (or `x86_64-apple-ios` on an Intel host).

```sh
rustup target add aarch64-apple-ios-sim
python3 scripts/ios/package.py --package counter --binary counter \
  --name "Taim Counter" --bundle-id com.example.taim.counter \
  --features taim/measurement --output target/ios/counter
```

The tool requires a locked Cargo graph and verifies package ownership of the
executable, architecture, iOS Simulator Mach-O platform, deployment minimum and
system-only dylibs. A failed build cannot package an older executable. It
creates an ad-hoc signed `.app` and a report containing source/lockfile and
executable hashes. It writes a new directory and never replaces an existing
product. `--release` selects an optimized build; `--privacy-manifest` includes
the application's reviewed manifest.

Install and launch on an explicitly chosen Simulator:

```sh
xcrun simctl list devices available
xcrun simctl install DEVICE_ID "target/ios/counter/Taim Counter.app"
xcrun simctl launch DEVICE_ID com.example.taim.counter
xcrun simctl io DEVICE_ID screenshot target/ios/counter.png
```

The Simulator bundle is deliberately identified as `iPhoneSimulator` and cannot
be substituted for an iPhone/device/App Store artifact. Device provisioning,
production signing, distribution metadata/privacy answers, app icons, physical
device flows and Apple validation remain separate work. The tool performs no
upload and does not create or alter Apple account/profile records.

## Platform services (`taim::apple`)

Every service is a plain function; asynchronous ones may be awaited from any
TAIM task (`spawn`/`cx.spawn`). UIKit work is posted to the main thread for
you. Never block the main thread waiting for one: its answer needs that thread.

| Service | API | Info.plist key (`--usage`) |
|---|---|---|
| Share sheet | `taim::platform::share_text` (`UIActivityViewController`, anchored on iPad) | — |
| Haptics | `ios::haptic(Haptic::Success)` — impact, selection and notification styles | — |
| Face ID / Touch ID | `ios::biometric_status(policy)`, `ios::authenticate(reason, policy).await`; dropping the future dismisses the prompt | `NSFaceIDUsageDescription` |
| Keychain | `ios::keychain_set/get/delete`; `KeychainAccess::{AfterFirstUnlock, WhenUnlocked, UserPresence, BiometryCurrentSet}`, always this-device-only | — |
| Local notifications | `notification_permission`, `request_notification_permission`, `schedule_notification(LocalNotification)`, `cancel_notifications`, `cancel_all_notifications`, `set_badge`, `notification_events()` (opened, dismissed, presented; the launch tap is kept for the first subscriber) | — |
| Location | `location_permission`, `request_location_permission` (when in use), `current_location(LocationOptions)` with cache age and timeout, `watch_location` (a `Stream`; dropping it stops updates) | `NSLocationWhenInUseUsageDescription` |
| Photo picker | `pick_media(MediaPicker { kind, limit })` (`PHPickerViewController`, no permission needed); an empty list means cancelled | — |
| Camera | `camera_permission`, `request_camera_permission`, `capture_media(CaptureKind::Photo/Video)` | `NSCameraUsageDescription` (video also `NSMicrophoneUsageDescription`) |

Picked and captured files are copied to `Library/Caches/taim-media` and
belong to the app. Keychain calls block (a prompting item waits for Face ID),
so call them from a task. iOS terminates an app that touches a protected
service without its usage description.

The shell also follows the system light/dark appearance and Reduce Motion
(read at launch, on resize and whenever the app comes forward, because winit
reports neither on iOS), and lays the root view out inside the safe area,
clear of the status bar, Dynamic Island and home indicator.

## Build for a real iPhone and the App Store

Device builds use the owner's Apple Developer certificate and provisioning
profile, never stored by the tool:

```sh
rustup target add aarch64-apple-ios
export TAIM_IOS_TEAM_ID=ABCDE12345
export TAIM_IOS_IDENTITY='Apple Development: Your Name (ABCDE12345)'
export TAIM_IOS_PROFILE=~/Downloads/YourApp_Development.mobileprovision
python3 scripts/ios/package.py --target aarch64-apple-ios --mode development \
  --package counter --binary counter --name "Taim Counter" \
  --bundle-id com.example.taim.counter --icon path/to/AppIcon-1024.png \
  --usage "NSCameraUsageDescription=Scan receipts" --release \
  --output target/ios/counter-device
xcrun devicectl device install app --device DEVICE_ID "target/ios/counter-device/Taim Counter.app"
```

`--mode` selects the profile the tool accepts: `development` (an iOS App
Development profile listing the device), `ad-hoc` (an Ad Hoc distribution
profile) or `app-store` (an App Store profile with an explicit App ID, a
release build, an Apple Distribution identity and an icon). The tool checks
the profile's platform, team, App ID, expiry and kind, embeds it, derives
entitlements from it (app identifier, team, the app's own keychain group,
`get-task-allow` only for development), signs, verifies and writes both the
`.app` and an `.ipa`. Raise `--build-number` for every upload. Upload the
`.ipa` yourself with Transporter or `xcrun altool`; this tool never uploads.

Simulator builds link the app's entitlements into the executable's
`__TEXT,__entitlements` section, as Xcode does, so the keychain works there.

Add `--size` (with `--release`) to build with the `release-small` Cargo
profile: the counter's device executable is 4.00 MiB instead of 5.77 MiB
(2.07 MiB instead of 2.65 MiB compressed), with software rendering about 25%
slower; Metal rendering is unaffected.

## Verified scope and remaining gaps

On 2026-11-20 the `apple_services_probe` fixture ran on an iPhone 17 Simulator
(iOS 26.2), with taps driven through the Simulator window:

| Check | Result |
|---|---|
| Keychain write, overwrite, read, delete; invalid names | Pass |
| Face ID-protected keychain item | Stored with a passcode-only, current-biometry ACL; the Simulator does not enforce the read prompt, so the prompt is unverified |
| Face ID: status before and after enrolment; matching face; non-matching face then Cancel | `NotEnrolled` → `Ok`; `Ok(())`; `Err(Denied)` |
| Notification permission prompt → Allow | `Ok(Granted)` |
| Foreground delivery | Banner with the app icon, `Presented` event with payload |
| Tap while backgrounded; tap that cold-launches the app | `Opened { id, payload }` in both |
| Schedule, badge, removal, invalid input | Pass |
| Location prompt → Allow While Using App / Don't Allow | `WhenInUse` then one-shot and watch fixes / `Denied` for request, one-shot and watch |
| Photo picker: two items selected; cancelled | Both copied to `Library/Caches/taim-media` (`public.png` 6 KB, `public.jpeg` 4.1 MB), in order; `Ok([])` |
| Camera opened; cancelled | `Ok(None)` (the Simulator camera produces no frames, so capture is unverified) |
| Share sheet | Presented with the text |
| Safe area, dark launch, live dark → light switch | Pass |

A release `aarch64-apple-ios` executable linked against every framework the
services use. The cold-launch run found a bug, now fixed: the
notification delegate was installed in winit's `resumed`, after launching
had finished, so the launch tap was lost. It is now installed before UIKit
starts.

Not yet verified: anything on a physical iPhone or iPad; the keychain
biometric read prompt; photo/video capture; dismiss events from Notification
Center; device-signed `.app`/`.ipa` installation and App Store Connect
validation; VoiceOver; complex input methods; the keyboard covering focused
fields (no keyboard avoidance yet). Deep links and push notifications are not
implemented. Consult the release gate ledger rather than treating Simulator
results as device certification.

Packaging regressions:

```sh
python3 -m unittest discover -s scripts/qa -p test_ios_package.py
```
