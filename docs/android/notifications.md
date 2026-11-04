# Local Android notifications

Taim's native host supports local notification channels, posting/updating,
cancelling, status queries, app/channel settings and cold/warm intent metadata.
The API uses the existing bounded asynchronous Android service transport.
It runs on the framework's API 26+ baseline without an AndroidX dependency.

## Enable in an application

Generate the app with the opt-in permission declaration:

```bash
python3 scripts/android/project.py /path/to/my-app \
  --package com.example.myapp --notifications
```

For an existing application, declare
`<uses-permission android:name="android.permission.POST_NOTIFICATIONS" />` in
its manifest. Include all host Java files, including TaimNotifications.java.
Creating a channel or posting never implicitly opens a permission prompt.
On API 33+, request POST_NOTIFICATIONS at an appropriate user action before
posting. On older releases, app/channel settings still determine availability.
This follows Android's [notification permission contract](https://developer.android.com/develop/ui/compose/notifications/notification-permission).

```rust,ignore
use taim::android::{
    Notification, NotificationChannel, NotificationImportance,
    NotificationVisibility, Request, ServiceError,
};

let client = taim::android::services().ok_or(ServiceError::Unavailable)?;
let device = client.request(Request::DeviceInfo).await?;
if device["sdk"].as_u64().unwrap_or(0) >= 33 {
    let permissions = client.request(Request::Permissions(vec![
        "android.permission.POST_NOTIFICATIONS".into(),
    ])).await?;
    if permissions[0]["granted"] != true { return Err(ServiceError::Denied); }
}
client.request(Request::NotificationChannel(NotificationChannel {
    id: "messages".into(), name: "Messages".into(),
    description: "New messages".into(),
    importance: NotificationImportance::Default,
})).await?;
client.request(Request::NotificationPost(Notification {
    id: 42, channel: "messages".into(),
    title: "New message".into(), body: "Open the app to read it".into(),
    small_icon: Some("ic_notification".into()), // Your app's drawable resource.
    payload: Some("conversation:42".into()),
    visibility: NotificationVisibility::Private,
    auto_cancel: true, only_alert_once: true, timeout_ms: None,
})).await?;
```

Run this in an application task, not by blocking the UI thread. Supply a drawable
named `ic_notification` in the example, or use `small_icon: None` for the system
fallback. Resource names are app-local drawable names, not file paths. Missing
custom resources return Invalid; the host does not silently replace a typo.

Posting the same nonnegative ID updates the app's untagged notification with
that ID. A missing channel is Invalid; blocked app/channel notification settings
or missing permission returns Denied. A successful post means native dispatch,
not proof of a heads-up alert or delivery to the display. Users and the OS
control interruption behavior. Recreating/updating a channel changes its
name/description while preserving its existing importance and user choices.
See Android's [notification construction guidance](https://developer.android.com/develop/ui/compose/notifications/create-notification).

## Status and settings

| Request | Response |
| --- | --- |
| NotificationChannel | `{id, name, importance, enabled}` |
| NotificationChannelDelete | null |
| NotificationPost, NotificationCancel | null after native dispatch |
| NotificationStatus | `{enabled, permissionGranted, channel, active}` |
| NotificationSettings | null after opening app/channel system settings |
| PermissionStatus | Permission array without opening a prompt |
| AppSettings | null after opening this app's system settings |

The optional channel status is null if missing/not requested. Active entries
are `{id, channel}` for this app's untagged notifications; titles, bodies and
routing data are not returned. Native removal is asynchronous: a status query
immediately after cancellation can briefly include a notification being removed.

`PermissionStatus` has the same `{name, granted, canAskAgain}` shape as
Permissions. Android's rationale flag is not a definitive permanent-denial
indicator; it can also be false before a first request. Notification status
reports `permissionGranted: true` before API 33 because that runtime permission
does not apply there.

## Tap routing

Subscribe through `Services::events()` and match:

```rust,ignore
if let taim::android::PlatformEvent::Intent {
    notification: Some(tap), ..
} = event {
    // Route using tap.id and tap.payload, after application validation.
}
```

The host creates explicit immutable Activity PendingIntents, with distinct
identities per notification ID. The app's activity receives warm taps through
onNewIntent; its launch intent carries cold-tap metadata. Initial intent replay
also makes it available to a newly attached service subscriber. Taps neither
execute arbitrary Java methods nor open a supplied external URL. Incoming
intents can be supplied by other applications, so routing metadata is untrusted
and must never be an authorization decision. Do not include secrets in payloads.

## Bounds and verification

The API 35 x86_64/16 KiB fixture passed 14 integration checks plus one cold-intent
replay check. The host suite passed 50 tests, targeted host/ARM64 strict Clippy
passed, and a newly generated notification-enabled project built both ABIs.
Its development APK signature and 16 KiB ZIP alignment verified. See the
[source/artifact acceptance manifest](notifications-acceptance-20261030.json).

Channel IDs accept 1–128 ASCII letters/digits/dots/dashes/underscores. Names are
limited to 256 UTF-8 bytes, descriptions to 1024, titles to 1024, bodies to 8192
and payloads to 4096. Optional expiry is 1 millisecond through seven days and is
a system timeout, not a scheduled delivery time. Private/Secret lock-screen
policies do not include an unredacted public-version notification.

The separate test-device fixture can exercise denied/granted permission,
channel metadata updates preserving importance, posting/updating distinct IDs,
private visibility, actual PendingIntent routing, subscriber replay and cleanup:

```bash
python3 scripts/android/counter.py build --app platform-probe --abi x86_64
python3 scripts/android/platform-probe.py --serial YOUR_TEST_DEVICE \
  --apk target/android/x86_64/debug/platform-probe/taim-platform-probe.apk \
  --notifications --output target/android/runtime-smoke/notifications-new
```

This opt-in fixture changes only its own notification permission and posts its
own test notifications. The second instrumentation launch verifies cold initial
intent replay using explicit fixture extras; it is not a physical shade-tap
after-process-death acceptance test. Physical device/shade/lock-screen behavior,
API 26 compatibility runtime and OEM interruption policies still need acceptance.

This milestone does not implement push delivery/FCM, exact alarms, scheduled
notifications, remote-input actions, foreground services or WorkManager. Those
are separate capability milestones with their own lifecycle and OS contracts.
