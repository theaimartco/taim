# Android WebView adapter

The built-in `taim.web` factory uses the retained native hierarchy slot. It
provides native WebView accessibility/input, rectangular clipping, modal hiding,
pause/resume and disposal. It does not use external GPU texture import.

```rust
use taim::prelude::*;
let document = android_web_view(AndroidWebContent::Html(
    "<html><body><h1>Help</h1><p>Offline documentation</p></body></html>".into(),
))?.height(320.0);
```

`AndroidWebContent::Https(url)` loads a remote HTTPS document. Generate the app
with `project.py --internet` or explicitly declare `android.permission.INTERNET`.
Inline HTML does not need that permission and blocks network loading. HTML is
limited to 8192 UTF-8 bytes, URLs to 4096 bytes, and the native slot's serialized
JSON properties to 16 KiB. Reactive/custom callers can use `android_view` with
factory `taim.web` and properties `{html: ..., javascript: false}` or
`{url: ..., javascript: false}`. Exactly one content field is required.

JavaScript is disabled by default. `android_web_view_with_script(content, true)`
explicitly enables it. No native JavaScript interface is installed. File/content
access, mixed-content loads, geolocation and DOM storage are disabled. Native
web permission requests are denied, TLS errors cancel, HTTP/client-certificate
authentication is cancelled, and downloads/external intents/file pickers have
no implicit handler. The adapter does not grant camera or microphone permission.

User navigation is restricted to HTTPS on the initial host and port; embedded
resource requests observed by `shouldInterceptRequest` use the same check.
This is **not a network sandbox**: Android WebView does not report every
redirect through that interception callback. Choose trusted HTTPS content and
enforce server-side resource/CSP policies when isolation is required. Cookies
use Android's application-wide WebView store; third-party cookies are disabled
for this view, but separate slots do not provide independent login sessions.
WebView updates and the system provider's security maintenance remain relevant.

Only payload-free lifecycle/status events are emitted through
`PlatformEvent::NativeView`: `started`, `finished`, `blocked`, `loadFailed`,
`httpFailed`, `tlsFailed`, and `rendererGone`. URLs, document text, titles,
credentials and provider exception strings are not included. The host drops
events from disposed/hidden/paused slots. Identical properties retain the page;
changed content or script policy starts a new load. Pause stops loading and
calls this view's `onPause`; it does not globally pause other WebViews' timers.
An interrupted load is not automatically replayed on resume.

A renderer-process failure destroys this WebView and emits `rendererGone`.
The application must remount a new slot to recover; credentials/forms/history
are not silently restored. The adapter does not implement OAuth browser flows,
popups, downloads, fullscreen media, custom resource routing or persistent
browser-state restoration. Those require explicit application policy and APIs.

Dedicated-emulator acceptance is opt-in via
`scripts/android/native-view-probe.py --web`: real accessible HTML, default
script/network/file restrictions, origin parsing, credential rejection,
idempotent retention, explicit scripting and disposal. Physical WebView/IME/AT
provider matrices and remote-server/TLS fault testing remain unverified.
