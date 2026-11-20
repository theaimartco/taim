//! Apple platform services for iOS and macOS: haptics, Face ID and Touch
//! ID, the keychain, local notifications, location, the photo picker and the
//! camera.
//!
//! Every service is a plain function with the same signature on both
//! systems. Asynchronous ones complete through channels and may be awaited
//! from any TAIM task; UIKit and AppKit work is posted to the main thread for
//! you. Never block the main thread waiting on one: the answer needs that
//! thread.
//!
//! | Service | Functions | iOS | macOS | Info.plist key |
//! |---|---|---|---|---|
//! | Haptics | [`haptic`] | Taptic Engine | Force Touch trackpad, while touched | — |
//! | Share sheet | `taim::platform::share_text` | `UIActivityViewController` | `NSSharingServicePicker` | — |
//! | Biometrics | [`biometric_status`], [`authenticate`] | Face ID, Touch ID | Touch ID | `NSFaceIDUsageDescription` (iOS) |
//! | Keychain | [`keychain_set`], [`keychain_get`], [`keychain_delete`] | yes | yes (see below) | — |
//! | Notifications | [`notification_permission`], [`request_notification_permission`], [`schedule_notification`], [`cancel_notifications`], [`cancel_all_notifications`], [`set_badge`], [`notification_events`] | yes | bundled apps | — |
//! | Location | [`location_permission`], [`request_location_permission`], [`current_location`], [`watch_location`] | yes | bundled apps | `NSLocationWhenInUseUsageDescription` (macOS also `NSLocationUsageDescription`) |
//! | Media picker | [`pick_media`] | Photos picker | open panel | — |
//! | Camera | [`camera_permission`], [`request_camera_permission`], [`capture_media`] | yes | permission only; capture is [`ServiceError::Unsupported`] | `NSCameraUsageDescription` |
//!
//! iOS and macOS terminate an app that touches a protected service without
//! the matching Info.plist key; the iOS and macOS packagers add them. A
//! sandboxed Mac app also needs the matching entitlement (location, camera,
//! user-selected files). Notifications and location need an app bundle: an
//! executable started with `cargo run` gets [`ServiceError::Unavailable`].
//!
//! On macOS the keychain uses the data-protection keychain when the app is
//! signed with a keychain access group (App Store and developer-signed
//! builds), and otherwise the login keychain, where the prompting
//! [`KeychainAccess`] kinds are unavailable.

#![allow(unsafe_code)]

// Host builds compile the types for their tests; some native callers are iOS-only.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
mod types;
pub use types::{
    BiometricPolicy, BiometricStatus, Biometry, CaptureKind, Haptic, KeychainAccess,
    LocalNotification, LocationAccuracy, LocationFix, LocationOptions, LocationPermission,
    LocationWatchOptions, MediaFile, MediaKind, MediaPicker, NotificationEvent,
    NotificationOptions, PermissionState, ServiceError, limits,
};

#[cfg(any(target_os = "ios", target_os = "macos"))]
mod biometric;
#[cfg(any(target_os = "ios", target_os = "macos"))]
mod camera;
#[cfg(any(target_os = "ios", target_os = "macos"))]
mod ffi;
#[cfg(any(target_os = "ios", target_os = "macos"))]
mod keychain;
#[cfg(any(target_os = "ios", target_os = "macos"))]
mod location;
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) mod notifications;

// What differs: UIKit on iOS, AppKit on macOS.
#[cfg(target_os = "ios")]
mod haptics;
#[cfg(target_os = "macos")]
#[path = "haptics_macos.rs"]
mod haptics;
#[cfg(target_os = "ios")]
mod media;
#[cfg(target_os = "macos")]
#[path = "media_macos.rs"]
mod media;
#[cfg(target_os = "ios")]
pub(crate) mod motion;
#[cfg(target_os = "ios")]
pub(crate) mod window;
#[cfg(target_os = "macos")]
#[path = "window_macos.rs"]
pub(crate) mod window;

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) use haptics::share_text;
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use {
    biometric::{authenticate, biometric_status},
    camera::{camera_permission, request_camera_permission},
    haptics::haptic,
    keychain::{keychain_delete, keychain_get, keychain_set},
    location::{
        LocationWatch, current_location, location_permission, request_location_permission,
        watch_location,
    },
    media::{capture_media, pick_media},
    notifications::{
        cancel_all_notifications, cancel_notifications, notification_events,
        notification_permission, request_notification_permission, schedule_notification, set_badge,
    },
};
