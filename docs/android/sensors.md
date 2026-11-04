# Foreground sensor snapshots

`Request::SensorInventory` returns the available default permission-free motion
and environment sensor descriptors, at most 13. Hardware availability varies.
`Request::ReadSensor(SensorOptions { sensor, timeout_ms })` acquires one fresh
sample while the activity is resumed. Decode the JSON result using
`SensorReading::from_value`; it checks the sensor kind, component count, finite
values, accuracy range and nonzero timestamp.

```rust
use taim::android::{Request, SensorKind, SensorOptions, SensorReading};
let value = client.request(Request::ReadSensor(SensorOptions {
    sensor: SensorKind::Accelerometer,
    timeout_ms: 2_000,
})).await?;
let sample = SensorReading::from_value(value);
```

`Services::read_sensor(options)` performs decoding and additionally checks that
the returned sensor matches the requested kind, returning a typed reading.

Supported kinds are accelerometer, magnetic field, gyroscope, light, pressure,
proximity, gravity, linear acceleration, rotation vector, relative humidity,
ambient temperature, game rotation vector and geomagnetic rotation vector.
Units and device-coordinate conventions are documented on `SensorKind`.
Rotation vectors are not automatically converted to display orientation.
The timestamp is Android monotonic elapsed realtime in nanoseconds, including
sleep; it is not Unix time. Accuracy is supplied by Android, not a calibration
guarantee. Device spoofing/mock-emulator data is not a security signal.

The requested sampling period is 20,000 microseconds (50 Hz); it is an OS hint,
not a promised event frequency. Each listener is removed after its first fresh
sample, deadline, activity pause or host close. Deadlines are 100–10,000 ms and
at most eight requests can be active. Dropping a pending Rust request posts
native cancellation; the bounded deadline/lifecycle also removes the listener.
There is no implicit prompt or background sensing, and no step/body/heart
rate/high-sampling-rate permission is requested. On-change sensors may time out
if they have no fresh event; cached older timestamps are not substituted.

Unsupported hardware returns `ServiceError::Unsupported`; invalid requests,
background cancellation, timeout and platform failures retain their existing
typed service errors. Inventory is metadata only and does not register sensors.
This implementation provides snapshots; continuous streams, sensor fusion,
geofencing and background sensing remain separate capabilities.

The dedicated-emulator native acceptance adds `--sensors` to test real
accelerometer delivery, listener cleanup, concurrency limits, cancellation,
invalid kinds/deadlines, the timeout handler and rejection after actual activity
backgrounding. The timeout-handler test invokes the registered deadline on the
main thread; it is not a wall-clock timeout-precision measurement. Physical
sensor calibration, OEM accuracy and power characteristics remain unverified.
