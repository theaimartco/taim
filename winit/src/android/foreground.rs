//! Foreground service control over the built-in `taim.foreground` channel.
//!
//! Requires the `TaimForegroundService` manifest entry (project.py `--foreground-service TYPE`).
//! The service keeps the app process at foreground priority while the activity is in the
//! background, so already-running Rust tasks continue (e.g. a BLE sync, a recording, playback).
//! It does not execute Rust after the activity is destroyed; use durable background work for that.
//! Android 12+ forbids starting it while the app is in the background (`Denied`).
use serde_json::{Value, json};

/// Built-in plugin channel name.
pub const FOREGROUND_CHANNEL: &str = "taim.foreground";

/// Android foreground service type; it must also be declared in the manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForegroundServiceType {
    /// Upload/download/sync. Android 15 limits this to 6 hours per day.
    DataSync,
    /// Audio/video playback.
    MediaPlayback,
    /// Continuous location; requires location permission.
    Location,
    /// Bluetooth/USB/companion device connection.
    ConnectedDevice,
    /// Microphone capture.
    Microphone,
    /// Camera capture.
    Camera,
    /// Fitness/health tracking.
    Health,
    /// Messaging transfer.
    RemoteMessaging,
    /// Short critical task (about 3 minutes).
    ShortService,
    /// Anything else; requires Play Console justification.
    SpecialUse,
}
impl ForegroundServiceType {
    fn wire(self) -> &'static str {
        match self {
            Self::DataSync => "dataSync",
            Self::MediaPlayback => "mediaPlayback",
            Self::Location => "location",
            Self::ConnectedDevice => "connectedDevice",
            Self::Microphone => "microphone",
            Self::Camera => "camera",
            Self::Health => "health",
            Self::RemoteMessaging => "remoteMessaging",
            Self::ShortService => "shortService",
            Self::SpecialUse => "specialUse",
        }
    }
}

/// The ongoing notification shown while the service runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForegroundService {
    /// Existing notification channel id (create it with `Request::NotificationChannel`).
    pub channel: String,
    /// 1–256 characters.
    pub title: String,
    /// 1–1,024 characters.
    pub text: String,
    /// Service type.
    pub service_type: ForegroundServiceType,
}
impl ForegroundService {
    pub(super) fn wire(&self) -> Option<Value> {
        let len = |s: &str, max| (1..=max).contains(&s.encode_utf16().count());
        (super::notifications::valid_channel(&self.channel) && len(&self.title, 256) && len(&self.text, 1024))
            .then(|| json!({"channel":self.channel,"title":self.title,"text":self.text,"type":self.service_type.wire()}))
    }
}

#[cfg(target_os = "android")]
mod client {
    use super::super::{ServiceError, Services};
    use super::*;
    const C: &str = FOREGROUND_CHANNEL;
    impl Services {
        /// Start (or restart with new content) the foreground service while the app is visible.
        /// Android 13+ hides the notification unless POST_NOTIFICATIONS is granted; the service still runs.
        pub async fn foreground_start(
            &self,
            service: &ForegroundService,
        ) -> Result<(), ServiceError> {
            self.plugin_call(C, "start", service.wire().ok_or(ServiceError::Invalid)?)
                .await
                .map(drop)
        }
        /// Update the running service's notification; `NotFound` if not running.
        pub async fn foreground_update(
            &self,
            service: &ForegroundService,
        ) -> Result<(), ServiceError> {
            self.plugin_call(C, "update", service.wire().ok_or(ServiceError::Invalid)?)
                .await
                .map(drop)
        }
        /// Stop the service, idempotently.
        pub async fn foreground_stop(&self) -> Result<(), ServiceError> {
            self.plugin_call(C, "stop", json!({})).await.map(drop)
        }
        /// Whether the service is currently in the foreground state.
        pub async fn foreground_running(&self) -> Result<bool, ServiceError> {
            self.plugin_call(C, "status", json!({})).await?["running"]
                .as_bool()
                .ok_or(ServiceError::Failed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notification_content_is_bounded() {
        let mut s = ForegroundService {
            channel: "sync".into(),
            title: "Syncing".into(),
            text: "3 files".into(),
            service_type: ForegroundServiceType::DataSync,
        };
        assert_eq!(s.wire().unwrap()["type"], "dataSync");
        s.title.clear();
        assert!(s.wire().is_none());
    }
}
