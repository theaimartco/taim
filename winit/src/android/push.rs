//! Firebase Cloud Messaging over the `taim.fcm` extension (project.py `--fcm google-services.json`).
//!
//! Message content is untrusted input from your server/FCM. Request POST_NOTIFICATIONS on
//! Android 13+ before relying on system-displayed notification messages.
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// Extension channel name, for filtering `PlatformEvent::Plugin`.
pub const PUSH_CHANNEL: &str = "taim.fcm";

/// Title/body of a notification message.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PushNotification {
    /// Title, if any.
    pub title: Option<String>,
    /// Body, if any.
    pub body: Option<String>,
}

/// A received FCM message.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushMessage {
    /// FCM message id.
    pub message_id: Option<String>,
    /// Sender id or topic.
    pub from: Option<String>,
    /// Server send time, ms since the Unix epoch.
    pub sent_time_ms: u64,
    /// Data payload (string key/values, at most 4 KiB total).
    pub data: BTreeMap<String, String>,
    /// Present for notification messages received while the app was in the foreground.
    pub notification: Option<PushNotification>,
}

/// Live push events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushEvent {
    /// A message arrived (also persisted; drain with `push_pending`).
    Message(PushMessage),
    /// The registration token rotated; send it to your server.
    Token(String),
}
impl PushEvent {
    /// Decode a `PlatformEvent::Plugin { channel, event }`, or None for other channels.
    pub fn decode(channel: &str, event: &Value) -> Option<Self> {
        if channel != PUSH_CHANNEL {
            return None;
        }
        match event["type"].as_str()? {
            "message" => serde_json::from_value(event["message"].clone())
                .ok()
                .map(Self::Message),
            "token" => event["token"].as_str().map(|t| Self::Token(t.to_owned())),
            _ => None,
        }
    }
}

fn topic(value: &str) -> bool {
    (1..=900).contains(&value.len())
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.~%".contains(&c))
}

#[cfg(target_os = "android")]
mod client {
    use super::super::plugin_support::parse;
    use super::super::{ServiceError, Services};
    use super::*;
    use serde_json::json;
    const C: &str = PUSH_CHANNEL;
    impl Services {
        /// Current registration token. `Unsupported` if the FCM extension is not configured.
        pub async fn push_token(&self) -> Result<String, ServiceError> {
            self.plugin_call(C, "token", json!({}))
                .await?
                .as_str()
                .map(str::to_owned)
                .ok_or(ServiceError::Failed)
        }
        /// Invalidate the token (e.g. on sign-out); a new one is issued later.
        pub async fn push_delete_token(&self) -> Result<(), ServiceError> {
            self.plugin_call(C, "deleteToken", json!({}))
                .await
                .map(drop)
        }
        /// Subscribe to a topic (`[a-zA-Z0-9-_.~%]{1,900}`).
        pub async fn push_subscribe(&self, name: &str) -> Result<(), ServiceError> {
            if !topic(name) {
                return Err(ServiceError::Invalid);
            }
            self.plugin_call(C, "subscribe", json!({"topic":name}))
                .await
                .map(drop)
        }
        /// Unsubscribe from a topic.
        pub async fn push_unsubscribe(&self, name: &str) -> Result<(), ServiceError> {
            if !topic(name) {
                return Err(ServiceError::Invalid);
            }
            self.plugin_call(C, "unsubscribe", json!({"topic":name}))
                .await
                .map(drop)
        }
        /// Enable/disable automatic token generation (for consent flows).
        pub async fn push_auto_init(&self, enabled: bool) -> Result<(), ServiceError> {
            self.plugin_call(C, "autoInit", json!({"enabled":enabled}))
                .await
                .map(drop)
        }
        /// Drain messages persisted since the last call (latest 32).
        pub async fn push_pending(&self) -> Result<Vec<PushMessage>, ServiceError> {
            parse(self.plugin_call(C, "pending", json!({})).await?).ok_or(ServiceError::Failed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn events_decode() {
        let m = json!({"type":"message","message":{"messageId":"1","from":"/topics/news","sentTimeMs":5,"data":{"k":"v"},"notification":null}});
        let PushEvent::Message(message) = PushEvent::decode(PUSH_CHANNEL, &m).unwrap() else {
            panic!()
        };
        assert_eq!(message.data["k"], "v");
        assert_eq!(
            PushEvent::decode(PUSH_CHANNEL, &json!({"type":"token","token":"t"})),
            Some(PushEvent::Token("t".into()))
        );
        assert!(PushEvent::decode("x", &m).is_none());
        assert!(topic("news-in") && !topic("a b") && !topic(""));
    }
}
