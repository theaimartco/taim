# Android implementation ledger

Checkpoint: 2026-10-30, experimental working tree. Existing unrelated user changes
were present. No immutable release candidate or physical runtime acceptance.
The tested API 35 x86_64/4 KiB Vulkan and 16 KiB GLES workloads pass.
See [native-host evidence](native-host-20261029.md); Vulkan corruption remains
an upstream limitation, and physical acceptance is still pending.

## Audit and gap matrix

| Capability | Existing implementation | Android gap / action | Verification / priority |
|---|---|---|---|
| Public app and signals | launch → shell App → Root | Explicit `run_android(AndroidApp)` added; retained API preserved | ARM64/external consumer compilation; P0 |
| Event-loop host | Desktop winit builder | NativeActivity handle injected; host feature selected on Android | APK exports checked; API 35/4 KiB emulator smoke passes; P0 |
| Feature selection | Desktop defaults | Vulkan-only and Vulkan/GLES Android opt-ins; desktop clipboard excluded | Android dependency tree and build; P0 |
| Lifecycle | Initial `resumed` only | Drop presentation target on suspend, retain root, recreate on resume | Three 4 KiB emulator cycles pass; physical QA pending; P0 |
| Frame scheduler | Suspend/deadline state machine | Surface availability participates in rendering gate | Existing host tests pass; device power trace pending; P0 |
| Rendering | wgpu plus software paths | Vulkan integrity guard and GLES SDR compatibility path | Device screenshots/GPU loss tests pending; P0 |
| Touch | winit conversion and view gestures | Existing path compiles; actual multitouch/cancellation unaccepted | Emulator taps/repaint pass; physical/multitouch QA pending; P0 |
| Hardware keys | Desktop modifier supplement | Taim logical-key/text path plus native factory key/shortcut/repeat routing and generation fencing | Native emulator key/IME checks pass; physical layouts/raw metadata/cross-framework Tab QA pending; P1 |
| Text/fonts | cosmic-text/fontdb, bundled font option | System discovery, script/emoji policy and API 31+ native color-glyph fallback | Hindi/Arabic/color-emoji emulator screenshots; physical QA pending; P0 |
| IME | Focus/IME vocabulary and desktop shell sync | Native snapshots, composition and secure redacted incremental connection | Protocol mixed-script/selection/deletion tests pass; physical IME matrix pending; P0 |
| Accessibility | Retained semantics and AT-SPI | Virtual AccessibilityNodeProvider, actions/hover/live/range, explicit autofill; native fixture passes | TalkBack physical QA; P0 |
| Clipboard | arboard worker | Native ClipboardManager text/HTML reads/writes and sensitive hint | Native clipboard fixture passes; foreground/device restrictions still apply; P1 |
| Safe areas/system UI | Generic mobile views | Native safe-area union, theme/motion observation and caret pan | Gesture navigation/rotation/keyboard QA; P0 |
| Navigation | View navigation models | API-isolated back/animation callbacks, app handlers, launch/warm intent events | Native back/deep-link tests; P1 |
| Packaging | Desktop/source tooling | Debug APK helper plus API 36 Gradle project, release APK/AAB and required signing environment | Signature/ELF/ZIP checks pass; P0/P1 |
| State restoration | In-memory reactive state | Versioned opt-in state checkpointed to native saved-task Bundle | Kill/recreate/relaunch tests; P1 |
| Platform services | No general Android bridge | Bounded async permissions, SAF/content/assets, intents/share, clipboard, haptics, Keystore and plugins | Typed lifecycle-aware adapter tests; P1 |
| Local notifications | Native Android service transport | Channels, post/update/cancel, status/settings, permission inspection and tap metadata | 14 platform checks + 1 cold-intent replay check pass; see notifications.md |
| Camera/media | Native picker/camera/viewer intents and private grant-only provider | Photo/video selection/capture, metadata, URI grants, recovery/delete | 18 media protocol/lifecycle checks; physical camera/codec matrix pending; see media.md |
| Foreground location | Native LocationManager with typed fixes/subscriptions | Permissions, cache/fresh acquisition, bounded watch, pause/drop cleanup | 16 synthetic-provider checks pass; physical GNSS/OEM acceptance pending; see location.md |
| Durable background work | JobScheduler + private SQLite ledger + headless Rust dispatcher | Constraints, periodic/retry/cancel, generation fencing, reconciliation | 14 scheduler + 3 headless restart checks; reboot/Doze/OEM acceptance pending; see background-work.md |
| Components | Existing retained views | Android behavior/accessibility/theme inventory needed | Device showcase acceptance; P1 |
| Flutter comparison | Methodology docs | Equivalent input/counter Flutter fixture and paired release comparison harness implemented | Same-device release workloads; P2 |

## Decisions

### A1 — Extend the existing shell

Keep Root, Scope, signals, rendering and the public app builder. Add an explicit
Android run method instead of restoring a legacy host or installing global JVM
state. Reuse winit 0.30's re-exported activity type to avoid glue-version conflicts.

### A2 — NativeActivity for the first slice

Use a small Java subclass of NativeActivity for insets, preferences and real
InputConnections while retaining winit's native lifecycle glue. The SDK helper
compiles it with javac/D8; no Gradle bootstrap is required. A test-only IME lives
in a separate APK. Android accessibility is now provided by the sibling virtual-view host.

### A3 — Vulkan with explicit GLES compatibility

`wgpu-android` enables Vulkan and GLES. Before rendering, Vulkan must pass mapped,
queue and staging roundtrips. Rejected initialization retries GLES; explicit
`wgpu-vulkan` never changes backend. COPY_SRC-less GLES surfaces resolve from a
retained, copyable SDR intermediate with appropriate sRGB encoding. No physical
backend/performance matrix or device-loss recovery is certified.

### A4 — Separate view lifetime from presentation lifetime

The shell retains root/signals/renderer and stores an optional presentation
target. Suspend releases it before returning to winit; resume creates it again.
Frame production stays suspended while absent. This is not persisted process
state. Memory pressure drops renderer caches; bounded device-loss recovery rebuilds graphics while retaining Root. Hardware loss injection remains unaccepted.

### A5 — SDK-tool debug packaging first

Build with the NDK compiler, then use aapt2, zipalign and apksigner directly.
Pin the demo to min API 26 and target/compile API 35. The script requires r28+
and checks ELF alignment and exported NativeActivity symbols. Generated signing
material remains under ignored target/. Production signing, Gradle/AAB integration
and current store-policy validation are separate milestones.

## Sequenced checklist

- [x] Inspect actual native entry path and platform blockers.
- [x] Add explicit Android activity API and native host selection.
- [x] Remove compile blockers in keyboard/Unix/clipboard paths.
- [x] Retain view state across presentation-target suspend/recreation.
- [x] Add counter cdylib, manifest, native logs and reproducible debug APK helper.
- [x] Build and sign an ARM64 Android APK; inspect alignment and entry exports.
- [x] Compile a fresh external path-dependent Android consumer.
- [x] Install, render, interact and resume on the tested API 35/4 KiB emulator.
- [x] Add adb-driven touch/repaint/lifecycle smoke with screenshots and reports.
- [x] Verify x86_64 build and add an independent Vulkan buffer-integrity probe.
- [x] Guard the tested Vulkan corruption and pass the 16 KiB GLES runtime workload.
- [ ] Resolve the upstream Vulkan failure and accept the physical API/device/GPU matrix.
- [x] Implement insets, system theme/motion preferences and native font fallback.
- [x] Implement native normal/secure editor connections and mixed-script protocol QA.
- [ ] Accept the physical stock/third-party IME and language matrix.
- [x] Implement Android accessibility/actions and explicit autofill; native instrumentation passes.
- [x] Harden clipping/hidden-node search and focus, semantic text selection/movement/clipboard, secret-safe events and durable permission-state/special-access flows.
- [ ] Accept physical TalkBack/password-manager behavior.
- [x] Add back navigation, opt-in state restoration and typed platform services.
- [x] Add local notifications, permission/settings inspection and tap routing.
- [x] Implement system media selection, delegated photo/video capture and URI lifecycle.
- [x] Implement foreground location acquisition, bounded watches and listener cleanup.
- [x] Implement durable deferrable background work and headless Rust execution.
- [x] Implement native hierarchy slots and a TextureView producer; see native-views.md.
- [x] Add generation-fenced native hardware/IME KeyEvent routing, shortcuts, repeats and focus/pause cancellation.
- [ ] Implement external GPU texture import/arbitrary layer interleaving and SDK-specific view adapters.
- [x] Add release AAB/APK signing and external Android project template.
- [ ] Accept a production app certificate and store submission (owner responsibility).
- [ ] Accept component showcase and physical recovery/memory-pressure behavior.
- [x] Build equivalent Flutter counter/input workload and paired comparison runner.
- [ ] Collect representative physical-device release performance/energy results.
- [ ] Review and sign Android production release gates.

## Evidence

The [2026-10-30 platform record](platform-20261030.md) describes the expanded implementation.
The [2026-10-29 native-host record](native-host-20261029.md) holds the earlier
build, runtime, input and color evidence. Earlier lifecycle builds and independent
4 KiB/16 KiB Vulkan probe results are preserved in the
[2026-10-28 investigation](runtime-acceptance-20261028.md).

Android remains experimental. TalkBack, physical-device/ARM64 runtime,
physical restoration matrices and production distribution are not accepted.
There are no Android performance, energy, startup, size-comparison or Flutter
victory claims. APK size alone is not a fair framework comparison.
