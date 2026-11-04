//! Shared helpers for built-in plugin channels (taim.bluetooth, taim.nfc, ...).
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Canonical 128-bit UUID text: 8-4-4-4-12 hexadecimal digits.
pub(super) fn uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, b)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                *b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}

/// Stable identifier: 1–64 ASCII letters, digits, dot, dash or underscore.
pub(super) fn identifier(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(super) fn encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

pub(super) fn decode(value: &Value) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(value.as_str()?)
        .ok()
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(super) fn parse<T: DeserializeOwned>(value: Value) -> Option<T> {
    serde_json::from_value(value).ok()
}

#[cfg(target_os = "android")]
impl super::Services {
    pub(super) async fn plugin_call(
        &self,
        channel: &str,
        method: &str,
        arguments: Value,
    ) -> Result<Value, super::ServiceError> {
        self.request(super::Request::Plugin {
            channel: channel.to_owned(),
            method: method.to_owned(),
            arguments,
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn uuid_and_identifier_validation() {
        assert!(super::uuid("0000180d-0000-1000-8000-00805f9b34fb"));
        assert!(!super::uuid("0000180d00001000800000805f9b34fb"));
        assert!(!super::uuid("0000180d-0000-1000-8000-00805f9b34fg"));
        assert!(super::identifier("home.office-1"));
        assert!(!super::identifier(""));
        assert!(!super::identifier("a/b"));
        assert!(!super::identifier(&"x".repeat(65)));
    }
}
