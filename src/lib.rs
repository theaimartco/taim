//! # TAIM
//!
//! TAIM is a Rust GUI framework built around views that own their state.
//! There is no message type and no update function: a view reads state
//! handles, and changing a handle repaints exactly the views that read it.
//!
//! ```no_run
//! use taim::prelude::*;
//!
//! fn main() -> taim::Result {
//!     taim::launch("Counter", |cx| {
//!         let count = cx.signal(0);
//!
//!         column(views![
//!             text(move || format!("Count: {}", count.get())).size(24),
//!             row(views![
//!                 button("-1").on_press(move || count.update(|n| *n -= 1)),
//!                 button("+1").on_press(move || count.update(|n| *n += 1)),
//!             ])
//!             .gap(8.0),
//!         ])
//!         .gap(12.0)
//!         .padding(24.0)
//!     })
//! }
//! ```
//!
//! The [`view`] module holds the view API and every built-in view; the
//! [`prelude`] imports what an application needs. The runtime owns frame
//! scheduling, accessibility and renderer recovery.
//!
//! TAIM is under active development. Consult `docs/status.md` before
//! depending on a platform or performance property.
#![doc(html_logo_url = "https://raw.githubusercontent.com/theaimartco/taim/main/docs/logo-256.png")]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(all(
    target_family = "unix",
    not(target_os = "macos"),
    not(target_os = "ios"),
    not(target_os = "android"),
    not(feature = "wayland"),
    not(feature = "x11"),
))]
compile_error!("No Unix display backend is enabled. Enable x11 or wayland.");

// Taim's Android host (`TaimActivity`, winit/android) is a NativeActivity, which
// taim_winit always enables on Android; GameActivity cannot drive it.
#[cfg(all(target_os = "android", feature = "android-game-activity"))]
compile_error!(
    "Taim's Android host is a NativeActivity; `android-game-activity` is not supported."
);

use taim_core as core;

mod error;
mod launch;

pub mod storage;
pub mod touch;
pub mod window;

/// What an application asks of the operating system: open a link, share
/// text, vibrate, and where to keep its files.
pub mod platform {
    pub use taim_winit::platform::*;
}

pub use error::Error;
pub use launch::{App, app, launch, prelude};

/// The TAIM view API: views, state handles and their contracts.
pub use taim_view as view;

pub use crate::core::{
    Background, Border, Color, Degrees, Font, Gradient, Padding, Pixels, Point, Radians, Rectangle,
    Settings, Shadow, Size, Theme, Transformation, Vector, alignment, border, color, font,
    gradient, keyboard, mouse, padding, semantics, theme, time,
};
pub use taim_signal::{Memo, Signal};

/// The renderer views draw with, chosen when TAIM is compiled.
pub type Renderer = taim_renderer::Renderer;

/// Renderer backend selection and preferences.
pub mod backend {
    pub use taim_core::backend::*;
}

pub use backend::{Backend, PowerPreference};

/// Fine-grained reactive state primitives.
pub mod signal {
    pub use taim_signal::*;
}

/// The VYOM backend for TAIM applications.
#[cfg(feature = "vyom")]
pub mod platform_vyom {
    pub use taim_platform_vyom::*;
}

#[cfg(feature = "highlighter")]
pub use taim_highlighter as highlighter;

#[cfg(feature = "wgpu-bare")]
pub use taim_renderer::wgpu::wgpu;

/// What running an application returns.
pub type Result = std::result::Result<(), Error>;

/// Apple platform services (iOS and macOS): haptics, Face ID and Touch ID,
/// the keychain, local notifications, location, the photo picker and the
/// camera.
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub mod apple {
    pub use taim_winit::apple::*;
}

/// Android application host types, shared with Taim's window shell.
#[cfg(target_os = "android")]
pub mod android {
    pub use taim_winit::android::{
        BILLING_CHANNEL, PUSH_CHANNEL, Price, PricingPhase, Product, ProductKind, Purchase,
        PurchaseState, PurchaseUpdate, PushEvent, PushMessage, PushNotification, SubscriptionOffer,
    };
    pub use taim_winit::android::{
        BLUETOOTH_CHANNEL, BleConnection, BleDevice, BleScanOptions, BluetoothEvent,
        BluetoothStatus, FOREGROUND_CHANNEL, ForegroundService, ForegroundServiceType,
        GEOFENCE_CHANNEL, GattCharacteristic, GattService, Geofence, GeofenceStatus,
        GeofenceTransition, NFC_CHANNEL, NdefContent, NdefRecord, NfcStatus, NfcTag,
    };
    pub use taim_winit::android::{
        BiometricAuthenticator, BiometricAvailability, BiometricKey, BiometricKeyKind,
        BiometricPolicy, BiometricPrompt, BiometricStatus, BiometricStrength, SealedData,
    };
    pub use taim_winit::android::{
        Haptic, LocationAccuracy, LocationEnd, LocationFix, LocationOptions, LocationSubscription,
        LocationWatchOptions, MAX_TRANSFER, MediaCapture, MediaKind, MediaPicker, Notification,
        NotificationChannel, NotificationImportance, NotificationTap, NotificationVisibility,
        PermissionState, PermissionStatus, PlatformEvent, Request, ServiceError, Services,
        SpecialAccess, WorkBackoff, WorkConstraints, WorkContext, WorkError, WorkInput,
        WorkNetwork, WorkPolicy, WorkResult, WorkSchedule, WorkSpec, WorkState, services,
    };
    pub use taim_winit::android::{SensorKind, SensorOptions, SensorReading};
    pub use taim_winit::android_background_worker as background_worker;
    pub use taim_winit::winit::platform::android::activity::AndroidApp;
}

/// Compiles every Rust example in the getting-started guide, so the guide
/// cannot drift from the API.
#[cfg(doctest)]
#[doc = include_str!("../docs/guide/getting-started.md")]
pub struct GettingStarted;
