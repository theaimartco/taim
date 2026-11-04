# Android runtime acceptance — 2026-10-28

The first functional Android runtime milestone passes on the tested **Android 15,
x86_64, 4 KiB AOSP emulator with Vulkan/SwiftShader**. This is emulator evidence,
not physical-device certification or an Android production release.

## Results

| Configuration | Counter runtime smoke | Independent buffer probe |
|---|---|---|
| API 35 AOSP, x86_64, 4 KiB pages, Vulkan/SwiftShader | PASS: visible UI, four touch increments/repaints, three Home/reopen cycles | PASS: 16/16 byte-copy checks |
| API 35 Google APIs 16 KiB image, x86_64, Vulkan/SwiftShader | FAIL: stable count pixels not produced | FAIL: 0/16 byte-copy checks |

The counter smoke requires actual count-region pixels, not just an activity launch
or a successful graphics API return. Every Home/reopen cycle released/recreated
the native presentation target, successfully presented again, retained the same
process and count pixels, and accepted a subsequent increment. Final count: 4;
process ID during the passing run: 2472.

An early version of the runner captured a resumed window during Android's launch
animation and rejected the temporarily scaled pixels. The final runner waits for
stable, nonblank pixels and eventual exact retained count pixels. System animations
remain enabled. This is a test synchronization fix, not a framework layout fix.

## 16 KiB compatibility blocker

The independent `android_gpu_probe` app has **no Taim dependency**. It checks:

- One mapped-at-creation initialization followed by a copy/readback.
- Seven subsequent queue writes followed by copy/readback.
- Eight staging-belt writes followed by copy/readback.

All 16 cases pass on the tested 4 KiB guest and fail on the tested 16 KiB guest,
using the same probe APK. Mapping completion callbacks reported success even when
returned bytes were wrong. Temporary uniform readback instrumentation in Taim also
observed incorrect bytes; that diagnostic instrumentation has been removed.

This isolates a failing path below Taim's view tree, renderer scene preparation
and window handling. It does **not** identify whether the cause is wgpu, its
allocator, the guest Vulkan driver, the emulator's memory transport, or another
component. The images have different system builds/vendor stacks: these results
are not proof that page size alone causes the failure, and do not establish that
all physical 16 KiB devices fail.

Changing the emulator from SwiftShader to host rendering did not make the counter
smoke pass in the tested setup. No production allocator changes, extra per-frame
GPU waits or disabled correctness checks were introduced to hide the failure.
The 16 KiB runtime gate remains **FAIL / RELEASE BLOCKER**, despite passing ELF
and APK alignment checks.

## Artifacts and provenance

All APKs are debuggable, development-signed artifacts of an experimental, dirty
working tree, not immutable release candidates. Rust 1.95.0, NDK 28.2.13676358,
SDK/build-tools 35.0.0 and emulator 36.5.11 were used on Linux with KVM.
The renderer uses wgpu 29.0.4. No timing output here is a benchmark.

| Artifact | SHA-256 |
|---|---|
| x86_64 counter APK | `6f1c71476ea5df9d9372e93112c232d9eff555fe2455c8255ce75be5b3417611` |
| x86_64 independent GPU probe APK (both guests) | `40b07ec038a81ae46417df6eb174ac109918a8624bf27e4440753e34b79a3188` |

Local ignored evidence directories:

- `target/android/runtime-smoke/android35-4k-02/`: passing report, screenshots,
  scoped logcat and Vulkan capability JSON, with artifact hashes.
- `target/android/runtime-smoke/buffer-4k-01/`: 16 passing byte-copy checks.
- `target/android/runtime-smoke/buffer-16k-final/`: 16 failing byte-copy checks.
- `target/android/runtime-smoke/android35-16k-final/`: failing stable-pixel gate.

Guest fingerprints:

```text
4 KiB: Android/sdk_phone64_x86_64/emu64x:15/AE3A.240806.019/12368160:userdebug/test-keys
16 KiB: google/sdk_gphone16k_x86_64/emu64xa16k:15/AE3A.240806.043/12960925:userdebug/dev-keys
```

## Reproduce

Build `counter.py build --abi x86_64` and run the runtime smoke described in
[Android setup](README.md#automated-runtime-smoke). The tested Pixel 6 emulator
configuration used a 1080×2400 screenshot, increment tap `(220, 396)` and count
crop `(70, 210, 320, 100)`. Verify coordinates on another device before using them.

Build the independent probe with:

```bash
python3 scripts/android/counter.py build --app gpu-probe --abi x86_64
python3 scripts/android/buffer-probe.py \
  --serial YOUR_DEVICE_SERIAL \
  --apk target/android/x86_64/debug/gpu-probe/taim-gpu-probe.apk \
  --output target/android/runtime-smoke/probe-new-run
```

The probe runner installs/stops only `com.theaimartco.taim.gpuprobe`, reads logcat
from the guest-clock cutoff without clearing global logs, verifies all 16 checks
and the completion marker, and exits unsuccessfully on failure. It records device
metadata, APK/log hashes and source provenance. The probe has no UI; it exits when
its checks finish.

For an existing configured emulator, the tested graphics launch options were:

```bash
emulator -avd YOUR_AVD -no-window -no-audio -no-snapshot -no-boot-anim \
  -gpu swiftshader -feature Vulkan -memory 2048 -cores 2
```

Explicitly enable Vulkan and verify `adb shell cmd gpu vkjson` reports a device;
`getconf PAGE_SIZE` records the guest page size. The initial API 37 attempt did
not explicitly enable Vulkan and reported no adapters; it is not an API 37
compatibility result. See the official [emulator command-line reference](https://developer.android.com/studio/run/emulator-commandline)
and [graphics configuration](https://developer.android.com/studio/run/emulator-acceleration).

## Remaining gates

ARM64 is build/link/package checked; physical runtime is not accepted. System
bars/insets, system fonts, full IME editing, TalkBack, back navigation, persistent
activity/process restoration, GPU/device recovery, release app bundles and
same-device Flutter measurements remain pending. Three surface cycles do not
certify resource use over long sessions or GPU loss recovery.
