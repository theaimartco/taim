//! Circular geofences over the built-in `taim.geofence` channel (framework proximity alerts).
//!
//! Requires the `TaimGeofenceReceiver` manifest entry (project.py `--geofencing`) and
//! ACCESS_FINE_LOCATION; transitions while the app is not visible also require
//! ACCESS_BACKGROUND_LOCATION. Android clears regions on reboot/app update: call
//! `geofence_restore` at startup. Transitions are persisted and can be drained with
//! `geofence_pending`, so none are lost while the app process is not running.
use super::plugin_support::identifier;
use serde::Deserialize;
use serde_json::{Value, json};

/// Built-in plugin channel name, for filtering `PlatformEvent::Plugin`.
pub const GEOFENCE_CHANNEL: &str = "taim.geofence";

/// One circular region.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Geofence {
    /// 1–64 characters `[A-Za-z0-9._-]`; adding an existing id replaces it.
    #[serde(skip)]
    pub id: String,
    /// Degrees, −90–90.
    pub latitude: f64,
    /// Degrees, −180–180.
    pub longitude: f64,
    /// 50–100,000 m. Android's location accuracy makes smaller regions unreliable.
    pub radius_m: f32,
    /// Lifetime in ms (1 s – 365 days), or None for no expiry.
    #[serde(skip)]
    pub expiration_ms: Option<u64>,
}
impl Geofence {
    pub(super) fn wire(&self) -> Option<Value> {
        (identifier(&self.id)
            && (-90.0..=90.0).contains(&self.latitude)
            && (-180.0..=180.0).contains(&self.longitude)
            && (50.0..=100_000.0).contains(&self.radius_m)
            && self.expiration_ms.is_none_or(|e| (1000..=365 * 86_400_000).contains(&e)))
        .then(|| {
            json!({"id":self.id,"latitude":self.latitude,"longitude":self.longitude,"radiusM":self.radius_m,
                   "expirationMs":self.expiration_ms.map_or(-1_i64, |e| e as i64)})
        })
    }
}

/// Permission and provider state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeofenceStatus {
    /// ACCESS_FINE_LOCATION is granted.
    pub fine_permission: bool,
    /// Background delivery is permitted.
    pub background_permission: bool,
    /// Location is switched on.
    pub enabled: bool,
    /// Registered region count.
    pub regions: u32,
}

/// An entry or exit.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeofenceTransition {
    /// Region id.
    pub id: String,
    /// True on entry, false on exit.
    pub entering: bool,
    /// Wall-clock time the transition was received, ms since the Unix epoch.
    pub time_ms: u64,
}
impl GeofenceTransition {
    /// Decode a live `PlatformEvent::Plugin { channel, event }`.
    pub fn decode(channel: &str, event: &Value) -> Option<Self> {
        (channel == GEOFENCE_CHANNEL && event["type"] == "transition")
            .then(|| serde_json::from_value(event.clone()).ok())
            .flatten()
    }
}

#[cfg(target_os = "android")]
mod client {
    use super::super::plugin_support::parse;
    use super::super::{ServiceError, Services};
    use super::*;
    const C: &str = GEOFENCE_CHANNEL;

    impl Services {
        /// Permission/provider state and region count.
        pub async fn geofence_status(&self) -> Result<GeofenceStatus, ServiceError> {
            parse(self.plugin_call(C, "status", json!({})).await?).ok_or(ServiceError::Failed)
        }
        /// Add or replace a region (at most 100).
        pub async fn geofence_add(&self, region: &Geofence) -> Result<(), ServiceError> {
            self.plugin_call(C, "add", region.wire().ok_or(ServiceError::Invalid)?)
                .await
                .map(drop)
        }
        /// Remove a region, idempotently.
        pub async fn geofence_remove(&self, id: &str) -> Result<(), ServiceError> {
            if !identifier(id) {
                return Err(ServiceError::Invalid);
            }
            self.plugin_call(C, "remove", json!({"id":id}))
                .await
                .map(drop)
        }
        /// Persisted regions (including ones Android may have cleared since reboot).
        pub async fn geofence_list(&self) -> Result<Vec<Geofence>, ServiceError> {
            let value = self.plugin_call(C, "list", json!({})).await?;
            let map = value.as_object().ok_or(ServiceError::Failed)?;
            map.iter()
                .map(|(id, r)| {
                    let mut region: Geofence = parse(r.clone()).ok_or(ServiceError::Failed)?;
                    region.id = id.clone();
                    Ok(region)
                })
                .collect()
        }
        /// Re-register unexpired persisted regions; returns how many were restored.
        pub async fn geofence_restore(&self) -> Result<u32, ServiceError> {
            Ok(self
                .plugin_call(C, "restore", json!({}))
                .await?
                .as_u64()
                .ok_or(ServiceError::Failed)? as u32)
        }
        /// Drain transitions persisted since the last call (at most the latest 64).
        pub async fn geofence_pending(&self) -> Result<Vec<GeofenceTransition>, ServiceError> {
            parse(self.plugin_call(C, "pending", json!({})).await?).ok_or(ServiceError::Failed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn regions_are_validated() {
        let mut g = Geofence {
            id: "office".into(),
            latitude: 28.6,
            longitude: 77.2,
            radius_m: 100.0,
            expiration_ms: None,
        };
        assert_eq!(g.wire().unwrap()["expirationMs"], -1);
        g.radius_m = 10.0;
        assert!(g.wire().is_none());
        g.radius_m = 100.0;
        g.latitude = 91.0;
        assert!(g.wire().is_none());
        g.latitude = 0.0;
        g.id = "bad id".into();
        assert!(g.wire().is_none());
    }
    #[test]
    fn transitions_decode() {
        let e = json!({"type":"transition","id":"office","entering":true,"timeMs":5});
        assert_eq!(
            GeofenceTransition::decode(GEOFENCE_CHANNEL, &e).unwrap().id,
            "office"
        );
        assert!(GeofenceTransition::decode("x", &e).is_none());
    }
}
