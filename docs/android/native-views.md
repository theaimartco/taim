# Android native hierarchy embedding

`android_view(factory, properties)` reserves an axis-aligned layout slot for an
explicitly registered Android View factory. Retained slot IDs preserve the
native instance across property/layout updates, modal hiding and Home/resume.
Android owns that subtree's input connection and accessibility nodes; Taim does
not publish duplicate virtual nodes for the slot. Other shells leave it empty.

```rust
use taim::prelude::*;
use serde_json::json;
let control = android_view("my.controls", json!({"label":"Continue"}))?
    .height(140.0)
    .enabled(true);
let slot_id = control.id();
```

Properties and enabled state accept reactive signals/closures. Properties must
be JSON objects up to 16 KiB. Invalid reactive properties dispose the native
instance; correcting the properties creates a fresh native generation. A scene
contains at most 64 slots / 2 MiB, each at most 8192 physical pixels per dimension
and 8,388,608 pixels in area, with a 33,554,432 pixel total allocation budget.
Oversized/invalid scenes fail closed. Factory names use 1–128 ASCII letters,
digits, periods, underscores or dashes. At most 32 factories are registered.

In an Activity subclass, call `registerTaimViewFactory` on the main thread after
`super.onCreate`. The standalone project generator includes the factory API and
host Java files automatically. Set the subclass as the manifest Activity while
retaining the `android.app.lib_name` metadata.

```java
registerTaimViewFactory("my.controls", (activity, props, host) -> {
    android.widget.Button button = new android.widget.Button(activity);
    button.setAllCaps(false);
    button.setOnClickListener(v -> {
        try { host.emit(new org.json.JSONObject().put("type", "click")); }
        catch (org.json.JSONException ignored) {}
    });
    return new TaimNativeViewFactory.Instance() {
        public android.view.View view() { return button; }
        public void update(org.json.JSONObject value) {
            button.setText(value.optString("label", "Continue"));
        }
        public void dispose() { button.setOnClickListener(null); }
    };
});
```

Return one unparented View and keep its identity stable. `update` is idempotent;
property-driven callbacks during create/update are suppressed. Implement
`resumed`, `paused`, `appearance(dark,reducedMotion)` and `dispose` for owned
resources. A visible instance resumes only while the Activity is resumed.
Hidden/modal slots pause producers; unmount, factory failure and Activity
teardown dispose them. `dispose` is called once for each returned instance.
Factory construction must clean up any resources allocated before throwing.

`Host.emit` marshals bounded JSON objects to the UI thread and rejects callbacks
from disposed/replaced, paused, hidden or disabled instances. Subscribe with
`Services::events()` and match `PlatformEvent::NativeView { id, generation,
event }`. A fresh native instance receives a new generation even when a retained
Rust slot recovers from factory failure. Lifecycle events have `type` values
`created`, `resumed`, `paused`, `disposed`; errors include `type:"error"` and a
code. Scene errors use ID/generation zero. Events use the existing bounded service
stream; applications should reconcile durable state through their properties.

The host retains NativeActivity callbacks while rendering Taim into a dedicated
SurfaceView behind the hardware-accelerated Android hierarchy. The host-owned
render surface is distinct from unsupported SurfaceView children in factories.

NativeActivity's OS InputQueue reaches winit before the Java hierarchy. The shell
claims gestures beginning in a visible native slot and forwards their pointer
streams to Android, keeping them out of the Taim gesture tree. Up to 16 pointers
are forwarded with stable pointer IDs; cancellation and pause clear the stream.
This currently forwards finger positions, not historical samples, pressure,
stylus buttons or arbitrary mouse/hover input. Native keyboard events now use
the key routing described below. Native focus cancels Taim's
composition and proxy editor ownership. Factories may report a local physical
caret rectangle through `Host.requestFocusRect`; it is bounded to the slot.
Taim applies density and content/IME translation once. Nested Taim scrolling
does not automatically scroll a native caret into view.

Clipping is rectangular and inherited from Taim scroll/container layout plus
window content insets. Disabled slots consume touches without activating native
children and hide their subtree from accessibility. Every active Taim modal hides
all native slots, including slots inside that modal. Later overlapping semantic
Taim content conservatively hides a native slot. Arbitrary decorative GPU
paint-over, rotations, rounded masks and interleaving GPU/native layers are not
supported; keep native slots in nonoverlapping layout regions.


## Native keyboard routing

While a native control owns focus, the shell forwards winit keyboard events to
that native subtree rather than the Taim editor. Standard Android editing,
letter/digit, function, navigation, numpad and modifier key identities are mapped
back to Android KeyEvents. Unidentified Android keycodes retain their value.
The dispatcher tracks down/up, repeat counts and left/right Shift, Ctrl, Alt and
Meta, plus lock keys while native focus is active. Shortcuts go through Android's
`dispatchKeyShortcutEvent` before ordinary key dispatch. A translated printable
Unicode string that differs from the reconstructed virtual keyboard mapping uses
Android's character event path, avoiding insertion of the wrong layout's text.

The main-thread dispatcher checks both slot ID and native generation. It keeps
at most 64 held keys, binds each to the actual native View that received its down,
and sends canceled releases when focus changes, the view hides/disables/unmounts,
or the Activity pauses. Repeats without a matching down are ignored. A delayed
key from an older native generation cannot type into its replacement. Native
buttons receive down/up activation normally; application shortcuts and explicit
Taim proxy-editor ownership continue through their existing paths.

The extended API35 16 KiB emulator suite passes 34 checks (22 original embedding
checks plus 12 keyboard checks). The shell has 61 host tests, and the UI contract
has 39; the separate normal/secure/multiline editor protocol passes 29 checks.
The extended suite covers real shell-injected typing,
Backspace, arrow/forward-delete, Ctrl+A replacement, Shift and Enter activation.
It also sends repeat/delete KeyEvents through a real native InputConnection,
and directly checks translated Unicode, stale generations and orphan repeats.
This is reconstructed winit input: original device IDs, scan codes, source flags,
exact repeat/timing metadata, ACTION_MULTIPLE payloads not exposed by winit, and
modifier/lock state established before native focus are not preserved. Physical
keyboard layouts, OEM IMEs and cross-framework Tab traversal still need separate
acceptance; this does not claim full raw Android input-event equivalence.

The built-in `taim.texture` factory owns a real TextureView/SurfaceTexture
producer. Its signed Android ARGB integer `color` property paints a static frame;
it allocates/releases its Surface with visibility/activity lifecycle and has no
idle animation loop. This establishes producer lifetime and system composition.
It is **not external GPU texture import into Taim's renderer**. SurfaceView
subtrees are rejected; factories must not insert them later. Camera2/player,
WebView and map SDK adapters still require their own implementation, permissions,
resource lifetime and acceptance. Native View state is retained only within the
living Activity; process restoration remains explicit application state.

Build the real controls/texture fixture and run isolated emulator acceptance:

```sh
python3 scripts/android/counter.py build --app native-view-probe --abi x86_64
python3 scripts/android/native-view-probe.py --serial emulator-5594 \
  --apk target/android/x86_64/debug/native-view-probe/taim-native-view-probe.apk
```

The API35 x86_64 16 KiB emulator embedding acceptance originally passed 22 checks, including
actual texture screenshot colors, native touch/events, Unicode/InputConnection,
modal hiding, disabled input, clipping, generation/stale callback fencing,
SurfaceView rejection and disposal. Host regressions cover geometry/scroll clips,
property validation, modal transitions and composition release.

The [keyboard follow-up record](native-keyboard-acceptance-20261030.json) records
the extended key-routing suite and current source/artifact hashes.
The original [acceptance record](native-views-acceptance-20261030.json) contains source and
artifact hashes, exact test reports and remaining limits. The counter also passes
9 touch/surface/process-restoration checks, editor protocol passes 29 checks,
and platform/notifications pass 14 checks plus cold-intent replay. Both Android
ABIs build; a fresh API36 standalone project builds and passes APK signature/16
KiB alignment checks.

Physical TalkBack/IME/GPU/OEM/API acceptance remains open. See
[the capability inventory](capabilities.md) for the remaining Android work.
