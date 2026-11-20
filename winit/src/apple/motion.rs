//! The device's tilt, for leaning the scene light on glass.
//!
//! Core Motion runs only while glass is on screen and the app is in front,
//! and is stopped otherwise: the sensors cost power. The light follows
//! changes of tilt, not the angle the phone is held at: a reference eases
//! towards the current attitude over about a second and a half, so holding
//! the phone still at any angle brings the light back to rest.

use std::cell::RefCell;

use objc2::msg_send;
use objc2::msg_send_id;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;

#[link(name = "CoreMotion", kind = "framework")]
unsafe extern "C" {}

struct Tilt {
    manager: Retained<AnyObject>,
    /// The attitude the light rests at: `(roll, pitch)` in radians.
    reference: Option<[f64; 2]>,
    last: std::time::Instant,
}

thread_local! {
    static TILT: RefCell<Option<Tilt>> = const { RefCell::new(None) };
}

/// Starts reading the device's attitude, if it has the sensors. Main thread.
pub(crate) fn start() {
    TILT.with(|tilt| {
        let mut tilt = tilt.borrow_mut();
        if tilt.is_some() {
            return;
        }
        let Some(class) = super::ffi::class("CMMotionManager") else {
            return;
        };
        // SAFETY: a Core Motion manager created, configured and started on
        // the main thread, and kept until `stop`.
        unsafe {
            let manager: Retained<AnyObject> = msg_send_id![msg_send_id![class, alloc], init];
            let available: bool = msg_send![&manager, isDeviceMotionAvailable];
            if !available {
                return;
            }
            let _: () = msg_send![&manager, setDeviceMotionUpdateInterval: 1.0f64 / 60.0];
            let _: () = msg_send![&manager, startDeviceMotionUpdates];
            *tilt = Some(Tilt {
                manager,
                reference: None,
                last: std::time::Instant::now(),
            });
        }
    });
}

/// Stops the sensors.
pub(crate) fn stop() {
    TILT.with(|tilt| {
        if let Some(tilt) = tilt.borrow_mut().take() {
            // SAFETY: the manager started in `start`, on the main thread.
            unsafe {
                let _: () = msg_send![&tilt.manager, stopDeviceMotionUpdates];
            }
        }
    });
}

/// Whether the sensors are running.
pub(crate) fn is_running() -> bool {
    TILT.with(|tilt| tilt.borrow().is_some())
}

/// How far the device has tilted from where it rests, each axis
/// `-1.0..=1.0`; `None` before the first reading.
pub(crate) fn lean() -> Option<[f32; 2]> {
    TILT.with(|tilt| {
        let mut tilt = tilt.borrow_mut();
        let tilt = tilt.as_mut()?;
        // SAFETY: the manager's latest reading and its attitude, read on the
        // main thread; both may be nil before the first update.
        let (roll, pitch) = unsafe {
            let motion: *const AnyObject = msg_send![&tilt.manager, deviceMotion];
            if motion.is_null() {
                return None;
            }
            let attitude: *const AnyObject = msg_send![motion, attitude];
            if attitude.is_null() {
                return None;
            }
            let roll: f64 = msg_send![attitude, roll];
            let pitch: f64 = msg_send![attitude, pitch];
            (roll, pitch)
        };

        let now = std::time::Instant::now();
        let seconds = now.duration_since(tilt.last).as_secs_f64().min(0.5);
        tilt.last = now;

        let reference = tilt.reference.get_or_insert([roll, pitch]);
        let ease = 1.0 - (-seconds / 1.5).exp();
        reference[0] += (roll - reference[0]) * ease;
        reference[1] += (pitch - reference[1]) * ease;

        Some(lean_for(roll - reference[0], pitch - reference[1]))
    })
}

/// About 20 degrees of tilt leans the light all the way.
fn lean_for(roll: f64, pitch: f64) -> [f32; 2] {
    let full = 20.0_f64.to_radians();
    [
        (roll / full).clamp(-1.0, 1.0) as f32,
        (-pitch / full).clamp(-1.0, 1.0) as f32,
    ]
}

#[cfg(test)]
mod tests {
    use super::lean_for;

    #[test]
    fn a_small_tilt_leans_the_light_part_way_and_a_large_one_all_the_way() {
        let [x, y] = lean_for(10.0_f64.to_radians(), 0.0);
        assert!((x - 0.5).abs() < 1.0e-4 && y == 0.0);
        assert_eq!(lean_for(1.0, -1.0), [1.0, 1.0]);
    }
}
