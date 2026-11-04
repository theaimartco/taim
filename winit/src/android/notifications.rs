//! Local notification descriptions shared by the Rust client and native host.
use serde_json::{Value, json};

/// Channel interruption level. Android users retain control after creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationImportance {
    /// Quiet notification with minimal prominence.
    Min,
    /// Quiet notification visible in the notification shade.
    Low,
    /// Standard notification, subject to system/user settings.
    Default,
    /// May interrupt with a heads-up presentation, subject to system/user settings.
    High,
}

/// A persistent Android notification channel, available from API 26.
#[derive(Debug, Clone)]
pub struct NotificationChannel {
    /// App-local channel identifier; 1–128 ASCII letters, digits, dots, dashes or underscores.
    pub id: String,
    /// User-visible name, at most 256 UTF-8 bytes.
    pub name: String,
    /// User-visible description, at most 1024 UTF-8 bytes.
    pub description: String,
    /// Initial importance; recreating a channel cannot override user settings.
    pub importance: NotificationImportance,
}

/// Lock-screen disclosure policy. Private/secret notifications never include a public version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationVisibility {
    /// System may show the notification's full contents on the lock screen.
    Public,
    /// System redacts sensitive contents on the lock screen.
    #[default]
    Private,
    /// System hides the notification from the lock screen.
    Secret,
}

/// A local notification. Posting the same ID updates the app's existing notification.
#[derive(Debug, Clone)]
pub struct Notification {
    /// Nonnegative app-local notification identifier.
    pub id: i32,
    /// An existing notification channel identifier.
    pub channel: String,
    /// Plain-text title, at most 1024 UTF-8 bytes.
    pub title: String,
    /// Plain-text expandable body, at most 8192 UTF-8 bytes.
    pub body: String,
    /// Optional app drawable resource name; a system notification icon is the fallback.
    pub small_icon: Option<String>,
    /// Optional app routing data, at most 4096 UTF-8 bytes. Do not put secrets here.
    pub payload: Option<String>,
    /// Lock-screen policy.
    pub visibility: NotificationVisibility,
    /// Remove the notification after the user taps it.
    pub auto_cancel: bool,
    /// Avoid repeated alerts when this ID is updated.
    pub only_alert_once: bool,
    /// Optional system timeout in milliseconds, from 1 to seven days.
    pub timeout_ms: Option<u64>,
}

/// Tap metadata delivered in cold/warm launch intents. Treat incoming intents as untrusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationTap {
    /// App-local notification ID.
    pub id: i32,
    /// Optional routing data supplied when posting.
    pub payload: Option<String>,
}

pub(super) fn valid_channel(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}

impl NotificationChannel {
    pub(super) fn wire(self) -> Option<Value> {
        if !valid_channel(&self.id)
            || self.name.trim().is_empty()
            || self.name.len() > 256
            || self.description.len() > 1024
        {
            return None;
        }
        Some(
            json!({"op":"notificationChannel", "id":self.id, "name":self.name,
            "description":self.description, "importance":match self.importance {
                NotificationImportance::Min=>1, NotificationImportance::Low=>2,
                NotificationImportance::Default=>3, NotificationImportance::High=>4
            }}),
        )
    }
}

impl Notification {
    pub(super) fn wire(self) -> Option<Value> {
        if self.id < 0
            || !valid_channel(&self.channel)
            || self.title.len() > 1024
            || self.body.len() > 8192
            || self.payload.as_ref().is_some_and(|p| p.len() > 4096)
            || self.small_icon.as_ref().is_some_and(|name| {
                name.is_empty()
                    || name.len() > 128
                    || !name
                        .bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
            })
            || self
                .timeout_ms
                .is_some_and(|ms| ms == 0 || ms > 604_800_000)
        {
            return None;
        }
        Some(
            json!({"op":"notificationPost", "id":self.id, "channel":self.channel,
            "title":self.title,"body":self.body,"icon":self.small_icon,"payload":self.payload,
            "visibility":match self.visibility { NotificationVisibility::Public=>1,
                NotificationVisibility::Private=>0, NotificationVisibility::Secret=>-1 },
            "autoCancel":self.auto_cancel,"onlyAlertOnce":self.only_alert_once,"timeoutMs":self.timeout_ms}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn notification() -> Notification {
        Notification {
            id: 7,
            channel: "messages".into(),
            title: "नमस्ते".into(),
            body: "Hello 🌍".into(),
            small_icon: None,
            payload: Some("route:7".into()),
            visibility: NotificationVisibility::Private,
            auto_cancel: true,
            only_alert_once: true,
            timeout_ms: Some(1000),
        }
    }
    #[test]
    fn notification_wire_retains_routing_and_private_policy() {
        let value = notification().wire().unwrap();
        assert_eq!(value["visibility"], 0);
        assert_eq!(value["payload"], "route:7");
        assert_eq!(value["body"], "Hello 🌍");
        assert!(value["icon"].is_null());
    }
    #[test]
    fn notification_rejects_invalid_ids_resources_limits_and_timeouts() {
        let mut value = notification();
        value.id = -1;
        assert!(value.wire().is_none());
        let mut value = notification();
        value.channel = "bad/channel".into();
        assert!(value.wire().is_none());
        let mut value = notification();
        value.small_icon = Some("@android:drawable/x".into());
        assert!(value.wire().is_none());
        let mut value = notification();
        value.payload = Some("🌍".repeat(1025));
        assert!(value.wire().is_none());
        for ms in [0, 604_800_001] {
            let mut value = notification();
            value.timeout_ms = Some(ms);
            assert!(value.wire().is_none());
        }
    }
    #[test]
    fn channel_validation_preserves_user_visible_names() {
        let value = NotificationChannel {
            id: "messages.v1".into(),
            name: "Messages".into(),
            description: "App updates".into(),
            importance: NotificationImportance::Low,
        }
        .wire()
        .unwrap();
        assert_eq!(value["importance"], 2);
        for id in ["", "../invalid", "a/b", "通知"] {
            assert!(!valid_channel(id));
        }
    }
}
