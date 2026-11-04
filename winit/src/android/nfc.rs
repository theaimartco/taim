//! Foreground NFC reader mode and NDEF over the built-in `taim.nfc` channel.
//! Declare `android.permission.NFC` (a normal, install-time permission).
use super::plugin_support::decode;
use serde::Deserialize;
use serde_json::Value;

/// Built-in plugin channel name, for filtering `PlatformEvent::Plugin`.
pub const NFC_CHANNEL: &str = "taim.nfc";

/// Adapter state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct NfcStatus {
    /// The device has an NFC controller.
    pub supported: bool,
    /// NFC is switched on.
    pub enabled: bool,
    /// Reader mode is requested by this app (active while resumed).
    pub reading: bool,
}

/// One NDEF record. `tnf` is the 3-bit type name format (1 = well-known, 2 = MIME, 3 = URI...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NdefRecord {
    /// Type name format, 0–6.
    pub tnf: u8,
    /// Record type, e.g. `b"T"` or `b"U"` for well-known text/URI.
    pub record_type: Vec<u8>,
    /// Optional record identifier.
    pub id: Vec<u8>,
    /// Payload bytes.
    pub payload: Vec<u8>,
}
impl NdefRecord {
    /// Well-known URI record (no prefix abbreviation).
    pub fn uri(uri: &str) -> Self {
        let mut payload = vec![0];
        payload.extend_from_slice(uri.as_bytes());
        Self {
            tnf: 1,
            record_type: b"U".to_vec(),
            id: Vec::new(),
            payload,
        }
    }
    /// Well-known UTF-8 text record with an IANA language code such as "en".
    pub fn text(language: &str, text: &str) -> Self {
        let language = &language.as_bytes()[..language.len().min(63)];
        let mut payload = vec![language.len() as u8];
        payload.extend_from_slice(language);
        payload.extend_from_slice(text.as_bytes());
        Self {
            tnf: 1,
            record_type: b"T".to_vec(),
            id: Vec::new(),
            payload,
        }
    }
    fn from_value(value: &Value) -> Option<Self> {
        let tnf = value["tnf"].as_u64().filter(|t| *t <= 6)? as u8;
        Some(Self {
            tnf,
            record_type: decode(&value["type"])?,
            id: decode(&value["id"])?,
            payload: decode(&value["payload"])?,
        })
    }
}

/// NDEF content of a discovered tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NdefContent {
    /// Android NDEF type, e.g. `org.nfcforum.ndef.type2`.
    pub tag_type: String,
    /// Maximum message size in bytes.
    pub max_size: u32,
    /// The tag can be written.
    pub writable: bool,
    /// Records of the current message (empty for a blank tag).
    pub records: Vec<NdefRecord>,
    /// The message could not be read (tag moved away mid-read).
    pub read_failed: bool,
}

/// A tag seen in reader mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NfcTag {
    /// Serial for `nfc_write_ndef`; only the most recent tag can be written.
    pub serial: u64,
    /// Tag UID as lowercase hex (may be random per tap for some cards).
    pub id: String,
    /// Supported technologies, e.g. `NfcA`, `IsoDep`, `Ndef`.
    pub techs: Vec<String>,
    /// NDEF content, if the tag is NDEF formatted.
    pub ndef: Option<NdefContent>,
}
impl NfcTag {
    /// Decode a `PlatformEvent::Plugin { channel, event }`, or None for other channels.
    pub fn decode(channel: &str, event: &Value) -> Option<Self> {
        if channel != NFC_CHANNEL || event["type"] != "tag" {
            return None;
        }
        let ndef = match &event["ndef"] {
            Value::Null => None,
            n => Some(NdefContent {
                tag_type: n["type"].as_str()?.to_owned(),
                max_size: n["maxSize"].as_u64()? as u32,
                writable: n["writable"].as_bool()?,
                records: n["records"]
                    .as_array()?
                    .iter()
                    .map(NdefRecord::from_value)
                    .collect::<Option<_>>()?,
                read_failed: n.get("error").is_some(),
            }),
        };
        Some(Self {
            serial: event["tag"].as_u64()?,
            id: event["id"].as_str()?.to_owned(),
            techs: serde_json::from_value(event["techs"].clone()).ok()?,
            ndef,
        })
    }
}

#[cfg(target_os = "android")]
mod client {
    use super::super::plugin_support::{encode, parse};
    use super::super::{ServiceError, Services};
    use super::*;
    use serde_json::json;
    const C: &str = NFC_CHANNEL;

    impl Services {
        /// Controller state.
        pub async fn nfc_status(&self) -> Result<NfcStatus, ServiceError> {
            parse(self.plugin_call(C, "status", json!({})).await?).ok_or(ServiceError::Failed)
        }
        /// Open system NFC settings.
        pub async fn nfc_settings(&self) -> Result<(), ServiceError> {
            self.plugin_call(C, "settings", json!({})).await.map(drop)
        }
        /// Start reader mode; tags arrive as plugin events (decode with `NfcTag::decode`).
        /// Reader mode is suspended while paused and resumed automatically until `nfc_stop_reader`.
        pub async fn nfc_start_reader(&self) -> Result<(), ServiceError> {
            self.plugin_call(C, "startReader", json!({}))
                .await
                .map(drop)
        }
        /// Stop reader mode.
        pub async fn nfc_stop_reader(&self) -> Result<(), ServiceError> {
            self.plugin_call(C, "stopReader", json!({})).await.map(drop)
        }
        /// Write an NDEF message (1–32 records) to the most recent tag while it is still in range.
        pub async fn nfc_write_ndef(
            &self,
            tag_serial: u64,
            records: &[NdefRecord],
        ) -> Result<(), ServiceError> {
            if records.is_empty() || records.len() > 32 {
                return Err(ServiceError::Invalid);
            }
            let records: Vec<Value> = records
                .iter()
                .map(|r| json!({"tnf":r.tnf,"type":encode(&r.record_type),"id":encode(&r.id),"payload":encode(&r.payload)}))
                .collect();
            self.plugin_call(C, "writeNdef", json!({"tag":tag_serial,"records":records}))
                .await
                .map(drop)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn tag_events_decode() {
        let event = json!({"type":"tag","tag":2,"id":"04a1","techs":["NfcA","Ndef"],"ndef":{"type":"org.nfcforum.ndef.type2","maxSize":137,"writable":true,"records":[{"tnf":1,"type":"VQ==","id":"","payload":"AGh0dHBz"}]}});
        let tag = NfcTag::decode(NFC_CHANNEL, &event).unwrap();
        assert_eq!(tag.serial, 2);
        let ndef = tag.ndef.unwrap();
        assert_eq!(ndef.records[0].record_type, b"U");
        assert!(!ndef.read_failed);
        assert!(NfcTag::decode("other", &event).is_none());
        assert!(
            NfcTag::decode(
                NFC_CHANNEL,
                &json!({"type":"tag","tag":1,"id":"00","techs":["NfcA"],"ndef":null})
            )
            .unwrap()
            .ndef
            .is_none()
        );
    }
    #[test]
    fn record_builders() {
        assert_eq!(
            NdefRecord::text("en", "hi").payload,
            vec![2, b'e', b'n', b'h', b'i']
        );
        assert_eq!(NdefRecord::uri("https://x").payload[0], 0);
    }
}
