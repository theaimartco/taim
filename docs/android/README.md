# Android development preview

Android consumer workspaces currently require the local `wgpu-hal` EGL precision
patch in `vendor/wgpu-hal`. The project generator adds the root-level Cargo
patch automatically. Hand-written workspaces must copy the `[patch.crates-io]`
entry from the root README; dependency workspace patches are not inherited.
The patch and original third-party licenses/hashes are documented in
`vendor/wgpu-hal/TAIM-PATCH.md`. Existing generated projects must refresh their
Java host snapshot and add this Cargo patch when upgrading.

See the [capability inventory and implementation order](capabilities.md) and
the [local notification API](notifications.md) for the current expansion.
The [camera/media API](media.md) covers selection, delegated capture and URI lifecycle.
The [foreground location API](location.md) covers permission/provider status,
validated fixes and bounded subscriptions with lifecycle/drop cleanup.
The [background-work API](background-work.md) provides durable JobScheduler jobs
and an Activity-independent Rust worker entry point.
The [native-view API](native-views.md) provides retained Android hierarchy slots,
native touch/keyboard input, accessibility and a real TextureView producer.
The [WebView adapter](webview.md) adds explicit inline/HTTPS content with native
input/accessibility. [Foreground sensor snapshots](sensors.md) add bounded
motion/environment reads and typed Rust decoding.

See [accessibility and permission APIs](accessibility-permissions.md) for typed
permission observations, special-access settings and virtual text editing.

The [production hardening audit](production-readiness.md) records verified fixes,
optimized-release validation and the remaining framework-wide release evidence.

Android is an experimental build target, **not a production-supported platform**.
The NativeActivity counter uses the existing persistent views and signals.
Rendering, touch/repaint and suspend/resume pass on the tested API 35 x86_64
4 KiB Vulkan guest and 16 KiB GLES guest. Native editor snapshots, secure input,
system insets/preferences and system-font discovery are implemented. The 16 KiB
Vulkan driver failure remains reproducible; the compatibility path rejects bad
buffer roundtrips and retries GLES. Physical-device runtime remains unverified.
The expanded platform implementation adds accessibility/autofill, back gestures,
opt-in process restoration, native services/plugins, GPU recovery and a standalone
release-project generator. See [platform implementation](platform-20261030.md),
[native-host evidence](native-host-20261029.md) and the
[earlier Vulkan investigation](runtime-acceptance-20261028.md). No Flutter
performance advantage has been established; the paired emulator run verifies the harness.

## Build the counter

Initial candidate configuration: Linux build host, Android API 26 minimum,
SDK/target API 35, NDK r28+, ARM64 physical-device ABI. The helper also builds the
x86_64 ABI; the tested API 35 emulator runtime is accepted only for the smoke
workloads recorded in the runtime evidence. These are build configurations, not
an accepted OS/device support matrix.

Install a JDK, Python 3, Android SDK platform 35, build-tools 35.0.0, platform-tools,
and an NDK r28 or newer. Use the repository's Rust toolchain/MSRV requirements.

```bash
rustup target add aarch64-linux-android
export ANDROID_HOME=/path/to/Android/Sdk
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/28.2.13676358"
python3 scripts/android/counter.py check
python3 scripts/android/counter.py build
python3 scripts/android/counter.py run --serial YOUR_DEVICE_SERIAL
python3 scripts/android/counter.py logs --serial YOUR_DEVICE_SERIAL
```

The build helper sets the API 26 NDK compiler/linker, checks required exported
entry points, checks native dependencies and ELF load alignment, packages an
uncompressed native library, aligns the APK, signs it with a local development
key, and verifies its signature and ZIP alignment. It uses SDK tools directly;
Gradle and cargo-apk are not required for this first debug APK.

Artifacts are under Cargo's target directory, normally:

```text
target/android/arm64-v8a/debug/taim-counter.apk
target/android/taim-development.keystore
```

`--release` optimizes the Rust library but **still produces a debuggable,
development-signed APK**. It is not a store-release command. For non-debuggable release APKs and AABs, use the
[standalone API 36 project generator](platform-20261030.md#standalone-release-project).
Actual production signing credentials, store review and distribution remain app-owner steps.
The helper supports `install` without launch and `--abi x86_64` after installing
`rustup target add x86_64-linux-android`. Set `TAIM_ANDROID_BUILD_TOOLS` to select
another compatible installed SDK build-tools version.

NDK/ELF and ZIP alignment checks follow the
[Android 16 KiB page-size guidance](https://developer.android.com/guide/practices/page-sizes).
They do not replace executing on a 16 KiB device.

## Use the public API

An external Android application needs a `cdylib` and a manifest pointing to its
library. For a local checkout dependency:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
taim = { path = "/path/to/taim", default-features = false, features = ["wgpu-android"] }
```

```rust,ignore
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(activity: taim::android::AndroidApp) {
    use taim::prelude::*;
    let result = taim::app("My Android app", |cx| {
        let count = cx.signal(0);
        column(views![
            text(move || format!("Count: {}", count.get())),
            button("Increment").on_press(move || count.update(|n| *n += 1)),
        ])
    }).run_android(activity);
    if let Err(error) = result {
        eprintln!("Taim failed: {error}");
    }
}
```

Use Taim's re-exported `AndroidApp`; do not independently select another version
of android-activity. `android_main` has Rust ABI. Its unmangled entry symbol is
required by the activity glue; the example isolates that export to Android.
Calling ordinary `.run()`/`launch()` on Android returns an explicit missing-activity
error. No global activity handle is installed.

`wgpu-android` enables Vulkan plus GLES compatibility. Vulkan initialization
checks mapped, queue and staging-buffer roundtrips before creating the renderer.
A failed check retries GLES. Explicit `wgpu-vulkan` remains Vulkan-only and
returns an initialization error if its check fails. GLES uses a retained SDR
intermediate when the surface cannot be copied or encode sRGB itself.

Include [`TaimActivity.java`](../../winit/android/com/theaimartco/taim/TaimActivity.java)
and its sibling Java host files in your Android application, use `com.theaimartco.taim.TaimActivity` as its
manifest activity, and set `android:hasCode="true"`. Keep
`android.app.lib_name` equal to your cdylib name without `lib`/`.so`. The helper
compiles this host with javac/D8 and checks its JNI exports. Plain
`android.app.NativeActivity` supplies lifecycle glue but not this editor/insets
contract. Disable Taim's desktop defaults on Android.

System fonts are discovered from Android's font directories with explicit
script and emoji fallback. On API 31+, missing system color glyphs can use
Android's native glyph rasterizer, with results retained in the bounded GPU
atlas. Older releases use the existing Swash outline/bitmap path. The demo
includes Hindi, Arabic, color emoji, normal/password fields and a multiline editor.

Normal editors exchange UTF-16 platform selections through normalized UTF-8
snapshots. Composition stays uncommitted until the connection finishes it;
focus generations reject stale edits. Password connections receive no existing
secret, selection, or length, and use incremental edits with learning disabled.
Normal native contexts are bounded to 262,144 UTF-16 units and 1 MiB UTF-8;
oversized contexts disable native input instead of truncating application text.
Password surrounding-text queries remain empty; arbitrary IME-driven secret
selection is intentionally unsupported. Explicit autofill hints and native clipboard/services
are implemented; third-party IMEs and password managers still need device acceptance.

The safe area unions bars, display cutouts and the IME, without adding overlapping
insets. Layout, paint and pointer coordinates use the same logical origin;
focused carets pan into the remaining area. Bars follow the resolved app theme,
and system dark mode/reduced-motion changes are observed. Explicit application
preferences retain precedence.

The example sends framework INFO messages and count changes to stderr, which
android-activity forwards to logcat (`RustStdoutStderr`). An application may
install its own logger instead.

## Lifecycle contract

The activity handle is supplied explicitly when constructing winit's event loop.
Windows and graphics initialize from `resumed`. Android `suspended` drops the
presentation target synchronously before returning, suspends the frame scheduler,
and sends focus loss to cancel interactions/composition. Views, signals and the
renderer remain in memory. Resume recreates the presentation target and schedules
a new layout/frame without calling the application's build closure again.

This follows [winit 0.30's lifecycle contract](https://docs.rs/winit/0.30.13/winit/application/trait.ApplicationHandler.html).
Repeated callbacks are tolerated. Zero-size targets are not drawn. This concerns
**surface recreation within a living activity**. For activity/process recreation, register versioned state with `Scope::restorable`;
the native host checkpoints that opt-in state into the saved-task Bundle. Ordinary
signals and passwords are not persisted automatically. See the expanded platform record.

## Physical-device acceptance (pending)

Record device model, OS/API, GPU/driver, refresh rate, page size, Rust/NDK versions,
commit or working-tree provenance and artifact SHA-256 with each run.

1. Install and launch; confirm legible text and a rendered increment button.
2. Tap increment repeatedly; verify the displayed count and `counter=N` logs.
3. Press Home and reopen repeatedly; confirm count survives actual surface loss,
   release/recreation messages appear, and there is no crash or blank screen.
4. Interrupt an active press with backgrounding; returning/releasing must not
   accidentally activate the button.
5. Rotate and resize; check density, clipping and layout. Verify opted-in state
   after activity recreation, and separately verify process death with a saved task.
6. Check idle and background CPU/frame activity with traces; no performance claim
   can be inferred from scheduler unit tests.
7. Run on both 4 KiB and 16 KiB page-size environments and multiple GPU vendors.
8. Record back behavior, system insets, IME and TalkBack failures as blockers.

Collect screenshots, logcat and crashes before making support claims. Use
`llvm-addr2line` from the selected NDK against the matching unstripped debug `.so`
for native crash offsets. Retain the matching library when saving evidence.

See [implementation plan and gap matrix](plan.md) for the remaining work.

## Automated runtime smoke

The counter enables the opt-in `measurement` feature. On Android it logs
`TAIM_ANDROID_PRESENT` after the first successful compositor presentation of
an initial/recreated presentation target. The marker is not a display scan-out
measurement and must not be used as an input-latency benchmark.

Build the APK for your test device ABI, install Python Pillow, and select a
specific test-device serial. The runner installs and force-stops only the
reference counter package. Supply the increment-button tap position and a crop
containing only the count text, both in physical screenshot pixels:

```bash
python3 scripts/android/runtime-smoke.py \
  --serial YOUR_DEVICE_SERIAL \
  --apk target/android/arm64-v8a/debug/taim-counter.apk \
  --tap BUTTON_CENTER_X BUTTON_CENTER_Y \
  --counter-region COUNT_X COUNT_Y COUNT_WIDTH COUNT_HEIGHT \
  --cycles 3 --process-recreation --output target/android/runtime-smoke/device-run-01
```

Use a new empty output directory for each run. Coordinates are intentionally explicit so this rendering/touch test stays
independent of the accessibility bridge. The separate platform probe exercises
the real virtual accessibility tree and its native actions. Inspect an initial screenshot to select the
button and count crop; exclude clocks, cursors and animations from the crop.

The runner requires a successful presentation, stable nonblank count pixels,
a real touch callback and changed count pixels. It waits through Android window
animations rather than disabling them. For each Home/reopen cycle it requires native target release,
recreation and presentation, unchanged process identity, a single app entry,
retained count pixels, and a subsequent successful increment. It writes
screenshots, scoped logcat, device/page-size information, Vulkan capabilities,
APK/artifact SHA-256 values, source provenance and a JSON result. Failures produce
a failing exit code and a report describing the failing stage.

A launched activity or an installed APK alone never passes this test. The raw
`am start -W` output is activity metadata, not a first-useful-frame benchmark.
Physical-device IME, TalkBack, GPU recovery and energy acceptance remain separate.


## Independent Vulkan buffer probe

`examples/android_gpu_probe` reproduces buffer-copy behavior without linking Taim.
Build it using `counter.py build --app gpu-probe --abi x86_64`, then use
`buffer-probe.py` to collect a pass/fail JSON report. See the
[runtime acceptance record](runtime-acceptance-20261028.md#reproduce) for commands,
artifact hashes and the observed 4 KiB/16 KiB difference. Passing package alignment
checks alone does not establish runtime buffer integrity.


## Native editor protocol probe

After building/installing the counter, run the separate **test-device-only** IME
against physical-pixel coordinates selected from a screenshot:

```bash
python3 scripts/android/ime-probe.py run --serial YOUR_TEST_DEVICE \
  --tap NORMAL_X NORMAL_Y --secure-tap PASSWORD_X PASSWORD_Y \
  --multiline-tap EDITOR_X EDITOR_Y --output target/android/runtime-smoke/ime-run-01
```

This compiles/installs a separate test IME, temporarily selects it, resets only
the counter, exercises real InputConnection calls, saves screenshots/reports,
and restores the previous IME in `finally`. The test IME is excluded from the
counter APK. Do not enable it for personal input or distribute it with an app.
The stock-IME and physical-device matrix still require separate acceptance.
