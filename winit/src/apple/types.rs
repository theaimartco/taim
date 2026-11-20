//! The values iOS services take and return, with their validation and the
//! mapping from Apple's status codes. Kept free of Objective-C so the host
//! test suite checks them on every platform.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use taim_core::{Point, Rectangle, Size};

/// Why an iOS service request did not produce a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ServiceError {
    /// This device or iOS version has no such capability (no camera, no
    /// biometric sensor, an iOS release before the API existed).
    #[error("iOS capability unavailable on this device")]
    Unsupported,
    /// No window or view controller is available to present system UI.
    #[error("iOS application window unavailable")]
    Unavailable,
    /// The user refused, or a permission is denied or restricted.
    #[error("iOS operation denied")]
    Denied,
    /// The system cancelled the operation (another app came forward, the
    /// request was dropped).
    #[error("iOS operation cancelled")]
    Cancelled,
    /// Another picker or prompt of the same kind is already showing.
    #[error("iOS service busy")]
    Busy,
    /// Input failed validation or exceeded a documented limit.
    #[error("iOS request invalid or too large")]
    Invalid,
    /// No result arrived before the request's deadline.
    #[error("iOS request timed out")]
    Timeout,
    /// No biometric or passcode satisfying the policy is set up.
    #[error("iOS biometric or passcode not enrolled")]
    NotEnrolled,
    /// Too many failed biometric attempts; the passcode unlocks it again.
    #[error("iOS biometric authentication locked")]
    Lockout,
    /// Location services are switched off for the whole device.
    #[error("iOS capability disabled in Settings")]
    Disabled,
    /// The keychain is locked (the device has not been unlocked since boot,
    /// or is locked now for a when-unlocked item).
    #[error("iOS keychain locked")]
    Locked,
    /// The app lacks an entitlement the service requires.
    #[error("iOS entitlement missing")]
    MissingEntitlement,
    /// The operating system failed. Its error text is withheld because it
    /// can contain user data.
    #[error("iOS platform operation failed")]
    Failed,
}

/// Bounds shared by validation and the native layer.
pub mod limits {
    /// Longest keychain service or account name, in bytes.
    pub const KEYCHAIN_NAME: usize = 256;
    /// Largest secret one keychain item holds.
    pub const KEYCHAIN_SECRET: usize = 64 * 1024;
    /// Longest notification identifier, in bytes.
    pub const NOTIFICATION_ID: usize = 256;
    /// Longest notification title, subtitle or body, in bytes.
    pub const NOTIFICATION_TEXT: usize = 4 * 1024;
    /// Largest application payload carried by a notification, in bytes.
    pub const NOTIFICATION_PAYLOAD: usize = 4 * 1024;
    /// Furthest a local notification may be scheduled ahead.
    pub const NOTIFICATION_DELAY_SECS: u64 = 366 * 24 * 60 * 60;
    /// Longest biometric prompt reason, in bytes.
    pub const PROMPT_REASON: usize = 512;
    /// Most items one photo-library selection may return.
    pub const MEDIA_SELECTION: u16 = 50;
    /// Longest location request deadline.
    pub const LOCATION_TIMEOUT_SECS: u64 = 120;
    /// Oldest cached location a request may accept.
    pub const LOCATION_MAX_AGE_SECS: u64 = 24 * 60 * 60;
}

/// Tactile feedback through the Taptic Engine. It follows the user's
/// "System Haptics" setting; devices without the engine stay silent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Haptic {
    /// A light collision, for small controls snapping into place.
    Light,
    /// A medium collision.
    Medium,
    /// A heavy collision.
    Heavy,
    /// A soft, flexible collision.
    Soft,
    /// A hard, rigid collision.
    Rigid,
    /// A selection changed (picker wheels, segmented controls).
    Selection,
    /// A task succeeded.
    Success,
    /// A task produced a warning.
    Warning,
    /// A task failed.
    Error,
}

/// Which UIKit generator plays a [`Haptic`], and with which constant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HapticGenerator {
    /// `UIImpactFeedbackGenerator` with a `UIImpactFeedbackStyle`.
    Impact(isize),
    /// `UISelectionFeedbackGenerator`.
    Selection,
    /// `UINotificationFeedbackGenerator` with a `UINotificationFeedbackType`.
    Notification(isize),
}

impl Haptic {
    pub(crate) fn generator(self) -> HapticGenerator {
        match self {
            Self::Light => HapticGenerator::Impact(0),
            Self::Medium => HapticGenerator::Impact(1),
            Self::Heavy => HapticGenerator::Impact(2),
            Self::Soft => HapticGenerator::Impact(3),
            Self::Rigid => HapticGenerator::Impact(4),
            Self::Selection => HapticGenerator::Selection,
            Self::Success => HapticGenerator::Notification(0),
            Self::Warning => HapticGenerator::Notification(1),
            Self::Error => HapticGenerator::Notification(2),
        }
    }
}

/// What may satisfy a biometric prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BiometricPolicy {
    /// Face ID, Touch ID or Optic ID only.
    #[default]
    Biometrics,
    /// Biometrics, falling back to the device passcode.
    BiometricsOrPasscode,
}

impl BiometricPolicy {
    /// The `LAPolicy` value.
    pub(crate) fn raw(self) -> isize {
        match self {
            Self::Biometrics => 1,
            Self::BiometricsOrPasscode => 2,
        }
    }
}

/// The biometric sensor the device has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Biometry {
    /// None, or none the app may use.
    None,
    /// Touch ID.
    TouchId,
    /// Face ID. The app's Info.plist must carry `NSFaceIDUsageDescription`.
    FaceId,
    /// Optic ID (Apple Vision Pro).
    OpticId,
}

impl Biometry {
    /// From `LABiometryType`.
    pub(crate) fn from_raw(raw: isize) -> Self {
        match raw {
            1 => Self::TouchId,
            2 => Self::FaceId,
            4 => Self::OpticId,
            _ => Self::None,
        }
    }
}

/// Whether a prompt with a given policy could succeed now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiometricStatus {
    /// The sensor the device has.
    pub biometry: Biometry,
    /// `Ok` when a prompt could succeed; otherwise why it cannot.
    pub available: Result<(), ServiceError>,
}

/// Maps an `LAError` code to a service error.
pub(crate) fn la_error(code: isize) -> ServiceError {
    match code {
        // authenticationFailed, userCancel, userFallback
        -3..=-1 => ServiceError::Denied,
        // systemCancel, appCancel, invalidContext
        -4 | -9 | -10 => ServiceError::Cancelled,
        // passcodeNotSet, biometryNotEnrolled
        -5 | -7 => ServiceError::NotEnrolled,
        // biometryNotAvailable, biometryNotPaired, biometryDisconnected
        -6 | -11 | -12 => ServiceError::Unsupported,
        -8 => ServiceError::Lockout,
        // notInteractive: the app is in the background.
        -1004 => ServiceError::Unavailable,
        _ => ServiceError::Failed,
    }
}

/// When a keychain item can be read, and whether reading it needs the user.
///
/// Every variant keeps the item on this device: it is excluded from
/// backups restored to another device and from iCloud Keychain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeychainAccess {
    /// Readable once the device has been unlocked after booting, including
    /// from background work. The usual choice for tokens.
    #[default]
    AfterFirstUnlock,
    /// Readable only while the device is unlocked.
    WhenUnlocked,
    /// Reading asks for Face ID, Touch ID or the passcode. Needs a passcode
    /// to be set; removing the passcode deletes the item.
    UserPresence,
    /// Reading asks for the currently enrolled biometrics only. Enrolling a
    /// new face or finger makes the item unreadable.
    BiometryCurrentSet,
}

impl KeychainAccess {
    /// Whether reading shows a prompt (and so must not run on the main thread).
    pub fn prompts(self) -> bool {
        matches!(self, Self::UserPresence | Self::BiometryCurrentSet)
    }

    /// The `SecAccessControlCreateFlags` for prompting items.
    pub(crate) fn access_control_flags(self) -> Option<usize> {
        match self {
            Self::UserPresence => Some(1 << 0),
            Self::BiometryCurrentSet => Some(1 << 3),
            Self::AfterFirstUnlock | Self::WhenUnlocked => None,
        }
    }
}

/// Checks a keychain service/account pair and secret.
pub(crate) fn validate_keychain(
    service: &str,
    account: &str,
    secret: Option<&[u8]>,
) -> Result<(), ServiceError> {
    let name_ok = |name: &str| {
        !name.is_empty()
            && name.len() <= limits::KEYCHAIN_NAME
            && !name.chars().any(char::is_control)
    };
    if !name_ok(service) || !name_ok(account) {
        return Err(ServiceError::Invalid);
    }
    if secret.is_some_and(|secret| secret.len() > limits::KEYCHAIN_SECRET) {
        return Err(ServiceError::Invalid);
    }
    Ok(())
}

/// Maps an `OSStatus` from the keychain.
pub(crate) fn keychain_status(status: i32) -> Result<(), ServiceError> {
    match status {
        0 => Ok(()),
        -25300 => Err(ServiceError::Invalid), // errSecItemNotFound, handled by callers
        -128 | -25293 => Err(ServiceError::Denied), // userCanceled, authFailed
        -25308 => Err(ServiceError::Locked),  // interactionNotAllowed
        -34018 => Err(ServiceError::MissingEntitlement),
        -25291 => Err(ServiceError::Unavailable), // notAvailable
        _ => Err(ServiceError::Failed),
    }
}

/// A permission's state, as iOS reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    /// The user has not been asked yet.
    NotDetermined,
    /// Parental controls or a device profile forbid it; the user cannot grant it.
    Restricted,
    /// The user refused. Only the Settings app can change that now.
    Denied,
    /// Granted.
    Granted,
    /// Notifications only: delivered quietly to Notification Center until
    /// the user decides.
    Provisional,
}

impl PermissionState {
    /// Whether the capability can be used now.
    pub fn allows(self) -> bool {
        matches!(self, Self::Granted | Self::Provisional)
    }

    /// From `UNAuthorizationStatus`.
    pub(crate) fn from_notification(raw: isize) -> Self {
        match raw {
            1 => Self::Denied,
            2 => Self::Granted,
            // provisional, ephemeral (App Clips)
            3 | 4 => Self::Provisional,
            _ => Self::NotDetermined,
        }
    }

    /// From `AVAuthorizationStatus`.
    pub(crate) fn from_capture(raw: isize) -> Self {
        match raw {
            1 => Self::Restricted,
            2 => Self::Denied,
            3 => Self::Granted,
            _ => Self::NotDetermined,
        }
    }
}

/// Location permission, as iOS reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationPermission {
    /// The user has not been asked yet.
    NotDetermined,
    /// Parental controls or a device profile forbid location.
    Restricted,
    /// The user refused.
    Denied,
    /// Allowed while the app is in use.
    WhenInUse,
    /// Allowed at all times.
    Always,
}

impl LocationPermission {
    /// Whether foreground location can be read now.
    pub fn allows(self) -> bool {
        matches!(self, Self::WhenInUse | Self::Always)
    }

    /// From `CLAuthorizationStatus`.
    pub(crate) fn from_raw(raw: i32) -> Self {
        match raw {
            1 => Self::Restricted,
            2 => Self::Denied,
            3 => Self::Always,
            4 => Self::WhenInUse,
            _ => Self::NotDetermined,
        }
    }
}

/// Which notification styles to ask permission for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationOptions {
    /// Banners and alerts.
    pub alert: bool,
    /// Sounds.
    pub sound: bool,
    /// The app icon's badge.
    pub badge: bool,
    /// Deliver quietly without asking first (iOS 12 and later).
    pub provisional: bool,
}

impl Default for NotificationOptions {
    fn default() -> Self {
        Self {
            alert: true,
            sound: true,
            badge: true,
            provisional: false,
        }
    }
}

impl NotificationOptions {
    /// The `UNAuthorizationOptions` bits.
    pub(crate) fn raw(self) -> usize {
        usize::from(self.badge)
            | usize::from(self.sound) << 1
            | usize::from(self.alert) << 2
            | usize::from(self.provisional) << 6
    }
}

/// A notification the device shows by itself, now or after a delay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalNotification {
    /// Identifies the notification; scheduling the same id again replaces it.
    pub id: String,
    /// The bold first line.
    pub title: String,
    /// An optional second line.
    pub subtitle: Option<String>,
    /// The message.
    pub body: String,
    /// How long from now to show it; `None` shows it at once.
    pub delay: Option<Duration>,
    /// Play the default sound.
    pub sound: bool,
    /// Set the app icon's badge to this number (`Some(0)` clears it).
    pub badge: Option<u32>,
    /// Groups related notifications together.
    pub thread: Option<String>,
    /// Application data returned when the user taps it. Never shown.
    pub payload: Option<String>,
}

impl LocalNotification {
    /// A notification shown at once with the default sound.
    pub fn new(id: impl Into<String>, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            body: body.into(),
            delay: None,
            sound: true,
            badge: None,
            thread: None,
            payload: None,
        }
    }

    /// Checks the documented limits.
    pub fn validate(&self) -> Result<(), ServiceError> {
        let text = |value: &str| value.len() <= limits::NOTIFICATION_TEXT;
        let id_ok = !self.id.is_empty()
            && self.id.len() <= limits::NOTIFICATION_ID
            && !self.id.chars().any(char::is_control);
        let thread_ok = self
            .thread
            .as_deref()
            .is_none_or(|thread| thread.len() <= limits::NOTIFICATION_ID);
        let delay_ok = self
            .delay
            .is_none_or(|delay| delay.as_secs() <= limits::NOTIFICATION_DELAY_SECS);
        let payload_ok = self
            .payload
            .as_deref()
            .is_none_or(|payload| payload.len() <= limits::NOTIFICATION_PAYLOAD);
        let content = !self.title.is_empty() || !self.body.is_empty();
        if id_ok
            && thread_ok
            && delay_ok
            && payload_ok
            && content
            && text(&self.title)
            && text(&self.body)
            && self.subtitle.as_deref().is_none_or(text)
        {
            Ok(())
        } else {
            Err(ServiceError::Invalid)
        }
    }

    /// The time-interval trigger, if any. iOS rejects a zero interval, so
    /// anything under 10 ms is delivered at once instead.
    pub(crate) fn interval(&self) -> Option<f64> {
        self.delay
            .filter(|delay| *delay >= Duration::from_millis(10))
            .map(|delay| delay.as_secs_f64())
    }
}

/// The user-info key carrying [`LocalNotification::payload`].
pub(crate) const PAYLOAD_KEY: &str = "taim.payload";

/// Something the user did with a notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationEvent {
    /// The user opened the notification (including one that launched the app).
    Opened {
        /// The notification's id.
        id: String,
        /// Its [`LocalNotification::payload`], if it had one.
        payload: Option<String>,
    },
    /// The user dismissed it from Notification Center.
    Dismissed {
        /// The notification's id.
        id: String,
        /// Its payload, if it had one.
        payload: Option<String>,
    },
    /// It arrived while the app was in front. It is shown as a banner too.
    Presented {
        /// The notification's id.
        id: String,
        /// Its payload, if it had one.
        payload: Option<String>,
    },
}

impl NotificationEvent {
    /// From a `UNNotificationResponse.actionIdentifier`.
    pub(crate) fn response(action: &str, id: String, payload: Option<String>) -> Self {
        if action == "com.apple.UNNotificationDismissActionIdentifier" {
            Self::Dismissed { id, payload }
        } else {
            Self::Opened { id, payload }
        }
    }
}

/// How precise a location reading should be. Finer readings take longer
/// and use more power.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LocationAccuracy {
    /// The best the hardware offers.
    Best,
    /// About ten meters.
    #[default]
    TenMeters,
    /// About a hundred meters.
    HundredMeters,
    /// About a kilometer.
    Kilometer,
}

impl LocationAccuracy {
    /// The `CLLocationAccuracy` value.
    pub(crate) fn meters(self) -> f64 {
        match self {
            Self::Best => -1.0,
            Self::TenMeters => 10.0,
            Self::HundredMeters => 100.0,
            Self::Kilometer => 1000.0,
        }
    }
}

/// A one-shot location request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocationOptions {
    /// Precision wanted.
    pub accuracy: LocationAccuracy,
    /// Give up after this long (at most two minutes).
    pub timeout: Duration,
    /// Accept a reading the system already has if it is at most this old.
    /// Zero always asks for a fresh one.
    pub maximum_age: Duration,
}

impl Default for LocationOptions {
    fn default() -> Self {
        Self {
            accuracy: LocationAccuracy::default(),
            timeout: Duration::from_secs(30),
            maximum_age: Duration::from_secs(60),
        }
    }
}

impl LocationOptions {
    /// Checks the documented limits.
    pub fn validate(&self) -> Result<(), ServiceError> {
        if self.timeout.is_zero()
            || self.timeout.as_secs() > limits::LOCATION_TIMEOUT_SECS
            || self.maximum_age.as_secs() > limits::LOCATION_MAX_AGE_SECS
        {
            Err(ServiceError::Invalid)
        } else {
            Ok(())
        }
    }
}

/// Continuous foreground location updates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocationWatchOptions {
    /// Precision wanted.
    pub accuracy: LocationAccuracy,
    /// Only report after moving at least this many meters (0 reports every change).
    pub distance_filter: f64,
}

impl Default for LocationWatchOptions {
    fn default() -> Self {
        Self {
            accuracy: LocationAccuracy::default(),
            distance_filter: 10.0,
        }
    }
}

impl LocationWatchOptions {
    /// Checks the documented limits.
    pub fn validate(&self) -> Result<(), ServiceError> {
        if self.distance_filter.is_finite() && (0.0..=100_000.0).contains(&self.distance_filter) {
            Ok(())
        } else {
            Err(ServiceError::Invalid)
        }
    }
}

/// A position reported by Core Location.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocationFix {
    /// Degrees north, −90 to 90.
    pub latitude: f64,
    /// Degrees east, −180 to 180.
    pub longitude: f64,
    /// Radius of uncertainty in meters.
    pub accuracy: f64,
    /// Meters above sea level, when known.
    pub altitude: Option<f64>,
    /// Uncertainty of the altitude in meters, when known.
    pub altitude_accuracy: Option<f64>,
    /// Meters per second, when known.
    pub speed: Option<f64>,
    /// Degrees clockwise from true north, when known.
    pub course: Option<f64>,
    /// When the reading was taken.
    pub timestamp: SystemTime,
}

impl LocationFix {
    /// Builds a fix from `CLLocation`'s raw fields, refusing invalid ones.
    /// Core Location marks unknown values with a negative accuracy or value.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_raw(
        latitude: f64,
        longitude: f64,
        horizontal_accuracy: f64,
        altitude: f64,
        vertical_accuracy: f64,
        speed: f64,
        course: f64,
        timestamp: f64,
    ) -> Option<Self> {
        let valid = latitude.is_finite()
            && longitude.is_finite()
            && (-90.0..=90.0).contains(&latitude)
            && (-180.0..=180.0).contains(&longitude)
            && horizontal_accuracy.is_finite()
            && horizontal_accuracy >= 0.0
            && timestamp.is_finite()
            && timestamp >= 0.0;
        if !valid {
            return None;
        }
        let known = |value: f64| (value.is_finite() && value >= 0.0).then_some(value);
        let altitude_known = known(vertical_accuracy).is_some() && altitude.is_finite();
        Some(Self {
            latitude,
            longitude,
            accuracy: horizontal_accuracy,
            altitude: altitude_known.then_some(altitude),
            altitude_accuracy: altitude_known.then_some(vertical_accuracy),
            speed: known(speed),
            course: known(course),
            timestamp: SystemTime::UNIX_EPOCH + Duration::from_secs_f64(timestamp),
        })
    }

    /// How long ago the reading was taken.
    pub fn age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.timestamp)
            .unwrap_or_default()
    }
}

/// What the photo library picker offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MediaKind {
    /// Photos, including Live Photos' still image.
    #[default]
    Images,
    /// Videos.
    Videos,
    /// Both.
    Any,
}

/// A photo library selection. No permission is needed: the system picker
/// runs outside the app and hands over only what the user chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaPicker {
    /// What to offer.
    pub kind: MediaKind,
    /// Most items to return, 1 to 50.
    pub limit: u16,
}

impl Default for MediaPicker {
    fn default() -> Self {
        Self {
            kind: MediaKind::Images,
            limit: 1,
        }
    }
}

impl MediaPicker {
    /// Checks the documented limits.
    pub fn validate(&self) -> Result<(), ServiceError> {
        if (1..=limits::MEDIA_SELECTION).contains(&self.limit) {
            Ok(())
        } else {
            Err(ServiceError::Invalid)
        }
    }
}

/// What the camera should capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaptureKind {
    /// A still photo, saved as JPEG.
    #[default]
    Photo,
    /// A video with sound. The Info.plist needs `NSMicrophoneUsageDescription`.
    Video,
}

/// A file the user picked or captured, copied into the app's caches
/// directory. The app owns it: move it somewhere durable or delete it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaFile {
    /// Where the copy is.
    pub path: PathBuf,
    /// Its uniform type identifier, such as `public.jpeg` or `com.apple.quicktime-movie`.
    pub type_identifier: String,
    /// Its size in bytes.
    pub size: u64,
}

/// The type identifiers a picked item is loaded as, most specific first.
pub(crate) fn media_types(kind: MediaKind) -> &'static [&'static str] {
    match kind {
        MediaKind::Images => &["public.image"],
        MediaKind::Videos => &["public.movie"],
        MediaKind::Any => &["public.movie", "public.image"],
    }
}

/// The uniform type identifier of a picked file, from its extension,
/// falling back to the abstract type it was loaded as.
pub(crate) fn type_for_extension(extension: Option<&str>, fallback: &'static str) -> &'static str {
    match extension.map(str::to_ascii_lowercase).as_deref() {
        Some("jpg" | "jpeg") => "public.jpeg",
        Some("heic") => "public.heic",
        Some("heif") => "public.heif",
        Some("png") => "public.png",
        Some("gif") => "com.compuserve.gif",
        Some("webp") => "org.webmproject.webp",
        Some("mov") => "com.apple.quicktime-movie",
        Some("mp4") => "public.mpeg-4",
        Some("m4v") => "com.apple.m4v-video",
        _ => fallback,
    }
}

/// A file extension safe to use for a copied item.
pub(crate) fn safe_extension(extension: Option<&str>) -> String {
    extension
        .filter(|extension| {
            !extension.is_empty()
                && extension.len() <= 10
                && extension.chars().all(|c| c.is_ascii_alphanumeric())
        })
        .map_or_else(|| "bin".to_owned(), str::to_ascii_lowercase)
}

/// The part of a full-screen view not covered by the status bar, the notch
/// or the home indicator, in logical pixels relative to the view.
///
/// winit reports the window's frame (`outer`) and its safe area (`inner`)
/// in physical screen coordinates; `physical` is the drawable size.
pub(crate) fn safe_area(
    physical: Size<u32>,
    scale: f32,
    outer: (Point<i32>, Size<u32>),
    inner: (Point<i32>, Size<u32>),
) -> Rectangle {
    let full = Rectangle::new(
        Point::ORIGIN,
        Size::new(
            physical.width as f32 / scale,
            physical.height as f32 / scale,
        ),
    );
    if scale <= 0.0 || inner.1.width == 0 || inner.1.height == 0 {
        return full;
    }
    let left = (inner.0.x - outer.0.x).max(0) as u32;
    let top = (inner.0.y - outer.0.y).max(0) as u32;
    let left = left.min(physical.width);
    let top = top.min(physical.height);
    let width = inner.1.width.min(physical.width - left);
    let height = inner.1.height.min(physical.height - top);
    Rectangle::new(
        Point::new(left as f32 / scale, top as f32 / scale),
        Size::new(width as f32 / scale, height as f32 / scale),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haptics_map_to_uikit_constants() {
        assert_eq!(Haptic::Light.generator(), HapticGenerator::Impact(0));
        assert_eq!(Haptic::Rigid.generator(), HapticGenerator::Impact(4));
        assert_eq!(Haptic::Selection.generator(), HapticGenerator::Selection);
        assert_eq!(Haptic::Error.generator(), HapticGenerator::Notification(2));
    }

    #[test]
    fn local_authentication_errors_are_classified() {
        assert_eq!(la_error(-2), ServiceError::Denied);
        assert_eq!(la_error(-4), ServiceError::Cancelled);
        assert_eq!(la_error(-7), ServiceError::NotEnrolled);
        assert_eq!(la_error(-5), ServiceError::NotEnrolled);
        assert_eq!(la_error(-6), ServiceError::Unsupported);
        assert_eq!(la_error(-8), ServiceError::Lockout);
        assert_eq!(la_error(-1004), ServiceError::Unavailable);
        assert_eq!(la_error(-999), ServiceError::Failed);
        assert_eq!(Biometry::from_raw(2), Biometry::FaceId);
        assert_eq!(Biometry::from_raw(3), Biometry::None);
    }

    #[test]
    fn keychain_names_and_secrets_are_bounded() {
        assert!(validate_keychain("svc", "acct", Some(b"x")).is_ok());
        assert!(validate_keychain("", "acct", None).is_err());
        assert!(validate_keychain("svc", "a\nb", None).is_err());
        assert!(validate_keychain(&"s".repeat(257), "acct", None).is_err());
        let big = vec![0; limits::KEYCHAIN_SECRET + 1];
        assert!(validate_keychain("svc", "acct", Some(&big)).is_err());
        assert_eq!(keychain_status(0), Ok(()));
        assert_eq!(keychain_status(-25308), Err(ServiceError::Locked));
        assert_eq!(
            keychain_status(-34018),
            Err(ServiceError::MissingEntitlement)
        );
        assert_eq!(keychain_status(-128), Err(ServiceError::Denied));
        assert!(KeychainAccess::UserPresence.prompts());
        assert!(!KeychainAccess::AfterFirstUnlock.prompts());
        assert_eq!(
            KeychainAccess::BiometryCurrentSet.access_control_flags(),
            Some(8)
        );
    }

    #[test]
    fn permission_states_decode_apple_values() {
        assert_eq!(
            PermissionState::from_notification(0),
            PermissionState::NotDetermined
        );
        assert_eq!(
            PermissionState::from_notification(2),
            PermissionState::Granted
        );
        assert_eq!(
            PermissionState::from_notification(3),
            PermissionState::Provisional
        );
        assert!(PermissionState::Provisional.allows());
        assert!(!PermissionState::Denied.allows());
        assert_eq!(
            PermissionState::from_capture(1),
            PermissionState::Restricted
        );
        assert_eq!(
            LocationPermission::from_raw(4),
            LocationPermission::WhenInUse
        );
        assert_eq!(LocationPermission::from_raw(3), LocationPermission::Always);
        assert!(!LocationPermission::from_raw(0).allows());
    }

    #[test]
    fn notification_options_and_limits() {
        assert_eq!(NotificationOptions::default().raw(), 0b111);
        let provisional = NotificationOptions {
            alert: false,
            sound: false,
            badge: false,
            provisional: true,
        };
        assert_eq!(provisional.raw(), 64);

        let mut note = LocalNotification::new("n1", "Hello", "World");
        assert!(note.validate().is_ok());
        assert_eq!(note.interval(), None);
        note.delay = Some(Duration::from_millis(1));
        assert_eq!(note.interval(), None);
        note.delay = Some(Duration::from_secs(5));
        assert_eq!(note.interval(), Some(5.0));
        note.delay = Some(Duration::from_secs(limits::NOTIFICATION_DELAY_SECS + 1));
        assert!(note.validate().is_err());

        let empty = LocalNotification::new("n1", "", "");
        assert!(empty.validate().is_err());
        let bad_id = LocalNotification::new("", "t", "b");
        assert!(bad_id.validate().is_err());
        let mut big = LocalNotification::new("n1", "t", "b");
        big.payload = Some("x".repeat(limits::NOTIFICATION_PAYLOAD + 1));
        assert!(big.validate().is_err());

        assert!(matches!(
            NotificationEvent::response(
                "com.apple.UNNotificationDismissActionIdentifier",
                "a".into(),
                None
            ),
            NotificationEvent::Dismissed { .. }
        ));
        assert!(matches!(
            NotificationEvent::response(
                "com.apple.UNNotificationDefaultActionIdentifier",
                "a".into(),
                None
            ),
            NotificationEvent::Opened { .. }
        ));
    }

    #[test]
    fn location_fixes_reject_invalid_samples() {
        let fix = LocationFix::from_raw(28.6, 77.2, 12.0, 210.0, 4.0, -1.0, 90.0, 1.0e9)
            .expect("a valid fix");
        assert_eq!(fix.altitude, Some(210.0));
        assert_eq!(fix.speed, None);
        assert_eq!(fix.course, Some(90.0));

        let no_altitude =
            LocationFix::from_raw(0.0, 0.0, 5.0, 10.0, -1.0, 1.0, 0.0, 1.0e9).expect("valid");
        assert_eq!(no_altitude.altitude, None);
        assert_eq!(no_altitude.altitude_accuracy, None);

        assert!(LocationFix::from_raw(91.0, 0.0, 5.0, 0.0, 0.0, 0.0, 0.0, 1.0).is_none());
        assert!(LocationFix::from_raw(0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0).is_none());
        assert!(LocationFix::from_raw(f64::NAN, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0).is_none());

        assert!(LocationOptions::default().validate().is_ok());
        let zero = LocationOptions {
            timeout: Duration::ZERO,
            ..LocationOptions::default()
        };
        assert!(zero.validate().is_err());
        let long = LocationOptions {
            timeout: Duration::from_secs(121),
            ..LocationOptions::default()
        };
        assert!(long.validate().is_err());
        assert!(LocationWatchOptions::default().validate().is_ok());
        let negative = LocationWatchOptions {
            distance_filter: -1.0,
            ..LocationWatchOptions::default()
        };
        assert!(negative.validate().is_err());
        assert_eq!(LocationAccuracy::Best.meters(), -1.0);
    }

    #[test]
    fn media_requests_are_bounded() {
        assert!(MediaPicker::default().validate().is_ok());
        let none = MediaPicker {
            limit: 0,
            ..MediaPicker::default()
        };
        assert!(none.validate().is_err());
        let many = MediaPicker {
            limit: 51,
            ..MediaPicker::default()
        };
        assert!(many.validate().is_err());
        assert_eq!(
            media_types(MediaKind::Any),
            &["public.movie", "public.image"]
        );
        assert_eq!(safe_extension(Some("HEIC")), "heic");
        assert_eq!(safe_extension(Some("../x")), "bin");
        assert_eq!(safe_extension(None), "bin");
        assert_eq!(
            type_for_extension(Some("HEIC"), "public.image"),
            "public.heic"
        );
        assert_eq!(
            type_for_extension(Some("MOV"), "public.movie"),
            "com.apple.quicktime-movie"
        );
        assert_eq!(
            type_for_extension(Some("raw"), "public.image"),
            "public.image"
        );
    }

    #[test]
    fn the_safe_area_excludes_notch_and_home_indicator() {
        // iPhone 15 portrait: 393×852 points at 3×, 59pt top, 34pt bottom.
        let physical = Size::new(1179, 2556);
        let area = safe_area(
            physical,
            3.0,
            (Point::new(0, 0), physical),
            (Point::new(0, 177), Size::new(1179, 2556 - 177 - 102)),
        );
        assert_eq!(area.y, 59.0);
        assert_eq!(area.x, 0.0);
        assert_eq!(area.height, 852.0 - 59.0 - 34.0);
        assert_eq!(area.width, 393.0);

        // Landscape: insets on both sides.
        let physical = Size::new(2556, 1179);
        let area = safe_area(
            physical,
            3.0,
            (Point::new(0, 0), physical),
            (Point::new(177, 0), Size::new(2556 - 354, 1179 - 63)),
        );
        assert_eq!(area.x, 59.0);
        assert_eq!(area.width, 852.0 - 118.0);

        // Nonsense from the platform falls back to the whole view.
        let area = safe_area(
            physical,
            3.0,
            (Point::new(0, 0), physical),
            (Point::new(0, 0), Size::new(0, 0)),
        );
        assert_eq!(area.width, 852.0);
    }
}
