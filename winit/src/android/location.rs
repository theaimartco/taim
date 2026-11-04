//! Foreground location policies and validated native samples.
use serde::Deserialize;
use serde_json::{Value, json};

/// Provider preference, not a guarantee of location precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationAccuracy {
    /// Prefer the network provider, falling back to GPS with precise permission.
    Balanced,
    /// Prefer GPS when precise permission is granted; otherwise use available network location.
    High,
}
impl LocationAccuracy {
    fn wire(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::High => "high",
        }
    }
}

/// A one-shot foreground fix, with a bounded cache age and acquisition deadline.
#[derive(Debug, Clone, Copy)]
pub struct LocationOptions {
    /// Provider preference; approximate permission remains respected.
    pub accuracy: LocationAccuracy,
    /// Accept cached fixes at most this old (0–86,400,000 ms). Zero requires a new sample.
    pub maximum_age_ms: u64,
    /// Acquisition deadline, 1–60,000 ms.
    pub timeout_ms: u32,
}
impl LocationOptions {
    pub(super) fn wire(self) -> Option<Value> {
        (self.maximum_age_ms<=86_400_000 && (1..=60_000).contains(&self.timeout_ms))
            .then(||json!({"op":"locationCurrent","accuracy":self.accuracy.wire(),"maximumAge":self.maximum_age_ms,"timeout":self.timeout_ms}))
    }
}

/// A foreground update session. It always expires, even if the caller forgets to stop it.
#[derive(Debug, Clone, Copy)]
pub struct LocationWatchOptions {
    /// Provider preference, subject to available permissions/providers.
    pub accuracy: LocationAccuracy,
    /// Minimum interval hint, 1,000–60,000 ms. OS delivery frequency is not guaranteed.
    pub interval_ms: u32,
    /// Minimum distance hint, finite and from 0 through 100,000 metres.
    pub minimum_distance_m: f32,
    /// Session lifetime, 1–60,000 ms; backgrounding stops it sooner.
    pub duration_ms: u32,
}
impl LocationWatchOptions {
    pub(super) fn wire(self) -> Option<Value> {
        ((1_000..=60_000).contains(&self.interval_ms)
            && (1..=60_000).contains(&self.duration_ms)
            && self.minimum_distance_m.is_finite()
            && (0.0..=100_000.0).contains(&self.minimum_distance_m))
        .then(|| {
            json!({"op":"locationWatch","accuracy":self.accuracy.wire(),"interval":self.interval_ms,
                "distance":self.minimum_distance_m,"duration":self.duration_ms})
        })
    }
}

/// A validated Android location sample. Permission precision is distinct from measured accuracy.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LocationFix {
    /// Latitude in degrees, -90 through 90.
    pub latitude: f64,
    /// Longitude in degrees, -180 through 180.
    pub longitude: f64,
    /// Estimated horizontal accuracy in metres, when supplied by Android.
    pub accuracy_m: Option<f64>,
    /// Altitude in metres, when supplied by Android.
    pub altitude_m: Option<f64>,
    /// Speed in metres/second, when supplied by Android.
    pub speed_mps: Option<f64>,
    /// Bearing in degrees, when supplied by Android.
    pub bearing_deg: Option<f64>,
    /// Provider sample time in Unix milliseconds.
    pub time_ms: u64,
    /// Provider sample time on Android's monotonic clock, in nanoseconds.
    pub elapsed_realtime_ns: u64,
    /// Sample age at native serialization time, in milliseconds.
    pub age_ms: u64,
    /// Android provider name.
    pub provider: String,
    /// Android marked this fix as originating from a mock provider.
    pub mocked: bool,
    /// The app held precise permission at delivery; this does not guarantee an accurate fix.
    pub precise_permission: bool,
}
impl LocationFix {
    /// Decode and validate a raw CurrentLocation/LastLocation or location-event value.
    pub fn from_value(value: Value) -> Option<Self> {
        let fix: Self = serde_json::from_value(value).ok()?;
        (fix.latitude.is_finite()
            && (-90.0..=90.0).contains(&fix.latitude)
            && fix.longitude.is_finite()
            && (-180.0..=180.0).contains(&fix.longitude)
            && fix.accuracy_m.is_none_or(|n| n.is_finite() && n >= 0.0)
            && fix.altitude_m.is_none_or(f64::is_finite)
            && fix.speed_mps.is_none_or(|n| n.is_finite() && n >= 0.0)
            && fix
                .bearing_deg
                .is_none_or(|n| n.is_finite() && (0.0..=360.0).contains(&n))
            && fix.elapsed_realtime_ns > 0
            && !fix.provider.is_empty()
            && fix.provider.len() <= 64)
            .then_some(fix)
    }
}

/// Why a location watch ended. No watch is resumed automatically after backgrounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationEnd {
    /// Explicit stop or subscription drop.
    Stopped,
    /// Activity paused.
    Background,
    /// Configured session lifetime elapsed.
    Expired,
    /// Selected provider or system location was disabled.
    Disabled,
    /// Foreground location permission was lost.
    Denied,
    /// Host/event channel closed or was saturated.
    Unavailable,
    /// Native provider failed.
    Failed,
}
#[cfg(target_os = "android")]
impl LocationEnd {
    pub(super) fn parse(reason: &str) -> Self {
        match reason {
            "stopped" => Self::Stopped,
            "background" => Self::Background,
            "expired" => Self::Expired,
            "disabled" => Self::Disabled,
            "denied" => Self::Denied,
            "unavailable" => Self::Unavailable,
            _ => Self::Failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requests_bound_deadlines_cache_age_and_sampling_hints() {
        for timeout_ms in [0, 60_001] {
            assert!(
                LocationOptions {
                    accuracy: LocationAccuracy::High,
                    maximum_age_ms: 0,
                    timeout_ms
                }
                .wire()
                .is_none()
            );
        }
        assert!(
            LocationOptions {
                accuracy: LocationAccuracy::Balanced,
                maximum_age_ms: 86_400_001,
                timeout_ms: 1
            }
            .wire()
            .is_none()
        );
        for distance in [f32::NAN, f32::INFINITY, -1.0, 100_001.0] {
            assert!(
                LocationWatchOptions {
                    accuracy: LocationAccuracy::High,
                    interval_ms: 1000,
                    minimum_distance_m: distance,
                    duration_ms: 1000
                }
                .wire()
                .is_none()
            );
        }
        assert_eq!(
            LocationOptions {
                accuracy: LocationAccuracy::Balanced,
                maximum_age_ms: 0,
                timeout_ms: 1000
            }
            .wire()
            .unwrap()["maximumAge"],
            0
        );
    }
    #[test]
    fn fixes_reject_invalid_coordinates_and_negative_accuracy() {
        let value = json!({"latitude":1.25,"longitude":2.5,"accuracy_m":3.0,"altitude_m":null,"speed_mps":null,"bearing_deg":null,"time_ms":10,"elapsed_realtime_ns":100,"age_ms":0,"provider":"gps","mocked":true,"precise_permission":true});
        assert!(LocationFix::from_value(value.clone()).unwrap().mocked);
        for (field, invalid) in [
            ("latitude", json!(91)),
            ("longitude", json!(-181)),
            ("accuracy_m", json!(-1)),
            ("speed_mps", json!(-1)),
            ("elapsed_realtime_ns", json!(0)),
            ("provider", json!("")),
        ] {
            let mut value = value.clone();
            value[field] = invalid;
            assert!(LocationFix::from_value(value).is_none());
        }
    }
}
