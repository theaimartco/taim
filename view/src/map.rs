//! Google Maps slot (`taim.map`, provided by the project.py `--maps` extension).
use crate::{AndroidView, NativeViewError, android_view};
use serde_json::{Value, json};

/// Base map style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapType {
    /// Road map.
    #[default]
    Normal,
    /// Satellite imagery.
    Satellite,
    /// Terrain.
    Terrain,
    /// Satellite with roads.
    Hybrid,
}

/// A marker; `id` (1–64 `[A-Za-z0-9._-]`) identifies it in `markerClick` events.
#[derive(Debug, Clone, PartialEq)]
pub struct MapMarker {
    /// Stable id.
    pub id: String,
    /// Degrees.
    pub latitude: f64,
    /// Degrees.
    pub longitude: f64,
    /// Info-window title.
    pub title: Option<String>,
}

/// Reactive map state. The camera moves only when `camera_token` changes, so user panning is kept.
#[derive(Debug, Clone, PartialEq)]
pub struct AndroidMap {
    /// Camera target latitude.
    pub latitude: f64,
    /// Camera target longitude.
    pub longitude: f64,
    /// 1–21.
    pub zoom: f32,
    /// Increment to move the camera to latitude/longitude/zoom.
    pub camera_token: u64,
    /// Base style.
    pub map_type: MapType,
    /// Static bitmap rendering (no gestures); always composes correctly.
    pub lite: bool,
    /// Show the user's location dot (requires granted location permission).
    pub my_location: bool,
    /// At most 500 markers.
    pub markers: Vec<MapMarker>,
}
impl AndroidMap {
    /// Map centred on a point.
    pub fn new(latitude: f64, longitude: f64, zoom: f32) -> Self {
        Self {
            latitude,
            longitude,
            zoom,
            camera_token: 0,
            map_type: MapType::Normal,
            lite: false,
            my_location: false,
            markers: Vec::new(),
        }
    }
    /// Native properties, or null for invalid input (the slot fails closed).
    pub fn properties(&self) -> Value {
        if !(-90.0..=90.0).contains(&self.latitude)
            || !(-180.0..=180.0).contains(&self.longitude)
            || !(1.0..=21.0).contains(&self.zoom)
            || self.markers.len() > 500
        {
            return Value::Null;
        }
        let markers: Vec<Value> = self
            .markers
            .iter()
            .map(|m| json!({"id":m.id,"latitude":m.latitude,"longitude":m.longitude,"title":m.title}))
            .collect();
        json!({
            "latitude": self.latitude, "longitude": self.longitude, "zoom": self.zoom,
            "cameraToken": self.camera_token, "lite": self.lite, "myLocation": self.my_location,
            "mapType": match self.map_type { MapType::Normal => "normal", MapType::Satellite => "satellite", MapType::Terrain => "terrain", MapType::Hybrid => "hybrid" },
            "markers": markers,
        })
    }
}

/// Google Maps slot. Events: `ready`, `cameraIdle` {latitude,longitude,zoom},
/// `mapClick` {latitude,longitude}, `markerClick` {id}. Requires the `--maps` extension and
/// TAIM_MAPS_API_KEY at build time. Note: `lite: false` composition must be verified per device.
/// Properties larger than 16 KiB (roughly 150+ markers) are rejected by the native slot.
pub fn android_map_view(
    map: impl Fn() -> AndroidMap + Send + 'static,
) -> Result<AndroidView, NativeViewError> {
    android_view("taim.map", move || map().properties())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn map_properties_validate() {
        assert_eq!(
            AndroidMap::new(28.6, 77.2, 12.0).properties()["mapType"],
            "normal"
        );
        assert!(AndroidMap::new(95.0, 77.2, 12.0).properties().is_null());
        assert!(AndroidMap::new(28.6, 77.2, 0.5).properties().is_null());
    }
}
