# Android accessibility and permissions

This implements the identified virtual-accessibility and permission-state gaps.
Taim apps expose their own semantic UI without requesting accessibility-service
privileges. A custom drawn View must provide `View::describe`; embedded Android
controls use their own native accessibility subtree.

## Runtime permissions

The native host reports current OS grants separately from request history and
rationale. It never treats `shouldShowRequestPermissionRationale == false` alone
as permanent denial. Runtime dialogs are explicit, manifest-aware, foreground-only
and serialized, including while a timed-out RPC's native dialog is still open.
Granted, undeclared, unavailable and non-runtime permissions do not open runtime
permission dialogs. Request lists contain 1–32 unique Android permission names.

```rust,ignore
use taim::android::{services, PermissionState, Request, ServiceError};
let client = services().ok_or(ServiceError::Unavailable)?;
let names = vec!["android.permission.CAMERA".to_owned()];
let current = client.permission_status(names.clone()).await?;
let result = client.request_permissions(names).await?;
match result[0].state {
    PermissionState::Granted => { /* use the capability */ }
    PermissionState::SettingsRequired => {
        // Open settings only in response to the user's explicit choice.
        client.request(Request::AppSettings).await?;
    }
    _ => { /* retain a usable denied/cancelled/unavailable UI */ }
}
```

Both typed helpers and the existing JSON `Request::Permissions` /
`Request::PermissionStatus` responses expose:

| Field | Meaning |
| --- | --- |
| `name`, `granted` | Permission identity and fresh OS grant |
| `declared`, `available`, `runtime` | App declaration, OS existence and runtime-dialog category |
| `requestedBefore` | This installation has attempted a Taim-managed request |
| `lastRequestCancelled` | Latest callback omitted a result for this permission |
| `shouldShowRationale` | Android's current rationale observation |
| `canAskAgain` | A runtime request is appropriate given the observations; OS decides whether to show UI |
| `settingsRequired` | A completed denial had no rationale; settings is the suggested recovery path |
| `state` | `granted`, `notRequested`, `denied`, `cancelled`, `settingsRequired`, `notDeclared`, `unavailable`, `restricted` |

Cancellation does not establish denial. Permission history is committed before
completion so a process exit cannot resurrect a cleared denial. An observed
settings grant clears prior denial evidence; later revocation is queried fresh.
Public Android APIs cannot prove every policy/admin/auto-reset state. Therefore
`settingsRequired` is an observation-based recovery suggestion, not a claim of
irrevocable denial or a guarantee that settings can grant it. Permissions first
requested outside Taim, or an unobserved grant/revoke cycle, may need another
observation. Always handle denial from the actual protected operation too.

`PlatformEvent::PermissionsChanged(Vec<PermissionStatus>)` reports changes after
request completion and Activity resume for up to 128 permission names observed
through this Activity's queries. Subscribe before requesting; query initial state
and reconcile after stream interruption. No background polling runs. Persisted
history survives process restarts; event subscriptions remain lifecycle-owned.

The generator accepts repeatable explicit permission declarations and deduplicates
them with notification/location/background-work flags:

```sh
python3 scripts/android/project.py /path/to/app --package com.example.app \
  --permission android.permission.CAMERA --notifications
```

A declaration does not request or grant permission. Capability-specific Android
sequencing still applies: for example coarse/fine location together where needed,
foreground before background location, and SDK-specific Bluetooth/media rules.
The generic permission transport accepts declared Android runtime permissions;
it does not implement every corresponding sensor/SDK feature.

## Special access

`Request::SpecialAccessStatus(kind)` queries OS-managed access;
`Request::SpecialAccessSettings(kind)` opens its system settings page. Both return
`{kind,supported,granted}`. Opening a page never grants access; requery on return.
`SpecialAccess` covers Overlay, WriteSettings, InstallPackages, AllFiles (API30+),
ExactAlarms (API31+), BatteryOptimization and FullScreenIntent (API34+).
Missing settings handlers return Unsupported. Special permissions must still be
declared where Android requires them; the generator's `--permission` is explicit.
Battery optimization opens the system exemption list without silently requesting
an exemption. `supported` describes the settings contract on this release;
exact alarms are unrestricted by this special-access gate on older releases.

## Accessibility behavior

The Android virtual provider publishes stable identities, ordered parents/children,
labels/descriptions, control classes, checked/selected/enabled/focus states, range
values, live-region flags and rectangular bounds. Semantic geometry now inherits
actual Taim ancestor clips. Hidden/fully clipped nodes and hidden descendants are
excluded from provider hierarchy/search/focus and autofill. Search is restricted
to the requested subtree. Removed or newly hidden nodes release accessibility,
hover and autofill focus. Duplicate IDs, cyclic/missing parents and invalid bounds
are rejected before replacing a valid snapshot.

Normal editable fields support SetText, UTF16 SetSelection, character/word/
paragraph cursor movement with optional selection extension, Copy, Cut, Paste,
and keyboard-focus release. Selection-only updates publish even when text does
not change. Routing validates the current semantic revision and live semantic
identity before touching an editor. UTF16 offsets convert to UTF8 boundaries and
the editor's grapheme model. Paragraph movement uses newline boundaries; visual
wrapped-line movement is not advertised without layout-specific line metadata.

The production hardening follow-up serializes semantic commands in one FIFO and
binds asynchronous paste to its original Root/editor/context; late results are
rejected. Secure composition finish without preedit is a no-op and a closed
secure connection cannot write again. See [the audit](production-readiness.md).

Control actions are gated by advertised support. Expand/Collapse respects current
state rather than blindly toggling. Keyboard focus events are sent on transitions;
text and selection updates emit accessibility events. Disabled controls remain
readable but reject activation/editing. Native-view factories retain their own
Android input/accessibility behavior and modal/disabled gating.

Secure fields expose neither value nor selection, provide no copy/cut/paste or
plain SetText actions, suppress live announcements and do not include secret values
in search/events. Explicit password autofill still routes through the protected
editor path without publishing the secret back into virtual nodes. Hidden and
disabled autofill targets reject writes.

## Verification

```sh
python3 scripts/android/counter.py build --app platform-probe --abi x86_64
python3 scripts/android/access-probe.py --serial emulator-5594 \
  --apk target/android/x86_64/debug/platform-probe/taim-platform-probe.apk
```

The dedicated fixture performs real camera-permission denial dialogs without
opening a camera. It tests 27 permission/accessibility checks, then 2 new-process
history/settings-grant checks and 1 new-process revocation check. The cancellation
case directly supplies Android's empty-callback protocol. Denial and grant are
real OS permission changes. Synthetic virtual trees separately exercise hidden
nodes, scoped search, malformed cycles, focus cleanup and secret-value withholding.

The [acceptance record](accessibility-permissions-acceptance-20261031.json) includes
exact artifact/source hashes and regression results. The final run passes all 30
new access checks, 40 host UI tests and 65 shell tests. Regressions pass 34 native
view/keyboard checks, 29 IME checks, 9 runtime/restoration checks and 14 platform
checks plus cold-intent replay. ARM64/x86_64 fixtures and a fresh API36 generated
project build with development signatures and verified 16 KiB alignment.
These are emulator and host results. Physical TalkBack speech/gesture behavior,
Switch Access, real autofill-provider behavior and the OEM/API matrix require
those environments; code or instrumentation cannot certify them by assertion.
This milestone does not claim complete Flutter ecosystem parity.
