//! Bluetooth Low Energy central role over the built-in `taim.bluetooth` channel.
//!
//! Permissions are the app's responsibility: declare and request BLUETOOTH_SCAN and
//! BLUETOOTH_CONNECT (Android 12+) or ACCESS_FINE_LOCATION (Android 11 and older).
//! GATT allows one outstanding operation per connection; concurrent calls return `Busy`.
use super::plugin_support::{decode, uuid};
use serde::Deserialize;
use serde_json::{Value, json};

/// Built-in plugin channel name, for filtering `PlatformEvent::Plugin`.
pub const BLUETOOTH_CHANNEL: &str = "taim.bluetooth";

/// Adapter and permission state, without prompting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BluetoothStatus {
    /// A Bluetooth adapter exists.
    pub supported: bool,
    /// The device declares Bluetooth Low Energy.
    pub le: bool,
    /// The radio is on.
    pub enabled: bool,
    /// Scanning permission is granted.
    pub scan_permission: bool,
    /// Connection permission is granted.
    pub connect_permission: bool,
}

/// A bounded foreground scan. The activity must stay resumed; pausing ends it with `Cancelled`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BleScanOptions {
    /// 500–30,000 ms.
    pub duration_ms: u32,
    /// Advertised service UUIDs to match (at most 16); empty matches every device.
    pub services: Vec<String>,
    /// Optional advertised-name prefix, 1–64 characters.
    pub name_prefix: Option<String>,
}
impl BleScanOptions {
    pub(super) fn wire(&self) -> Option<Value> {
        ((500..=30_000).contains(&self.duration_ms)
            && self.services.len() <= 16
            && self.services.iter().all(|s| uuid(s))
            && self
                .name_prefix
                .as_ref()
                .is_none_or(|p| (1..=64).contains(&p.chars().count())))
        .then(|| json!({"durationMs":self.duration_ms,"services":self.services,"namePrefix":self.name_prefix}))
    }
}

/// One advertising device, deduplicated by address (latest advertisement wins).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BleDevice {
    /// Hardware or resolvable private address, `AA:BB:CC:DD:EE:FF`.
    pub address: String,
    /// Advertised or cached name.
    pub name: Option<String>,
    /// Signal strength in dBm.
    pub rssi: i32,
    /// Advertised service UUIDs.
    pub services: Vec<String>,
    /// The advertisement accepts connections.
    pub connectable: bool,
}

/// GATT characteristic and its Android property bit mask (read=0x02, write=0x08, notify=0x10...).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GattCharacteristic {
    /// Characteristic UUID.
    pub uuid: String,
    /// `BluetoothGattCharacteristic.PROPERTY_*` bits.
    pub properties: u32,
}
/// A discovered primary service.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GattService {
    /// Service UUID.
    pub uuid: String,
    /// Its characteristics.
    pub characteristics: Vec<GattCharacteristic>,
}
/// An open GATT connection with discovered services.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BleConnection {
    /// Native connection handle; never reused within the activity.
    pub handle: u64,
    /// Services discovered after connecting.
    pub services: Vec<GattService>,
}

/// Unsolicited Bluetooth events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BluetoothEvent {
    /// First sighting of a device during an active scan.
    ScanResult(BleDevice),
    /// Characteristic notification or indication.
    Notification {
        /// Connection handle.
        handle: u64,
        /// Service UUID.
        service: String,
        /// Characteristic UUID.
        characteristic: String,
        /// Value bytes.
        value: Vec<u8>,
    },
    /// The connection closed; the handle is no longer valid.
    Disconnected {
        /// Connection handle.
        handle: u64,
        /// Android GATT status (0 is a normal close).
        status: i32,
    },
}
impl BluetoothEvent {
    /// Decode a `PlatformEvent::Plugin { channel, event }`, or None for other channels.
    pub fn decode(channel: &str, event: &Value) -> Option<Self> {
        if channel != BLUETOOTH_CHANNEL {
            return None;
        }
        Some(match event["type"].as_str()? {
            "scanResult" => Self::ScanResult(serde_json::from_value(event["device"].clone()).ok()?),
            "notify" => Self::Notification {
                handle: event["handle"].as_u64()?,
                service: event["service"].as_str()?.to_owned(),
                characteristic: event["characteristic"].as_str()?.to_owned(),
                value: decode(&event["value"])?,
            },
            "disconnected" => Self::Disconnected {
                handle: event["handle"].as_u64()?,
                status: event["status"].as_i64()? as i32,
            },
            _ => return None,
        })
    }
}

fn address(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 17
        && b.iter().enumerate().all(|(i, c)| {
            if i % 3 == 2 {
                *c == b':'
            } else {
                c.is_ascii_hexdigit() && !c.is_ascii_lowercase()
            }
        })
}

#[cfg(target_os = "android")]
mod client {
    use super::super::plugin_support::{encode, parse};
    use super::super::{ServiceError, Services};
    use super::*;
    const C: &str = BLUETOOTH_CHANNEL;

    impl Services {
        /// Adapter, radio and permission state.
        pub async fn bluetooth_status(&self) -> Result<BluetoothStatus, ServiceError> {
            parse(self.plugin_call(C, "status", json!({})).await?).ok_or(ServiceError::Failed)
        }
        /// Open the system Bluetooth settings; never enables the radio itself.
        pub async fn bluetooth_settings(&self) -> Result<(), ServiceError> {
            self.plugin_call(C, "settings", json!({})).await.map(drop)
        }
        /// Scan for the full duration (or until `ble_stop_scan`) and return every matching device.
        /// New devices are also streamed as `BluetoothEvent::ScanResult` while scanning.
        pub async fn ble_scan(
            &self,
            options: &BleScanOptions,
        ) -> Result<Vec<BleDevice>, ServiceError> {
            let args = options.wire().ok_or(ServiceError::Invalid)?;
            parse(self.plugin_call(C, "scan", args).await?).ok_or(ServiceError::Failed)
        }
        /// End an active scan early; its `ble_scan` call returns the devices seen so far.
        pub async fn ble_stop_scan(&self) -> Result<(), ServiceError> {
            self.plugin_call(C, "stopScan", json!({})).await.map(drop)
        }
        /// Connect and discover services. `timeout_ms` is 1,000–60,000.
        pub async fn ble_connect(
            &self,
            address: &str,
            timeout_ms: u32,
        ) -> Result<BleConnection, ServiceError> {
            if !super::address(address) || !(1000..=60_000).contains(&timeout_ms) {
                return Err(ServiceError::Invalid);
            }
            parse(
                self.plugin_call(
                    C,
                    "connect",
                    json!({"address":address,"timeoutMs":timeout_ms}),
                )
                .await?,
            )
            .ok_or(ServiceError::Failed)
        }
        /// Read a characteristic value.
        pub async fn ble_read(
            &self,
            handle: u64,
            service: &str,
            characteristic: &str,
        ) -> Result<Vec<u8>, ServiceError> {
            if !uuid(service) || !uuid(characteristic) {
                return Err(ServiceError::Invalid);
            }
            let value = self
                .plugin_call(
                    C,
                    "read",
                    json!({"handle":handle,"service":service,"characteristic":characteristic}),
                )
                .await?;
            decode(&value).ok_or(ServiceError::Failed)
        }
        /// Write at most 512 bytes. `with_response` waits for the peer's acknowledgement.
        pub async fn ble_write(
            &self,
            handle: u64,
            service: &str,
            characteristic: &str,
            value: &[u8],
            with_response: bool,
        ) -> Result<(), ServiceError> {
            if !uuid(service) || !uuid(characteristic) || value.len() > 512 {
                return Err(ServiceError::Invalid);
            }
            self.plugin_call(C, "write", json!({"handle":handle,"service":service,"characteristic":characteristic,"value":encode(value),"withResponse":with_response}))
                .await
                .map(drop)
        }
        /// Enable or disable notifications/indications (writes the CCCD descriptor).
        pub async fn ble_subscribe(
            &self,
            handle: u64,
            service: &str,
            characteristic: &str,
            enable: bool,
        ) -> Result<(), ServiceError> {
            if !uuid(service) || !uuid(characteristic) {
                return Err(ServiceError::Invalid);
            }
            self.plugin_call(C, "subscribe", json!({"handle":handle,"service":service,"characteristic":characteristic,"enable":enable}))
                .await
                .map(drop)
        }
        /// Close a connection, idempotently.
        pub async fn ble_disconnect(&self, handle: u64) -> Result<(), ServiceError> {
            self.plugin_call(C, "disconnect", json!({"handle":handle}))
                .await
                .map(drop)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scan_options_are_bounded() {
        let mut options = BleScanOptions {
            duration_ms: 5000,
            services: vec![],
            name_prefix: None,
        };
        assert!(options.wire().is_some());
        options.duration_ms = 499;
        assert!(options.wire().is_none());
        options.duration_ms = 5000;
        options.services = vec!["not-a-uuid".into()];
        assert!(options.wire().is_none());
        options.services = vec!["0000180d-0000-1000-8000-00805f9b34fb".into()];
        options.name_prefix = Some(String::new());
        assert!(options.wire().is_none());
    }
    #[test]
    fn addresses_require_canonical_uppercase_form() {
        assert!(address("AA:BB:CC:DD:EE:0F"));
        assert!(!address("aa:bb:cc:dd:ee:0f"));
        assert!(!address("AA-BB-CC-DD-EE-0F"));
    }
    #[test]
    fn events_decode_and_fail_closed() {
        let notify =
            json!({"type":"notify","handle":3,"service":"s","characteristic":"c","value":"AQI="});
        assert_eq!(
            BluetoothEvent::decode(BLUETOOTH_CHANNEL, &notify),
            Some(BluetoothEvent::Notification {
                handle: 3,
                service: "s".into(),
                characteristic: "c".into(),
                value: vec![1, 2]
            })
        );
        assert!(BluetoothEvent::decode("other", &notify).is_none());
        assert!(
            BluetoothEvent::decode(BLUETOOTH_CHANNEL, &json!({"type":"notify","handle":3}))
                .is_none()
        );
        let device = json!({"type":"scanResult","device":{"address":"AA:BB:CC:DD:EE:FF","name":null,"rssi":-60,"services":[],"connectable":true}});
        assert!(matches!(
            BluetoothEvent::decode(BLUETOOTH_CHANNEL, &device),
            Some(BluetoothEvent::ScanResult(_))
        ));
    }
}
