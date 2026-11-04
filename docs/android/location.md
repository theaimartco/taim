# Foreground Android location

This milestone adds permission/provider status, cached and fresh samples,
bounded update sessions, typed samples and listener cancellation. It uses Android
LocationManager directly and has no Google Play Services dependency. Location is
acquired only by explicit calls while the host Activity is resumed.

## Enable and request permission

```bash
# Coarse permission only:
python3 scripts/android/project.py /path/to/my-app \
  --package com.example.myapp --location
# Or coarse plus fine, when the application needs precise access:
python3 scripts/android/project.py /path/to/precise-app \
  --package com.example.preciseapp --location precise
```

The flag declares ACCESS_COARSE_LOCATION, optionally ACCESS_FINE_LOCATION. It
does not declare background location, create a foreground service, turn location
on or open a permission prompt. Applications request permission explicitly
through Request::Permissions at an appropriate user action. When requesting fine
location, include coarse too so Android's approximate choice remains available.
See Android's [location permission guidance](https://developer.android.com/develop/sensors-and-location/location/permissions).

```rust,ignore
use taim::android::{LocationAccuracy, LocationOptions, Request, ServiceError};
let client = taim::android::services().ok_or(ServiceError::Unavailable)?;
let permissions = client.request(Request::Permissions(vec![
    "android.permission.ACCESS_COARSE_LOCATION".into(),
    "android.permission.ACCESS_FINE_LOCATION".into(),
])).await?;
// Inspect permission results before requesting coordinates.
let fix = client.current_location(LocationOptions {
    accuracy: LocationAccuracy::Balanced,
    maximum_age_ms: 5_000,
    timeout_ms: 10_000,
}).await?;
```

Run requests inside an application task. LocationStatus can inspect permissions
and providers without acquiring coordinates or prompting. LocationSettings opens
the OS settings page; it does not silently change any setting.

## One-shot and cached samples

| Request / helper | Response |
| --- | --- |
| LocationStatus | `{enabled, coarseGranted, fineGranted, foreground, providers, active}` |
| LocationSettings | null after settings dispatch |
| LastLocation { accuracy, maximum_age_ms } | Sample map, or null if no acceptable cached sample |
| CurrentLocation(LocationOptions) | Sample map or acquisition error |
| Services::current_location(options) | Validated LocationFix |

Balanced prefers network location and falls back to GPS when fine permission is
granted. High prefers GPS with fine permission, otherwise an available network
provider. Preference is not a precision or accuracy guarantee. A device with no
enabled compatible provider returns Unsupported; this implementation does not
substitute a Play Services fused provider or require fine permission implicitly.

Cache age is bounded to 0–86,400,000 ms and acquisition timeout to 1–60,000 ms.
Zero maximum age means a new sample timestamped since the request started, not
a sample whose delivery age is mathematically zero. Staleness uses Android's
monotonic elapsed-realtime clock, not the wall clock. Invalid/future monotonic
timestamps and malformed coordinates are rejected. No eligible cache returns
null for LastLocation; CurrentLocation starts a listener and ends with Timeout
if no eligible fix arrives.

LocationFix contains latitude/longitude in degrees, optional accuracy/altitude/
speed/bearing, Unix milliseconds, monotonic sample time, age at serialization,
provider name, Android's mock flag and `precise_permission`. The latter describes
permission held at delivery, not whether the measured fix is accurate. Optional
measurements are null when Android does not supply them. The framework does not
automatically log or persist coordinates, or include them in its event replay cache.

Dropping a pending CurrentLocation/current_location future posts cancellation
to the native request and removes its listener, rather than waiting for the
deadline. Cleanup is queued to the Activity's UI thread. Normal completion,
timeout, pause and destruction also remove listeners and timers.

## Bounded update subscriptions

```rust,ignore
use taim::android::{LocationAccuracy, LocationWatchOptions};
let mut subscription = client.watch_location(LocationWatchOptions {
    accuracy: LocationAccuracy::High,
    interval_ms: 1_000,
    minimum_distance_m: 0.0,
    duration_ms: 30_000,
}).await?;
while let Some(sample) = subscription.next().await {
    match sample {
        Ok(fix) => { /* update application state */ }
        Err(reason) => { /* session ended; inspect LocationEnd */ }
    }
}
// Dropping a still-active subscription also posts native removal.
```

At most four location acquisitions/watches may be active per host. Watch
interval hints are 1,000–60,000 ms, distance hints are finite 0–100,000 metres,
and sessions expire after 1–60,000 ms. OS/coarse-provider throttling can deliver
less frequently than requested. A session may expire without delivering a fix.
Renew explicitly while still foreground if the feature needs more updates.

The helper subscribes before starting native updates, so early samples cannot
be lost between the request and response. Each session receives a monotonically
allocated activity-local token, which is not reused. The helper filters events
by token. `next()` returns a terminal Err(LocationEnd) once, then None. Stop
reasons include Stopped, Background, Expired, Disabled, Denied, Unavailable and
Failed. Explicit `subscription.stop().await` and Drop remove listeners; saturated
or closed event streams cause the guard to stop the native watch.

Advanced callers can use Request::WatchLocation/StopLocation and the shared
PlatformEvent::Location/LocationEnded events directly. They must manage the
returned `{watch, provider}` token. Dropping a watch *creation* future before its
token is acknowledged cannot construct the subscription guard; any created
native session still expires within its configured duration or stops on pause.

All acquisitions and watches stop on Activity pause, even when another visible
window/dialog caused the pause. They are not automatically resumed. A new request
after resume is an application decision. Samples queued after removal are ignored.
Provider disable and permission loss terminate affected sessions. No foreground
service, background tracking, geofencing, GNSS measurements or motion-sensor API
is implemented by this milestone; background work and sensors remain separate.

Errors distinguish Denied, LocationDisabled, Cancelled, Unsupported, Timeout,
Busy and Unavailable. LocationStatus itself returns permission/settings state
without requesting a fix. A precise-permission downgrade can cause Android to
restart the process; ordinary saved-state rules apply, and watches are not
restored into a new activity/process.

## Verification

```bash
python3 scripts/android/counter.py build --app location-probe --abi x86_64
python3 scripts/android/location-probe.py --serial YOUR_DEDICATED_EMULATOR \
  --apk target/android/x86_64/debug/location-probe/taim-location-probe.apk \
  --output target/android/runtime-smoke/location-new
```

The fixture refuses physical devices. Its separate instrumentation installs
synthetic GPS/network test providers, tests denied/approximate/precise permission
paths and logs no coordinates. It removes its providers in finally and restores
the original emulator location setting and mock-location app-op. Use a dedicated
emulator because mock providers temporarily replace its system providers.

Checks include cached/fresh fixes, cache age, timeout and one-shot future drop,
watch delivery/stop/drop/expiry, capacity rejection, disabled provider/settings,
background watch stop, foreground cancellation and activity teardown cleanup.
The host unit suite also validates bounds and rejects malformed sample values.

Final acceptance: 16 location integration checks and 55 host unit tests passed.
Targeted host/ARM64 strict Clippy passed. A fresh two-ABI location project built,
and its development APK signature/16 KiB ZIP alignment verified. Its manifest
declares only coarse/fine foreground location permissions. Final shared-service
regressions also passed: 14 platform/notification checks plus one cold-intent
check, and 18 media checks. Source/artifact hashes and reports are captured in
the [acceptance manifest](location-acceptance-20261030.json).

These are API 35 x86_64/16 KiB emulator results, not physical GPS acquisition,
OEM power behavior, coarse-permission dialog UX, API 26 runtime or energy
acceptance. The native implementation follows the public
[LocationManager contract](https://developer.android.com/reference/android/location/LocationManager).
