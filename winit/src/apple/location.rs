//! Foreground location through Core Location.
//!
//! One `CLLocationManager` lives on the main thread for the process. The
//! app's Info.plist must carry `NSLocationWhenInUseUsageDescription`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::channel::{mpsc, oneshot};
use futures::{Stream, StreamExt as _};
use objc2::encode::{Encode, Encoding};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{ClassType, DeclaredClass, declare_class, msg_send, msg_send_id, mutability};
use objc2_foundation::MainThreadMarker;

use super::ffi;
use super::types::{
    LocationFix, LocationOptions, LocationPermission, LocationWatchOptions, ServiceError,
};

#[link(name = "CoreLocation", kind = "framework")]
unsafe extern "C" {
    static kCLErrorDomain: *const AnyObject;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Coordinate {
    latitude: f64,
    longitude: f64,
}

// SAFETY: matches `CLLocationCoordinate2D`, two doubles.
unsafe impl Encode for Coordinate {
    const ENCODING: Encoding =
        Encoding::Struct("CLLocationCoordinate2D", &[f64::ENCODING, f64::ENCODING]);
}

/// Reads a `CLLocation`.
///
/// # Safety
/// `location` must point to a `CLLocation`.
unsafe fn fix(location: *const AnyObject) -> Option<LocationFix> {
    if location.is_null() {
        return None;
    }
    // SAFETY: CLLocation's documented properties.
    unsafe {
        let coordinate: Coordinate = msg_send![location, coordinate];
        let horizontal: f64 = msg_send![location, horizontalAccuracy];
        let altitude: f64 = msg_send![location, altitude];
        let vertical: f64 = msg_send![location, verticalAccuracy];
        let speed: f64 = msg_send![location, speed];
        let course: f64 = msg_send![location, course];
        let date: *const AnyObject = msg_send![location, timestamp];
        let timestamp: f64 = if date.is_null() {
            f64::NAN
        } else {
            msg_send![date, timeIntervalSince1970]
        };
        LocationFix::from_raw(
            coordinate.latitude,
            coordinate.longitude,
            horizontal,
            altitude,
            vertical,
            speed,
            course,
            timestamp,
        )
    }
}

/// Main-thread state shared with the manager's delegate.
struct Hub {
    manager: Retained<AnyObject>,
    _delegate: Retained<LocationDelegate>,
    permission_waiters: Vec<oneshot::Sender<LocationPermission>>,
    fix_waiters: HashMap<u64, oneshot::Sender<Result<LocationFix, ServiceError>>>,
    watchers: HashMap<
        u64,
        (
            LocationWatchOptions,
            mpsc::Sender<Result<LocationFix, ServiceError>>,
        ),
    >,
    next: u64,
}

thread_local! {
    static HUB: RefCell<Option<Hub>> = const { RefCell::new(None) };
}

impl Hub {
    fn permission(&self) -> LocationPermission {
        // SAFETY: `authorizationStatus` (iOS 14) on the main-thread manager.
        LocationPermission::from_raw(unsafe { msg_send![&self.manager, authorizationStatus] })
    }

    /// Starts or stops continuous updates to match the watchers, using the
    /// finest accuracy and smallest distance any of them asks for.
    fn reconfigure(&self) {
        // SAFETY: CLLocationManager configuration on the main thread.
        unsafe {
            if self.watchers.is_empty() {
                let _: () = msg_send![&self.manager, stopUpdatingLocation];
                return;
            }
            let accuracy = self
                .watchers
                .values()
                .map(|(options, _)| options.accuracy.meters())
                .fold(f64::INFINITY, f64::min);
            let distance = self
                .watchers
                .values()
                .map(|(options, _)| options.distance_filter)
                .fold(f64::INFINITY, f64::min);
            let _: () = msg_send![&self.manager, setDesiredAccuracy: accuracy];
            // kCLDistanceFilterNone is -1.
            let distance = if distance <= 0.0 { -1.0 } else { distance };
            let _: () = msg_send![&self.manager, setDistanceFilter: distance];
            let _: () = msg_send![&self.manager, startUpdatingLocation];
        }
    }
}

/// Runs `body` with the hub, creating the manager on first use.
fn with_hub<T>(
    _mtm: MainThreadMarker,
    body: impl FnOnce(&mut Hub) -> T,
) -> Result<T, ServiceError> {
    HUB.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            // Authorization belongs to an app bundle; a bare executable can
            // never be granted location.
            if ffi::bundle_identifier().is_none() {
                return Err(ServiceError::Unavailable);
            }
            // SAFETY: referencing a framework constant keeps CoreLocation linked.
            let _ = unsafe { kCLErrorDomain };
            let class = ffi::class("CLLocationManager").ok_or(ServiceError::Unsupported)?;
            // SAFETY: created on the main thread, whose run loop delivers the
            // delegate callbacks; the delegate is retained by the hub because
            // the manager holds it weakly.
            unsafe {
                let manager: Retained<AnyObject> = msg_send_id![class, new];
                let delegate: Retained<LocationDelegate> =
                    msg_send_id![LocationDelegate::alloc(), init];
                let _: () = msg_send![&manager, setDelegate: &*delegate];
                *slot = Some(Hub {
                    manager,
                    _delegate: delegate,
                    permission_waiters: Vec::new(),
                    fix_waiters: HashMap::new(),
                    watchers: HashMap::new(),
                    next: 1,
                });
            }
        }
        let hub = slot.as_mut().ok_or(ServiceError::Failed)?;
        Ok(body(hub))
    })
}

/// Runs `body` with the hub from a delegate callback, if it exists and is
/// not already borrowed.
fn in_callback(body: impl FnOnce(&mut Hub)) {
    HUB.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut()
            && let Some(hub) = slot.as_mut()
        {
            body(hub);
        }
    });
}

declare_class!(
    struct LocationDelegate;

    // SAFETY: NSObject has no subclassing requirements; no state, no Drop.
    unsafe impl ClassType for LocationDelegate {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "TaimLocationDelegate";
    }

    impl DeclaredClass for LocationDelegate {}

    unsafe impl LocationDelegate {
        #[method(locationManagerDidChangeAuthorization:)]
        fn did_change_authorization(&self, _manager: *mut AnyObject) {
            in_callback(|hub| {
                let permission = hub.permission();
                if permission != LocationPermission::NotDetermined {
                    for waiter in hub.permission_waiters.drain(..) {
                        let _ = waiter.send(permission);
                    }
                }
                if !permission.allows() {
                    for (_, waiter) in hub.fix_waiters.drain() {
                        let _ = waiter.send(Err(ServiceError::Denied));
                    }
                    for (_, (_, mut watcher)) in hub.watchers.drain() {
                        let _ = watcher.try_send(Err(ServiceError::Denied));
                    }
                    hub.reconfigure();
                }
            });
        }

        #[method(locationManager:didUpdateLocations:)]
        fn did_update(&self, _manager: *mut AnyObject, locations: *mut AnyObject) {
            // SAFETY: Core Location passes a non-empty NSArray of CLLocation.
            let latest = unsafe {
                let last: *const AnyObject = msg_send![locations, lastObject];
                fix(last)
            };
            let Some(latest) = latest else {
                return;
            };
            in_callback(|hub| {
                for (_, waiter) in hub.fix_waiters.drain() {
                    let _ = waiter.send(Ok(latest));
                }
                hub.watchers
                    .retain(|_, (_, watcher)| match watcher.try_send(Ok(latest)) {
                        Ok(()) => true,
                        // A slow reader skips samples; a closed one leaves.
                        Err(error) => error.is_full(),
                    });
            });
        }

        #[method(locationManager:didFailWithError:)]
        fn did_fail(&self, _manager: *mut AnyObject, error: *mut AnyObject) {
            // SAFETY: Core Location passes an NSError.
            let code = unsafe { ffi::error_code(error) }.unwrap_or(-1);
            in_callback(|hub| {
                // kCLErrorLocationUnknown (0) is transient: keep waiting.
                if code == 0 {
                    return;
                }
                let failure = if code == 1 {
                    ServiceError::Denied
                } else {
                    ServiceError::Failed
                };
                for (_, waiter) in hub.fix_waiters.drain() {
                    let _ = waiter.send(Err(failure));
                }
                if code == 1 {
                    for (_, (_, mut watcher)) in hub.watchers.drain() {
                        let _ = watcher.try_send(Err(failure));
                    }
                    hub.reconfigure();
                }
            });
        }
    }
);

/// Whether location services are on for the device at all.
fn services_enabled() -> bool {
    let Some(class) = ffi::class("CLLocationManager") else {
        return false;
    };
    // SAFETY: a class method Apple asks not to call on the main thread; the
    // services call it from the calling task's thread.
    unsafe { msg_send![class, locationServicesEnabled] }
}

/// The app's location permission, without asking.
pub async fn location_permission() -> Result<LocationPermission, ServiceError> {
    let (sender, receiver) = oneshot::channel();
    ffi::on_main(move |mtm| {
        let _ = sender.send(with_hub(mtm, |hub| hub.permission()));
    });
    receiver.await.map_err(|_| ServiceError::Cancelled)?
}

/// Asks for when-in-use permission if the user has not been asked yet, and
/// returns the resulting permission.
pub async fn request_location_permission() -> Result<LocationPermission, ServiceError> {
    let (sender, receiver) = oneshot::channel();
    ffi::on_main(move |mtm| {
        let outcome = with_hub(mtm, |hub| {
            let permission = hub.permission();
            if permission == LocationPermission::NotDetermined {
                hub.permission_waiters.push(sender);
                // SAFETY: on the main thread; the answer arrives through
                // `locationManagerDidChangeAuthorization:`.
                unsafe {
                    let _: () = msg_send![&hub.manager, requestWhenInUseAuthorization];
                }
            } else {
                let _ = sender.send(permission);
            }
        });
        if let Err(error) = outcome {
            log::warn!("location permission request failed: {error}");
        }
    });
    receiver.await.map_err(|_| ServiceError::Unsupported)
}

/// One foreground location. Uses a cached reading no older than
/// `options.maximum_age`, otherwise waits for a fresh one until
/// `options.timeout`. Needs permission: [`ServiceError::Denied`] otherwise.
pub async fn current_location(options: LocationOptions) -> Result<LocationFix, ServiceError> {
    options.validate()?;
    if !services_enabled() {
        return Err(ServiceError::Disabled);
    }
    let (sender, receiver) = oneshot::channel();
    ffi::on_main(move |mtm| {
        let outcome = with_hub(mtm, |hub| {
            if !hub.permission().allows() {
                let _ = sender.send(Err(ServiceError::Denied));
                return;
            }
            // SAFETY: `location` is the manager's last reading or nil.
            let cached = unsafe {
                let last: *const AnyObject = msg_send![&hub.manager, location];
                fix(last)
            };
            if let Some(cached) = cached.filter(|cached| {
                !options.maximum_age.is_zero()
                    && cached.age() <= options.maximum_age
                    && (options.accuracy.meters() < 0.0
                        || cached.accuracy <= options.accuracy.meters())
            }) {
                let _ = sender.send(Ok(cached));
                return;
            }
            let id = hub.next;
            hub.next += 1;
            let _ = hub.fix_waiters.insert(id, sender);
            // While continuous updates run, the next update answers;
            // `requestLocation` would interfere with them.
            if hub.watchers.is_empty() {
                // SAFETY: one-shot request on the main-thread manager.
                unsafe {
                    let _: () =
                        msg_send![&hub.manager, setDesiredAccuracy: options.accuracy.meters()];
                    let _: () = msg_send![&hub.manager, requestLocation];
                }
            }
            ffi::after(options.timeout, move |mtm| {
                let _ = with_hub(mtm, |hub| {
                    if let Some(waiter) = hub.fix_waiters.remove(&id) {
                        let _ = waiter.send(Err(ServiceError::Timeout));
                    }
                });
            });
        });
        if let Err(error) = outcome {
            log::warn!("location request failed: {error}");
        }
    });
    receiver.await.map_err(|_| ServiceError::Unsupported)?
}

/// Continuous foreground updates. Ends with an error item if permission is
/// withdrawn. Dropping it stops the updates.
pub struct LocationWatch {
    id: u64,
    updates: mpsc::Receiver<Result<LocationFix, ServiceError>>,
}

impl Stream for LocationWatch {
    type Item = Result<LocationFix, ServiceError>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.updates.poll_next_unpin(context)
    }
}

impl Drop for LocationWatch {
    fn drop(&mut self) {
        let id = self.id;
        ffi::on_main(move |mtm| {
            let _ = with_hub(mtm, |hub| {
                if hub.watchers.remove(&id).is_some() {
                    hub.reconfigure();
                }
            });
        });
    }
}

/// Starts continuous updates. Needs permission.
pub async fn watch_location(options: LocationWatchOptions) -> Result<LocationWatch, ServiceError> {
    options.validate()?;
    if !services_enabled() {
        return Err(ServiceError::Disabled);
    }
    let (sender, receiver) = oneshot::channel();
    ffi::on_main(move |mtm| {
        let started = with_hub(mtm, |hub| {
            if !hub.permission().allows() {
                return Err(ServiceError::Denied);
            }
            let id = hub.next;
            hub.next += 1;
            let (updates_sender, updates) = mpsc::channel(32);
            let _ = hub.watchers.insert(id, (options, updates_sender));
            hub.reconfigure();
            Ok(LocationWatch { id, updates })
        });
        let _ = sender.send(started.and_then(|started| started));
    });
    receiver.await.map_err(|_| ServiceError::Unsupported)?
}
