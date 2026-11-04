//! Android-owned activity callbacks and input connection transport.
#[cfg(target_os = "android")]
mod bridge;
pub(crate) mod geometry;
#[cfg(target_os = "android")]
pub(crate) use bridge::Bridge;
#[cfg(target_os = "android")]
pub(crate) use semantics::SemanticCommand;

mod billing;
mod biometric;
pub use billing::{
    BILLING_CHANNEL, Price, PricingPhase, Product, ProductKind, Purchase, PurchaseState,
    PurchaseUpdate, SubscriptionOffer,
};
mod bluetooth;
pub use bluetooth::{
    BLUETOOTH_CHANNEL, BleConnection, BleDevice, BleScanOptions, BluetoothEvent, BluetoothStatus,
    GattCharacteristic, GattService,
};
mod foreground;
pub use foreground::{FOREGROUND_CHANNEL, ForegroundService, ForegroundServiceType};
mod geofence;
pub use geofence::{GEOFENCE_CHANNEL, Geofence, GeofenceStatus, GeofenceTransition};
mod nfc;
pub use nfc::{NFC_CHANNEL, NdefContent, NdefRecord, NfcStatus, NfcTag};
pub(crate) mod keys;
mod plugin_support;
mod push;
pub use biometric::{
    BiometricAuthenticator, BiometricAvailability, BiometricKey, BiometricKeyKind, BiometricPolicy,
    BiometricPrompt, BiometricStatus, BiometricStrength, SealedData,
};
pub use push::{PUSH_CHANNEL, PushEvent, PushMessage, PushNotification};
mod location;
mod sensors;
pub use sensors::{SensorKind, SensorOptions, SensorReading};
pub(crate) mod native_views;
mod work;
pub use work::{
    WorkBackoff, WorkConstraints, WorkContext, WorkError, WorkInput, WorkNetwork, WorkPolicy,
    WorkResult, WorkSchedule, WorkSpec, WorkState,
};
#[cfg(target_os = "android")]
#[doc(hidden)]
#[path = "work_bridge.rs"]
pub mod __work_bridge;
mod media;
pub use location::{
    LocationAccuracy, LocationEnd, LocationFix, LocationOptions, LocationWatchOptions,
};
mod notifications;
mod permissions;
pub use media::{MediaCapture, MediaKind, MediaPicker};
pub use permissions::{PermissionState, PermissionStatus, SpecialAccess};
pub(crate) mod semantics;
pub use notifications::{
    Notification, NotificationChannel, NotificationImportance, NotificationTap,
    NotificationVisibility,
};

#[cfg(target_os = "android")]
mod services;
#[cfg(target_os = "android")]
pub use services::{
    Haptic, LocationSubscription, MAX_TRANSFER, PlatformEvent, Request, ServiceError, Services,
    services,
};

#[cfg(all(target_os = "android", feature = "measurement"))]
pub(crate) use bridge::FrameTrace;
