# Android capability expansion

See the [production hardening audit](production-readiness.md) before making release claims.

This tracks built-in framework support separately from extension infrastructure.
A Java/Kotlin plugin channel makes integration possible; it does not mean a
camera, map, billing or other SDK implementation already exists.

| Capability | Current implementation | Next work / acceptance |
| --- | --- | --- |
| App host, retained UI/signals, touch, rendering | Implemented, tested emulator workloads | Physical API/GPU/page-size matrix |
| Text, composition, password input, fonts/insets | Implemented; 29 editor protocol checks | Stock/third-party IMEs and physical language matrix |
| Accessibility and explicit autofill | Semantic clipping, hidden/scoped search/focus, Unicode selection/movement, clipboard actions and secret-safe autofill implemented; see accessibility-permissions.md | Physical TalkBack/Switch Access/password-manager acceptance |
| Back/predictive back, intents, opt-in state restoration | Implemented; back and saved-task process-kill fixtures | Physical gesture and recreation matrix |
| Clipboard, permissions, SAF, assets, sharing, Keystore, haptics | Typed permission observations, durable denial/cancellation history, resume events and special-access settings implemented | Real denial/process/revocation fixture; broader picker/OEM/API acceptance remains |
| Native extension calls/events | Implemented and echo fixture tested | SDK-specific integrations still need implementation |
| Local notifications and permission/settings UI | Implemented; see notifications.md | Physical shade/lock-screen/tap acceptance |
| Camera/media selection and delegated capture | Photo/video picker, native photo/video capture, preview dispatch, metadata/grants and owned-file recovery/deletion implemented | 18 emulator protocol/lifecycle checks; physical camera/codec/OEM matrix pending; see media.md |
| Embedded camera preview and video playback | `taim.camera` (Camera2 preview) and `taim.video` (MediaPlayer) native views; see integrations.md | Code complete; physical orientation/codec/lifecycle acceptance pending |
| Foreground location | Typed samples, status/cache/fresh fixes, bounded watches and pause/drop cleanup implemented | Synthetic OS-provider acceptance; physical GPS/OEM/API matrix pending; see location.md |
| Motion/environment sensor snapshots | Typed inventory and fresh bounded foreground samples for 13 permission-free sensor kinds; pause/timeout cleanup; see sensors.md | Physical calibration/OEM/power validation; continuous streams/fusion and background sensing remain separate |
| Geofencing | Framework proximity alerts with persisted transitions and restore; see integrations.md | Code complete; mock-location, process-death and reboot acceptance pending |
| Durable background work | JobScheduler, private ledger, headless Rust entry, constraints, periodic/retry/cancel and reconciliation implemented | 14 scheduler/lifecycle + 3 headless restart checks; real reboot/Doze/OEM/API matrix pending; see background-work.md |
| Push and long-running foreground services | FCM Gradle extension (`--fcm`) and typed foreground-service control (`--foreground-service`); see integrations.md | Code complete; physical push delivery and long-run service acceptance pending |
| Native-view embedding | Retained factory slots, rectangular clips, native touch/keyboard/accessibility, lifecycle, modal hiding and TextureView producer implemented | See native-views.md; GPU texture import/arbitrary interleaving and physical matrix pending |
| WebView | Built-in retained `taim.web` adapter: inline/HTTPS documents, explicit scripting, native input/accessibility and lifecycle cleanup; see webview.md | Physical/provider matrix, remote/TLS fault tests; advanced browser workflows remain separate |
| Maps | Google Maps Gradle extension (`--maps`, `taim.map` view); see integrations.md | Code complete; interactive composition unverified (SurfaceView), lite mode is the safe fallback |
| Bluetooth/NFC/biometrics/billing | BLE central, NFC reader/NDEF, BiometricPrompt and Play Billing (`--billing`) implemented; see integrations.md | Biometric success path accepted on one physical phone; everything else pending physical acceptance |
| Release packaging | API 36 standalone Gradle project, APK/AAB signing, ARM64/x86_64 | Owner certificate and store/device acceptance |
| Flutter competition | Paired release counter/input fixture and harness | Representative lists/animation/startup/energy benchmarks on physical devices |

Implementation order agreed with the user: notifications + permission/settings,
then camera/media, location, background work and native-view embedding. Code
comes before the verification pass for each capability. Historical evidence is
kept intact; new source/artifact hashes accompany each acceptance record.

This is a concrete expansion sequence, not a claim of complete Android SDK or
Flutter ecosystem equivalence. See [platform evidence](platform-20261030.md)
and [local notification API](notifications.md).
