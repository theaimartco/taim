# Android media selection and delegated camera capture

This milestone adds image/video selection, full-file camera capture, native
viewer dispatch, metadata, read-grant lifecycle and owned capture-file recovery.
It uses the existing bounded async Services API and NativeActivity lifecycle.
It does not add an embedded camera texture, inline video player or direct
Camera2/CameraX recording session. Those require the native-view/texture milestone.

## Enable and use

```bash
python3 scripts/android/project.py /path/to/my-app \
  --package com.example.myapp --media
```

The generator copies all Java host classes and registers TaimMediaProvider with
authority `${applicationId}.taim.media`, `exported=false` and
`grantUriPermissions=true`. The placeholder follows Gradle application-ID
suffixes. In a hand-maintained project, register the same private provider and
include TaimMedia.java/TaimMediaProvider.java with the rest of the host.

The picker does not request broad storage access. Capture delegates to an
installed camera app; this integration does not add CAMERA or microphone
permissions to your app. If your own application separately declares CAMERA,
Android may require it to be granted before a capture intent can launch. Follow
the [MediaStore capture contract](https://developer.android.com/reference/android/provider/MediaStore#ACTION_IMAGE_CAPTURE).

```rust,ignore
use taim::android::{MediaCapture, MediaKind, MediaPicker, Request, ServiceError};
let client = taim::android::services().ok_or(ServiceError::Unavailable)?;
let selected = client.request(Request::PickMedia(MediaPicker {
    kind: MediaKind::ImagesAndVideos, maximum: 2,
})).await?;
let captured = client.request(Request::CaptureMedia(MediaCapture::Photo)).await?;
let uri = captured["items"][0]["uri"].as_str()
    .ok_or(ServiceError::Failed)?.to_owned();
client.request(Request::PreviewMedia(uri.clone())).await?;
// After the application has finished with its private capture:
client.request(Request::DeleteCapturedMedia(uri)).await?;
```

Run these requests from an application task; do not block the UI thread.
The interactive example is `examples/android_media_probe`. A normal launcher
start uses real system pickers/camera/viewers. The isolated instrumentation
opts into test responders through example-only code, never the production host.

## API and response shapes

| Request | Behavior / response |
| --- | --- |
| PickMedia(MediaPicker) | `{items:[metadata], source:"photoPicker" or "documents"}` |
| CaptureMedia(MediaCapture) | `{items:[metadata], source:"camera"}` |
| MediaInfo(uri) | Metadata without loading full content |
| CapturedMedia | `{items:[metadata], source:"owned"}`; up to 64 owned capture files |
| RetainMedia(uri) | Attempt to persist a read grant; returns metadata or Denied |
| ReleaseMedia(uri) | Release existing persisted read/write grants for this URI; returns null |
| PreviewMedia(uri) | Dispatch a native ACTION_VIEW with a read-only grant; returns null |
| DeleteCapturedMedia(uri) | Delete only a framework-owned capture and revoke temporary grants; returns null |

Metadata is `{uri, mime, name, size, persisted, owned}`. Unknown name/size/MIME
values are null. Size is bytes, not decoded pixels or playback duration.
Metadata queries run on the bounded I/O worker. Existing ReadDocument/WriteDocument
requests operate on granted content URIs; binary transfers remain bounded to
4 MiB. Large media should be consumed via URI-aware native integrations rather
than copied wholesale through JNI.

Selection accepts Images, Videos or ImagesAndVideos, with a maximum of 1–32.
On API 33+, the host uses the platform photo picker; if unavailable it falls back
to ACTION_OPEN_DOCUMENT. Earlier releases use the document picker directly;
this host does not require AndroidX or attempt the backported picker contract.
The OS may impose a smaller multiple-selection limit. Fallback results are
validated against the requested maximum too: excess selections fail, rather
than silently discarding user choices. Returned URIs are deduplicated, must be
content URIs, and their MIME types must match the requested media category.
See Android's [photo picker guidance](https://developer.android.com/training/data-storage/shared/photo-picker).

Transient grants are not silently made persistent. Call RetainMedia explicitly
if you need a selected URI after process/device restart. Providers may refuse
persistence; inspect the returned `persisted` flag or handle Denied. ReleaseMedia
does not delete the underlying external image/video. DeleteCapturedMedia rejects
foreign providers, arbitrary paths and invalid capture filenames.

## Capture ownership and lifecycle

Capture allocates an opaque UUID file under the app's private `files/taim-media`
directory and sends its URI as EXTRA_OUTPUT plus ClipData, with temporary
read/write grants. It does not accept a caller-supplied output path, thumbnail
result or replacement URI returned by a camera. RESULT_OK must leave a nonempty
output file. Full image/video decoding is the application's next step; nonempty
bytes alone do not prove valid encoding.

MediaCapture::Video accepts a 1–60 second duration hint and a high-quality flag.
Camera apps control the actual encoding and may ignore hints. This API does not
guarantee duration, resolution, orientation, codec, file size or battery use.

Only one interactive operation is active per host. Concurrent picker/camera/
permission requests return Busy. The existing service deadline is 120 seconds;
OS cancellation returns Denied, as with the existing document-picker contract.
Missing camera/viewer/provider support returns Unsupported. Stale result codes
cannot complete a later request; codes are not reused in an activity lifetime.

Normal cancellation, timeout and activity teardown revoke capture grants and
remove incomplete output. Success revokes the camera's temporary grants and
retains the owned file until explicit deletion. Dropping a Rust future does not
itself dismiss another application's UI; host/deadline cleanup still applies.
An external camera can remain on screen after the framework request expires.

Abrupt process death cannot run Java cleanup. CapturedMedia exposes the private
files for recovery/deletion on the next launch, including potentially incomplete
files. Validate these files before displaying/uploading them; do not assume a
recovered file represents a completed capture. At most 64 capture files may be
allocated before new capture requests return Busy. Picked external files do not
consume this limit, and ordinary selected URIs are not copied to private storage.

The provider exposes only strict UUID `.jpg`/`.mp4` capture names under its own
canonical directory. It rejects traversal and external insert/update/delete,
and cannot create a removed file through openFile after a cancellation race.
Neither broad storage permission nor an exported file provider is required.

## Verification

```bash
python3 scripts/android/counter.py build --app media-probe --abi x86_64
python3 scripts/android/media-probe.py --serial YOUR_TEST_DEVICE \
  --apk target/android/x86_64/debug/media-probe/taim-media-probe.apk \
  --output target/android/runtime-smoke/media-new
```

The API 35 x86_64/16 KiB guest passed 18 integration checks. These cover
image/video/mixed selection, metadata and bounded readback, denied persistence,
grant release, MIME/count rejection, cancellation, concurrent-request rejection,
JPEG capture, read-only native-viewer access, capture deletion/recovery listing,
video output protocol, path isolation, real system-picker cancellation and
activity teardown during an incomplete capture. The host suite passed 53 tests.
Targeted host and ARM64 strict Clippy passed. A fresh `--media` Gradle project
built ARM64/x86_64, and its debug APK signature/16 KiB ZIP alignment verified.
Its merged manifest contains the private grant-only provider with the resolved
application ID and no CAMERA or broad media/storage permission declaration.
The existing platform/notification fixture also passed its 14 checks plus one
cold-intent check against the updated service dispatcher. See the
[source/artifact acceptance manifest](media-acceptance-20261030.json).

The separate responder APK runs under a different UID and generates test media;
it receives actual Android URI grants and sends actual activity results. Its
photo is encoded JPEG, but its video bytes are a protocol fixture, not encoded
video. These checks do not certify physical camera hardware, video codecs,
OEM camera/viewer behavior or real photo-picker selection. The production host
contains no responder override. Physical device and older-API runtime acceptance
remain pending.
