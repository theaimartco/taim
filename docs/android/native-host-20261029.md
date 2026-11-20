# Android native host milestone — 2026-10-29

Status: **implementation complete for the development-preview slice below**.
Android remains experimental and is not production certified. Work was performed
on a dirty working tree rooted at `13121da901b0a711b5e94df20ae30246423a282b`;
existing unrelated changes were preserved.

## Implemented behavior

- `TaimActivity` extends NativeActivity and supplies real native InputConnections,
  bar/cutout/IME insets, resolved bar appearance and system theme/motion changes.
  Java/D8 packaging and JNI-symbol checks are included in the debug APK helper.
- Layout, paint, touch conversion and native caret coordinates share a safe-area
  origin. A focused caret pans above the IME, including landscape. Dismissing and
  retapping the same field can reopen the keyboard. App preferences override
  system defaults. Overlapping navigation/IME insets are combined by maxima.
- Native UTF-16 selections convert to normalized UTF-8 editor snapshots. Composing
  text stays outside application state until committed. Snapshot coalescing
  preserves commit/composition transitions, and focus/connection generations
  reject stale callbacks. Batches and modern TextAttribute overloads are covered;
  newer connection classes are isolated behind API-level checks. Cursor anchors
  use Taim's caret rather than the invisible proxy editor's text layout.
- Password input uses incremental operations. Existing content, selection and
  length are never exported as editor context. Surrounding-text queries are
  empty, learning/extraction are disabled, and composition is masked. UTF-16,
  Unicode-scalar and key/backspace deletion have distinct semantics. A request
  that splits a surrogate pair rounds outward to a valid Unicode boundary.
- System font directories and script/emoji fallback policies are loaded. On
  API 31+, missing system color glyphs use Android's glyph rasterizer, then the
  existing bounded GPU atlas. This renders the tested COLRv1 Noto Color Emoji
  font without bundling a replacement emoji font. Only shaped glyph IDs and
  font metadata cross this raster boundary, not application text. The native
  font cache is bounded to eight faces; glyph dimensions are bounded to 1024².
- `wgpu-android` enables Vulkan plus GLES. Vulkan startup checks mapped, queue and
  staging buffer roundtrips before creating renderer pipelines. Failed
  initialization retries GLES. Explicit Vulkan-only selection remains strict.
  GLES surfaces with no COPY_SRC capability draw through a retained SDR texture;
  the final pass applies sRGB encoding exactly once. Shader array indexing was
  removed from affected fullscreen/quad/glyph vertex paths for GLES compatibility.

The native context limit is 262,144 UTF-16 units and 1 MiB UTF-8. Oversized normal
contexts disable the native connection rather than truncating/replacing app text.
Arbitrary IME-directed password selection is unsupported because secret context
remains redacted. The demo includes normal, password and multiline editors.

## Build and host verification

Linux, Rust 1.95.0, NDK 28.2.13676358, SDK/build-tools 35.0.0, min API 26 and target
API 35. ARM64 and x86_64 debug APKs pass signature, JNI export, native dependency,
ELF LOAD alignment and `zipalign -P 16` checks.

| Artifact | SHA-256 |
|---|---|
| `target/android/x86_64/debug/taim-counter.apk` | `ccbaed2ac4803761e28d2097a82c1fc8072f53f93ba4860bcc60908a59ca9367` |
| `target/android/arm64-v8a/debug/taim-counter.apk` | `bc32f3499d3762a5369f04edfa7209561695e335dfabed32c611e114a67c24ea` |

- Native-view integration: **31 passed**, including uncommitted composition,
  multiline UTF-8 selection, secure context and UTF-16/scalar deletion differences.
- Shell unit tests with measurement: **44 passed**, including inset geometry.
- Core input-method/security tests: **3 passed**.
- Renderer unit tests: **61 passed**. Explicit copyless-surface GPU color test:
  **1 passed**, with final readback bytes checked against the sRGB transfer curve.
- Existing GPU color-output tests: **10 passed**.
- Strict Clippy: facade/native-view target and ARM64 Android counter/dependencies
  passed. The standalone glyph crate has no unit tests; its GLSL vertex path and
  native color fallback were checked through actual Android screenshots.

## Runtime evidence

The same x86_64 APK is used for the 4 KiB and 16 KiB runtime checks. Each lifecycle
run requires stable nonblank count pixels, four touch increments, three real
Home/reopen target release/recreation/presentation cycles, retained count pixels
and unchanged process identity.

The API 35 16 KiB guest is
`google/sdk_gphone16k_x86_64/emu64xa16k:15/AE3A.240806.043/12960925:userdebug/dev-keys`.
Its Vulkan roundtrip is rejected and the tested counter runs on GLES/SwiftShader.
The upstream Vulkan corruption is **guarded, not fixed**. See the
[earlier independent probe investigation](runtime-acceptance-20261028.md).

Local evidence directories under `target/android/runtime-smoke/`:

| Directory | Evidence |
|---|---|
| `native-host-16k-release-check` | PASS: 16 KiB lifecycle/touch/pixel report and scoped logcat |
| `native-ime-16k-release-check` | PASS: 29 real InputConnection protocol operations/assertions and screenshots |
| `native-platform-16k-05` | PASS: 8 preference/inset/rotation checks; settled light/dark screenshots and landscape caret above keyboard |
| `native-host-4k-release-check` | PASS: 4 KiB Vulkan lifecycle/touch/pixel report |
| `native-ime-4k-release-check` | PASS: 29 InputConnection protocol operations/assertions on the 4 KiB guest |

The separate test IME exercises modern commit/composition overloads, batching,
Hindi/Arabic/Japanese/emoji, UTF-16 selection, codepoint deletion, stale remote
connections, redacted password context and multiline edits. The previous IME is
restored in `finally`. Stock Gboard typing, dismissal/reopen and landscape
keyboard geometry were also exercised on the 16 KiB guest. Preference checks
restore the previous night mode, animator-duration setting and rotation settings.
Dark/light acceptance uses settled application pixels, not just a preference log.

The API 35 guest's earlier clipped status icons were reproducible on its launcher
as well; they were not used as a Taim rendering acceptance signal. Settled theme
screenshots show the resolved light/dark icon appearance.

## Reproduce

Build using [the setup guide](README.md), then select an explicit test-device
serial. Coordinates below are for the recorded 1080×2400 / 420 dpi counter.

```bash
python3 scripts/android/runtime-smoke.py \
  --serial YOUR_TEST_SERIAL \
  --apk target/android/x86_64/debug/taim-counter.apk \
  --tap 220 520 --counter-region 75 340 300 90 \
  --output target/android/runtime-smoke/new-runtime-run

python3 scripts/android/ime-probe.py run --serial YOUR_TEST_SERIAL \
  --tap 250 750 --secure-tap 250 980 --multiline-tap 250 1150 \
  --output target/android/runtime-smoke/new-ime-run

cargo test --test native_views --locked
cargo test -p taim_winit --lib --features measurement --locked
cargo test -p taim_wgpu --lib copyless_surface_encodes_sdr_once --locked -- --ignored
cargo test --test gpu color_output --locked
```

For preferences, change test-device `cmd uimode night yes/no`, global
`animator_duration_scale` 0/1, and system `user_rotation` while observing
`TAIM_ANDROID_INSETS`. Check actual settled pixels, keyboard dismissal/reopening,
and the visible caret after rotation; restore every prior setting. The original
local preference driver is saved with its evidence.

## Remaining acceptance

Physical ARM64/16 KiB GPUs, older Android releases, every stock/third-party IME,
all scripts/emoji sequences, accessibility/TalkBack, predictive back, clipboard,
autofill, stylus handwriting, platform services, process restoration, device-loss
recovery, memory pressure, production AAB/signing and store policies remain
unaccepted. Surface retention is not persisted state restoration.

There is no Flutter speed, memory, energy, startup or package-size superiority
claim. The next implementation focus is the Android accessibility-node/action
bridge, followed by TalkBack acceptance.

Platform contracts:
[Android font glyph bounds](https://developer.android.com/reference/android/graphics/fonts/Font#getGlyphBounds(int,android.graphics.Paint,android.graphics.RectF)),
[Canvas glyph drawing](https://developer.android.com/reference/android/graphics/Canvas#drawGlyphs(int[],int,float[],int,int,android.graphics.fonts.Font,android.graphics.Paint)),
[InputConnection](https://developer.android.com/reference/android/view/inputmethod/InputConnection).
