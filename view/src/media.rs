//! Android inline video playback and camera preview slots (`taim.video`, `taim.camera`).
use crate::{AndroidView, NativeViewError, android_view};
use serde_json::{Value, json};

/// Video source for [`android_video_view`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoSource {
    /// HTTPS URL, at most 4096 bytes. Requires the INTERNET permission.
    Https(String),
    /// A granted `content://` URI, e.g. from the media picker.
    Content(String),
    /// A file packaged in the APK's assets directory (stored uncompressed).
    Asset(String),
}

/// How the frame fills the slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoFit {
    /// Letterbox: the whole frame is visible.
    #[default]
    Contain,
    /// Crop: the slot is filled.
    Cover,
}

/// Reactive playback state. Change `seek_token` to seek to `seek_ms`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AndroidVideo {
    /// Media to play; changing it reopens the player.
    pub source: VideoSource,
    /// Play while the activity is resumed.
    pub playing: bool,
    /// Restart at the end.
    pub looping: bool,
    /// Silence audio.
    pub muted: bool,
    /// Frame fit.
    pub fit: VideoFit,
    /// Seek target in ms, applied when `seek_token` changes.
    pub seek_ms: u64,
    /// Increment to request a seek.
    pub seek_token: u64,
}
impl AndroidVideo {
    /// Paused, unmuted, non-looping playback of `source`.
    pub fn new(source: VideoSource) -> Self {
        Self {
            source,
            playing: false,
            looping: false,
            muted: false,
            fit: VideoFit::Contain,
            seek_ms: 0,
            seek_token: 0,
        }
    }
    /// Native properties, or null (the slot fails closed) for an invalid source.
    pub fn properties(&self) -> Value {
        let uri = match &self.source {
            VideoSource::Https(url)
                if url.len() <= 4096 && url.to_ascii_lowercase().starts_with("https://") =>
            {
                url.clone()
            }
            VideoSource::Content(uri) if uri.len() <= 4096 && uri.starts_with("content://") => {
                uri.clone()
            }
            VideoSource::Asset(path)
                if !path.is_empty()
                    && path.len() <= 1024
                    && !path.contains("..")
                    && !path.starts_with('/') =>
            {
                format!("asset:///{path}")
            }
            _ => return Value::Null,
        };
        json!({
            "uri": uri, "playing": self.playing, "looping": self.looping, "muted": self.muted,
            "fit": match self.fit { VideoFit::Contain => "contain", VideoFit::Cover => "cover" },
            "seekMs": self.seek_ms, "seekToken": self.seek_token,
        })
    }
}

/// Inline video slot. `video` is re-evaluated reactively, so read signals inside it.
///
/// Events (filter `PlatformEvent::NativeView` by [`AndroidView::id`]): `prepared`
/// {durationMs,width,height}, `position` {positionMs,playing} every 500 ms while playing,
/// `buffering` {active}, `completed`, `error`. Playback pauses with the activity.
pub fn android_video_view(
    video: impl Fn() -> AndroidVideo + Send + 'static,
) -> Result<AndroidView, NativeViewError> {
    android_view("taim.video", move || video().properties())
}

/// Camera for [`android_camera_preview`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CameraLens {
    /// Rear camera.
    #[default]
    Back,
    /// Selfie camera.
    Front,
}

/// Inline Camera2 preview. Request the CAMERA permission first; without it the slot emits
/// `error {reason:"denied"}`. The camera opens only while the activity is resumed and `active`
/// is true, and is released on pause. Events: `opened` {width,height,lens}, `closed`,
/// `error` {reason: denied|unsupported|inUse|disabled|disconnected|failed}.
/// Preview only: capture photos/videos with `Request::CaptureMedia`.
pub fn android_camera_preview(
    lens_and_active: impl Fn() -> (CameraLens, bool) + Send + 'static,
) -> Result<AndroidView, NativeViewError> {
    android_view("taim.camera", move || {
        let (lens, active) = lens_and_active();
        json!({"lens": match lens { CameraLens::Back => "back", CameraLens::Front => "front" }, "active": active})
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn video_sources_fail_closed() {
        assert_eq!(
            AndroidVideo::new(VideoSource::Https("https://x/v.mp4".into())).properties()["uri"],
            "https://x/v.mp4"
        );
        assert!(
            AndroidVideo::new(VideoSource::Https("http://x/v.mp4".into()))
                .properties()
                .is_null()
        );
        assert!(
            AndroidVideo::new(VideoSource::Asset("../secret".into()))
                .properties()
                .is_null()
        );
        assert_eq!(
            AndroidVideo::new(VideoSource::Asset("intro.mp4".into())).properties()["uri"],
            "asset:///intro.mp4"
        );
        assert!(
            AndroidVideo::new(VideoSource::Content("file:///sdcard/x".into()))
                .properties()
                .is_null()
        );
    }
}
