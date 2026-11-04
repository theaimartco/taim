//! User-mediated Android media selection and capture descriptions.
use serde_json::{Value, json};

/// Media kinds accepted by the system photo picker (SAF fallback before API 33).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    /// Images only.
    Images,
    /// Videos only.
    Videos,
    /// Images and videos.
    ImagesAndVideos,
}

/// A user-mediated media selection. No broad storage permission is requested.
#[derive(Debug, Clone)]
pub struct MediaPicker {
    /// Permitted content category.
    pub kind: MediaKind,
    /// Maximum selection count, from 1 through 32. The OS may impose a lower limit.
    pub maximum: u8,
}

/// Capture delegated to an installed camera application, with an app-private output URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaCapture {
    /// Full-resolution image output; the camera controls encoding, commonly JPEG.
    Photo,
    /// Video output. Limits are camera hints; returned file size/duration is not guaranteed.
    Video {
        /// Requested duration limit, 1–60 seconds (within the service's 120s deadline).
        duration_seconds: u8,
        /// Ask the camera for high rather than low quality.
        high_quality: bool,
    },
}

impl MediaPicker {
    pub(super) fn wire(self) -> Option<Value> {
        (1..=32).contains(&self.maximum).then(|| json!({"op":"mediaPick",
            "kind":match self.kind {MediaKind::Images=>"image",MediaKind::Videos=>"video",MediaKind::ImagesAndVideos=>"visual"},
            "maximum":self.maximum}))
    }
}
impl MediaCapture {
    pub(super) fn wire(self) -> Option<Value> {
        match self {
            Self::Photo => Some(json!({"op":"mediaCapture","kind":"image"})),
            Self::Video {duration_seconds,high_quality} => (1..=60).contains(&duration_seconds)
                .then(||json!({"op":"mediaCapture","kind":"video","duration":duration_seconds,"highQuality":high_quality})),
        }
    }
}

pub(super) fn content_uri(uri: &str) -> bool {
    uri.len() <= 8192
        && uri.starts_with("content://")
        && uri[10..]
            .split('/')
            .next()
            .is_some_and(|authority| !authority.is_empty())
        && !uri.chars().any(char::is_control)
        && !uri.contains('#')
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn picker_bounds_and_categories_are_enforced() {
        for maximum in [0, 33, 255] {
            assert!(
                MediaPicker {
                    kind: MediaKind::Images,
                    maximum
                }
                .wire()
                .is_none()
            );
        }
        let wire = MediaPicker {
            kind: MediaKind::ImagesAndVideos,
            maximum: 32,
        }
        .wire()
        .unwrap();
        assert_eq!(wire["kind"], "visual");
        assert_eq!(wire["maximum"], 32);
    }
    #[test]
    fn capture_requires_a_duration_inside_the_request_deadline() {
        for duration_seconds in [0, 61, 255] {
            assert!(
                MediaCapture::Video {
                    duration_seconds,
                    high_quality: true
                }
                .wire()
                .is_none()
            );
        }
        assert_eq!(MediaCapture::Photo.wire().unwrap()["kind"], "image");
        assert_eq!(
            MediaCapture::Video {
                duration_seconds: 60,
                high_quality: false
            }
            .wire()
            .unwrap()["duration"],
            60
        );
    }
    #[test]
    fn media_requests_do_not_accept_file_paths_or_empty_authorities() {
        for uri in [
            "file:///private/file",
            "content:///empty",
            "content://",
            "content://a/p#fragment",
            "content://a/\n",
        ] {
            assert!(!content_uri(uri));
        }
        assert!(content_uri("content://example.provider/document/42"));
    }
}
