# Android integrations: biometric, BLE, NFC, geofencing, foreground service, video, camera, FCM, Maps, Billing

**Status (2026-11-04): code complete, not device-accepted.** Everything below compiles for
arm64 Android (Rust + Java). FCM, Maps and Billing Java also compile against their real Google
libraries in a generated Gradle project. Only the biometric success path, the status query and
the strong/weak/credential prompts have run on a physical phone (OPPO CPH2637, Android 16).
Nothing here is production-cleared until the acceptance matrix at the end passes on physical devices.

## Architecture

Framework-only integrations are built-in plugin channels and native-view factories in
`winit/android/com/theaimartco/taim/`. They have no AndroidX or Play-services dependency and
ship in every app:

| Integration | Java | Rust API | Manifest (project.py flag) |
|---|---|---|---|
| Biometric prompt | `TaimBiometric` (service op) | `Services::biometric_status`, `authenticate` | `--biometric` |
| Bluetooth LE central | `TaimBluetooth`, channel `taim.bluetooth` | `ble_scan/connect/read/write/subscribe/disconnect`, `BluetoothEvent` | `--bluetooth` |
| NFC reader + NDEF | `TaimNfc`, channel `taim.nfc` | `nfc_start_reader`, `nfc_write_ndef`, `NfcTag` | `--nfc` |
| Geofencing | `TaimGeofence` + `TaimGeofenceReceiver`, channel `taim.geofence` | `geofence_add/remove/list/restore/pending`, `GeofenceTransition` | `--geofencing` |
| Foreground service | `TaimForeground` + `TaimForegroundService`, channel `taim.foreground` | `foreground_start/update/stop/running` | `--foreground-service TYPE` |
| Inline video | `TaimVideoView`, factory `taim.video` | `android_video_view(|| AndroidVideo)` | `--internet` for HTTPS |
| Camera preview | `TaimCameraView`, factory `taim.camera` | `android_camera_preview(|| (lens, active))` | `--permission android.permission.CAMERA` |

Google-library integrations are opt-in Gradle extensions in `scripts/android/extensions/`.
`project.py` copies their Java, adds dependencies, enables AndroidX and lists them in the
`com.theaimartco.taim.extensions` meta-data; `TaimActivity` attaches them after `onCreate`:

| Integration | Java | Rust API | Flag / secrets |
|---|---|---|---|
| Push (FCM) | `TaimFcm`, `TaimFcmService`, channel `taim.fcm` | `push_token/subscribe/pending`, `PushEvent` | `--fcm path/google-services.json` |
| Google Maps | `TaimMaps`, factory `taim.map` | `android_map_view(|| AndroidMap)` | `--maps`; key from `TAIM_MAPS_API_KEY` at build time |
| Play Billing | `TaimBilling`, channel `taim.billing` | `billing_connect/products/purchase/purchases/acknowledge/consume`, `PurchaseUpdate` | `--billing` |

Plugin errors map to typed `ServiceError`s: `Denied`, `Cancelled`, `Unsupported`, `Disabled`
(radio/service off), `NotFound`, `Busy`, `Timeout`, `Lockout`, `LockoutPermanent`, `NotEnrolled`.

## Contracts and known limits

- **Biometric.** Android 10+ (API 29). On API 29 the class (strong/weak) is not enforceable.
  A successful prompt proves local user presence only. It is **not** a server credential, and it
  does not yet bind a Keystore key (CryptoObject). Use it for UI gating, not for unlocking secrets.
- **BLE.** Central role only (no advertising/peripheral). One GATT operation per connection at a
  time (`Busy` otherwise, never silently queued). Scans are foreground-only and bounded to 30 s.
  No MTU request, PHY selection, bonding/pairing UI or long writes beyond 512 bytes.
  `neverForLocation` is declared. Remove it if scan results are used to derive location.
- **NFC.** Foreground reader mode (NFC-A/B/F/V/barcode) and NDEF read/write of the latest tag.
  No host card emulation, no raw ISO-DEP/MIFARE transceive, no background tag dispatch.
- **Geofencing.** Uses framework `LocationManager.addProximityAlert`, not Play services
  `GeofencingClient`. It is less battery-efficient and has OEM-dependent latency. Android clears
  alerts on reboot/app update: call `geofence_restore` at startup. Transitions are persisted
  (latest 64) and drained with `geofence_pending`. Background delivery needs
  ACCESS_BACKGROUND_LOCATION, which Play reviews strictly.
- **Foreground service.** Keeps the process at foreground priority so running Rust tasks continue.
  It does not run Rust after the activity is destroyed (use background work). Android 12+ refuses
  background starts (`Denied`). Android 14+ requires the declared type and its permission.
  The notification icon is a system sync icon; app branding is pending.
- **Video.** Framework `MediaPlayer` into a TextureView: HTTPS, `content://`, `asset:///`.
  No adaptive streaming (HLS/DASH via ExoPlayer/Media3), DRM, subtitles, picture-in-picture or
  media-session controls.
- **Camera.** Camera2 preview only. Capture uses the existing delegated `CaptureMedia`. The
  rotation/aspect transform must be verified on front and back cameras in all orientations.
- **FCM.** Data messages are persisted (latest 32) and delivered live. Background notification
  messages are shown by the system. Requires Google Play services on the device.
- **Maps.** Taim native slots reject SurfaceView children. Interactive `MapView` composition may be
  rejected or render incorrectly; `lite: true` (static bitmap) is the safe mode until verified.
  Native-slot properties are limited to 16 KiB (about 150 markers).
- **Billing.** The client never grants entitlement. Verify `Purchase::token` on your server, then
  acknowledge/consume within 3 days. Testing requires a Play Console app on an internal test track
  and license-tester accounts.

## Device acceptance (pending)

Test apps:

- `python3 scripts/android/counter.py build --app integrations-probe --abi arm64-v8a`
  covers biometric, BLE, NFC, geofence, foreground service, video and camera.
- For extensions, generate a project with `--fcm --maps --billing`, copy
  `scripts/android/extensions/probe/lib.rs` to `rust/src/lib.rs`, add `futures = "0.3"` and
  `./gradlew assembleDebug`.

Both log `TAIM_PROBE <step> <result>`.

| Check | Required evidence |
|---|---|
| Biometric | success ✅ (OPPO); negative-button → Denied, credential path → DeviceCredential, Home → Cancelled, lockout after 5 failures |
| BLE | scan finds a known peripheral; connect + read Device Name; write; notifications; peer power-off → Disconnected; permission denial → Denied; radio off → Disabled |
| NFC | NDEF tag read, write, tag removed mid-write; needs an NFC phone (the OPPO CPH2637 has none) |
| Geofence | mock-location entry/exit live and after process death (`pending`), reboot + `restore` |
| Foreground service | starts and shows the notification, survives backgrounding for 10+ minutes, background start refused |
| Video | HTTPS + asset playback, pause on Home, resume, rotation, contain/cover |
| Camera | back/front preview orientation in portrait and landscape, release on pause (another camera app opens), permission denied |
| FCM | token, topic message foreground/background/killed, token rotation |
| Maps | interactive composition, markers, camera events, lite mode |
| Billing | internal-track product query, purchase, pending purchase, acknowledge, consume, restore |
