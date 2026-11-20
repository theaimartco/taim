# macOS apps and Mac App Store packaging

Taim builds native macOS applications using AppKit windows and Metal through
wgpu. The packager in `scripts/macos/package.py` builds a real `.app`, enables
App Sandbox, compiles the app icon, validates the Mach-O deployment target and
signatures, and optionally creates a signed Mac App Store `.pkg`.

Local packaging and runtime results are recorded separately from Apple
distribution acceptance. An ad-hoc signed local app is **not** an uploadable
store release. Store signing needs the app owner's Apple Developer account,
matching certificates, explicit App ID and provisioning profile. Packaging
does not upload or publish anything, and does not grant framework-wide
production clearance.

## Requirements

- macOS with Xcode selected by `xcode-select` and its license/setup completed.
- The repository's pinned Rust toolchain, Python 3.11+, and the Rust targets
  for the architectures being packaged.
- For store distribution: Apple Developer Program membership, a registered
  explicit Mac App ID and App Store Connect app record, app distribution and
  installer distribution certificates with their private keys in Keychain,
  and the explicit Mac App Store provisioning profile.

## Build a local app

From this checkout:

```sh
python3 scripts/macos/package.py build --config scripts/macos/counter.toml \
  --package counter --binary counter --output target/macos/counter-local
open "target/macos/counter-local/Taim Counter.app"
python3 scripts/macos/package.py validate "target/macos/counter-local/Taim Counter.app"
```

The default is an optimized release build for the host architecture, signed
ad-hoc with App Sandbox enabled. `--debug` is for faster local smoke builds.
Outputs go to a **new directory**; existing artifacts are never overwritten.
Each output includes `report.json` with architecture, signature/sandbox checks,
executable and Cargo.lock hashes, toolchain/SDK versions, source commit and
working-tree status. Bundle selection uses Cargo's actual artifact messages,
including custom target directories; a failed build cannot reuse a stale binary.

`taim-new` includes `macos/app.toml`, `package.py`, an editable starter PNG icon
and `PrivacyInfo.xcprivacy` in generated projects. See the generated README for
commands specific to that project. Replace starter branding before shipping.

## Application configuration

Copy `scripts/macos/counter.toml` for an existing application. Paths for the
icon and privacy manifest are relative to the configuration file. Configure
display name, registered bundle ID, numeric marketing/build versions,
minimum macOS version, App Store category and copyright. The packager sets
`MACOSX_DEPLOYMENT_TARGET` when compiling and rejects binaries requiring a
newer OS than the bundle advertises. Actual support on an older OS still
needs runtime testing there.

Icons may be 1024 × 1024 PNGs or ICNS files containing a 1024px representation.
The PNG is converted into all standard macOS icon sizes using Apple's tools.

`[entitlements]` adds the app's actual sandbox capabilities. For example:

```toml
[entitlements]
"com.apple.security.network.client" = true
"com.apple.security.files.user-selected.read-write" = true

[usage_descriptions]
NSCameraUsageDescription = "Take the profile photo you choose to capture."
```

Keep capabilities limited to those the app implements. A usage description
does not grant a capability or runtime permission. A user-file entitlement
does not grant arbitrary paths: use an OS file picker/security-scoped grant.
The packager does not yet bundle/sign extra dynamic libraries, app extensions
or nested helper applications; it rejects non-system dylib dependencies.

App Sandbox cannot be disabled. Debugger access, inherited helper sandboxing,
JIT, unsigned executable memory and disabled library validation are rejected
by this workflow. Signing identity entitlements are derived from the owner's
team/profile rather than accepting a configurable application identifier.

## Storage and links inside the sandbox

On macOS, `taim::platform::data_dir()` obtains Application Support using
Foundation, so sandboxed apps write into their own container. Bundled apps
use the bundle ID as the settings namespace. A bare executable retains its
executable-name namespace. This avoids depending on the `HOME` environment
variable, which may refer to the real home rather than the container.
Moving a previously unbundled app into a bundle therefore needs an explicit
data-migration decision; the framework does not read data outside its container.

`taim::platform::open_url` uses `NSWorkspace`, rather than launching the
`/usr/bin/open` helper from inside the sandbox.

## Platform services

`taim::apple` offers the same functions on macOS as on iOS
(`docs/platform/ios.md`), backed by AppKit:

| Service | macOS behaviour | Needs |
|---|---|---|
| Share (`platform::share_text`) | `NSSharingServicePicker` over the key window | — |
| `haptic` | Force Touch trackpad, only while a finger rests on it | — |
| `biometric_status`, `authenticate` | Touch ID, or the account password | — |
| `keychain_set`/`get`/`delete` | data-protection keychain when signed with a keychain access group, else the login keychain | prompting `KeychainAccess` kinds need the data-protection keychain |
| Notifications | `UNUserNotificationCenter`, with open/dismiss/foreground events | an app bundle; `Denied` until the user allows them |
| Location | Core Location; `Disabled` while Location Services are off in System Settings | `NSLocationUsageDescription`, `NSLocationWhenInUseUsageDescription`, `com.apple.security.personal-information.location` |
| `pick_media` | the open panel, filtered to images or movies; files are copied to `~/Library/Caches/<bundle id>/taim-media` | `com.apple.security.files.user-selected.read-only` |
| Camera | `camera_permission`, `request_camera_permission`; `capture_media` is `Unsupported` | `NSCameraUsageDescription`, `com.apple.security.device.camera` |

Started with `cargo run`, outside a bundle, notifications and location report
`ServiceError::Unavailable`. Add usage descriptions under
`[usage_descriptions]` and entitlements under `[entitlements]` in the app
configuration; `scripts/macos/apple-services-probe.toml` is a complete example:

```sh
python3 scripts/macos/package.py build --config scripts/macos/apple-services-probe.toml \
  --package apple_services_probe --binary apple_services_probe --output target/macos/probe
"target/macos/probe/Taim Apple Services Probe.app/Contents/MacOS/apple_services_probe" --share
```

It prints one `TAIM_APPLE_PROBE name=result` line per check. On an ad-hoc
signed build (2026-11-20, macOS 26, Apple silicon) the keychain round trip,
Touch ID status, haptics, share picker and input validation passed;
notifications reported `NotDetermined`/`Denied` before permission; location
reported `Disabled` because Location Services were off on that Mac. The
permission prompts, Touch ID prompt and open panel need a person at the Mac
and have not been recorded yet (`--notify`, `--ask-location`, `--auth`,
`--pick`).

## Smaller executables

`--size` builds with the `release-small` Cargo profile: about 40% smaller
than a thin-LTO release (counter: 3.58 MiB instead of 6.05 MiB; 5.26 MiB with
the default fat-LTO `release`), with software rendering about 25% slower.
GPU rendering is unaffected. The profile must exist in the app's workspace;
`taim-new` projects have it.

## Produce a Mac App Store installer

Install both distribution identities in Keychain. Do not substitute Developer
ID certificates; those are for distribution outside the store. Set these
variables in your shell, without committing credentials:

```sh
export TAIM_MACOS_TEAM_ID=YOURTEAMID
export TAIM_MACOS_APP_IDENTITY='Apple Distribution: Your Company (YOURTEAMID)'
export TAIM_MACOS_INSTALLER_IDENTITY='3rd Party Mac Developer Installer: Your Company (YOURTEAMID)'
export TAIM_MACOS_PROFILE=/absolute/path/YourApp.provisionprofile
rustup target add aarch64-apple-darwin x86_64-apple-darwin
python3 scripts/macos/package.py build --config /path/to/app/macos/app.toml \
  --manifest /path/to/app/Cargo.toml --package your_app --binary your_app \
  --arch universal --mode app-store --output /path/to/new/store-build-1
```

`--arch arm64` is also supported when only Apple Silicon is intended. Intel
support requires an x86_64 build and runtime acceptance on appropriate hardware;
the universal option combines the two compiled binaries using `lipo`.

Store mode requires an optimized build and a locked Cargo dependency graph.
It checks the profile's expiry, explicit bundle/team identifier, macOS platform,
distribution policy and certificate. It embeds the profile, signs the app with
the matching distribution identity, checks the signature's actual entitlements
and embedded certificate, and signs an installer using the separate installer
identity. It also verifies the installer signature. No production identity is
silently replaced with an ad-hoc or development signature.

Open Apple's **Transporter** application and upload the resulting signed `.pkg`.
The owner must complete app/privacy metadata, screenshots, export compliance
and the application's testing, then submit it for Apple review. Local validation
is preparation for Apple's server validation, not a substitute for it. Increment
`app.build` for each upload.

Useful independent local check:

```sh
python3 scripts/macos/package.py validate "/path/to/Your App.app" --app-store
pkgutil --check-signature "/path/to/Your App.pkg"
```

## Runtime acceptance fixture

Build and run the dedicated optimized storage/input fixture:

```sh
python3 scripts/macos/package.py build --config scripts/macos/store-probe.toml \
  --package macos_store_probe --binary macos_store_probe \
  --output target/macos/store-probe
python3 scripts/macos/smoke.py --bundle "target/macos/store-probe/Taim Store Probe.app" \
  --output target/macos/store-probe-smoke --keep-open
```

The runner verifies the signed sandbox entitlement, launches only this fixture,
requires a fresh storage report from the real private container and a successful
wgpu presentation, then restarts it and requires persisted state. It stops only
processes it starts. `--keep-open` leaves the final run for visual/keyboard checks.
Frame presentation is not proof of correct pixels; inspect the window and test
typing, composition and buttons. Test the real application's flows, clipboard,
VoiceOver, resize, sleep/wake and supported OS versions separately.

Packaging regression checks:

```sh
python3 -m unittest discover -s scripts/qa -p test_macos_package.py
cargo test -p taim-new --locked
```

The starter privacy manifest declares no tracking/collection and elapsed-time
measurement for UI scheduling. Review all app/dependency API use and update it
and App Store Connect privacy answers before distribution; adding networking,
analytics or other SDKs changes the application's declarations.

Apple references:

- [Preparing apps for distribution](https://help.apple.com/xcode/mac/current/en.lproj/dev91fe7130a.html)
- [Packaging third-party-built Mac software](https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution)
- [App Store provisioning profiles](https://developer.apple.com/help/account/provisioning-profiles/create-an-app-store-provisioning-profile)
- [Upload builds to App Store Connect](https://developer.apple.com/help/app-store-connect/manage-builds/upload-builds/)
- [Required-reason API declarations](https://developer.apple.com/documentation/bundleresources/app-privacy-configuration/nsprivacyaccessedapitypes/nsprivacyaccessedapitype)
