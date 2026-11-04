//! Foreground motion/environment sensor snapshots, without implicit permissions.
use serde_json::{Value, json};

/// Android sensor types exposed by the permission-free snapshot adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SensorKind {
    /// Acceleration including gravity, metres/second squared, device axes.
    Accelerometer = 1,
    /// Magnetic field, microteslas, device axes.
    MagneticField = 2,
    /// Angular velocity, radians/second, device axes.
    Gyroscope = 4,
    /// Ambient illuminance, lux.
    Light = 5,
    /// Atmospheric pressure, hPa.
    Pressure = 6,
    /// Proximity, centimetres; some devices expose only near/far.
    Proximity = 8,
    /// Gravity vector, metres/second squared.
    Gravity = 9,
    /// Acceleration with gravity removed, metres/second squared.
    LinearAcceleration = 10,
    /// Rotation vector; Android component order, not screen coordinates.
    RotationVector = 11,
    /// Relative humidity, percent.
    RelativeHumidity = 12,
    /// Ambient temperature, Celsius.
    AmbientTemperature = 13,
    /// Rotation vector without geomagnetic north.
    GameRotationVector = 15,
    /// Rotation vector using accelerometer and magnetometer.
    GeomagneticRotationVector = 20,
}
impl SensorKind {
    fn parse(value: u64) -> Option<Self> {
        Some(match value {
            1 => Self::Accelerometer,
            2 => Self::MagneticField,
            4 => Self::Gyroscope,
            5 => Self::Light,
            6 => Self::Pressure,
            8 => Self::Proximity,
            9 => Self::Gravity,
            10 => Self::LinearAcceleration,
            11 => Self::RotationVector,
            12 => Self::RelativeHumidity,
            13 => Self::AmbientTemperature,
            15 => Self::GameRotationVector,
            20 => Self::GeomagneticRotationVector,
            _ => return None,
        })
    }
}

/// One fresh sample with a 100–10,000 ms acquisition deadline.
#[derive(Debug, Clone, Copy)]
pub struct SensorOptions {
    /// Device sensor; absence returns ServiceError::Unsupported.
    pub sensor: SensorKind,
    /// Deadline in milliseconds. The activity must remain resumed.
    pub timeout_ms: u32,
}
impl SensorOptions {
    pub(super) fn wire(self) -> Option<Value> {
        (100..=10_000).contains(&self.timeout_ms).then(
            || json!({"op":"sensorRead","sensor":self.sensor as u8,"timeout":self.timeout_ms}),
        )
    }
}

/// Validated sensor result. Values use Android sensor coordinates and units.
#[derive(Debug, Clone, PartialEq)]
pub struct SensorReading {
    /// Sensor producing the sample.
    pub sensor: SensorKind,
    /// Monotonic Android elapsed-realtime timestamp, including sleep, in ns.
    pub timestamp_ns: u64,
    /// Android accuracy: -1 (no contact), 0 (unreliable), or 1–3.
    pub accuracy: i8,
    /// Sensor-specific components. Device/sensor calibration remains relevant.
    pub values: Vec<f32>,
}
impl SensorReading {
    /// Decode a Request::ReadSensor result, rejecting malformed native data.
    pub fn from_value(value: Value) -> Option<Self> {
        let sensor = SensorKind::parse(value.get("sensor")?.as_u64()?)?;
        let timestamp_ns = value.get("timestamp_ns")?.as_u64()?;
        let accuracy = value.get("accuracy")?.as_i64()?;
        let values: Vec<f32> = serde_json::from_value(value.get("values")?.clone()).ok()?;
        let components = match sensor {
            SensorKind::Accelerometer
            | SensorKind::MagneticField
            | SensorKind::Gyroscope
            | SensorKind::Gravity
            | SensorKind::LinearAcceleration => 3..=3,
            SensorKind::RotationVector
            | SensorKind::GameRotationVector
            | SensorKind::GeomagneticRotationVector => 3..=5,
            _ => 1..=1,
        };
        (timestamp_ns > 0
            && (-1..=3).contains(&accuracy)
            && components.contains(&values.len())
            && values.iter().all(|value| value.is_finite()))
        .then_some(Self {
            sensor,
            timestamp_ns,
            accuracy: accuracy as i8,
            values,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_deadlines_are_bounded() {
        for timeout_ms in [0, 99, 10_001, u32::MAX] {
            assert!(
                SensorOptions {
                    sensor: SensorKind::Accelerometer,
                    timeout_ms
                }
                .wire()
                .is_none()
            );
        }
        assert!(
            SensorOptions {
                sensor: SensorKind::Light,
                timeout_ms: 100
            }
            .wire()
            .is_some()
        );
    }
    #[test]
    fn readings_enforce_sensor_shape_accuracy_and_finite_components() {
        let sample = json!({"sensor":1,"timestamp_ns":123,"accuracy":3,"values":[0,9.8,0]});
        assert_eq!(
            SensorReading::from_value(sample.clone()).unwrap().sensor,
            SensorKind::Accelerometer
        );
        for (key, invalid) in [
            ("sensor", json!(21)),
            ("timestamp_ns", json!(0)),
            ("accuracy", json!(4)),
            ("values", json!([1, 2])),
            ("values", json!([1, 2, 1e100])),
        ] {
            let mut value = sample.clone();
            value[key] = invalid;
            assert!(SensorReading::from_value(value).is_none());
        }
    }
}
