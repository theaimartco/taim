//! Typed, bounded Android service requests. Handles hold no activity alive.
use super::WorkSpec;
use super::bridge::{self, Bridge};
use super::{LocationAccuracy, LocationEnd, LocationFix, LocationOptions, LocationWatchOptions};
use super::{MediaCapture, MediaPicker, Notification, NotificationChannel, NotificationTap};
use futures::channel::{mpsc, oneshot};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, Instant};

/// Maximum binary transfer per request, independent of document size on disk.
pub const MAX_TRANSFER: usize = 4 * 1024 * 1024;
/// A platform request completed without a usable result.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ServiceError {
    /// Location is disabled in system/provider settings.
    #[error("Android location is disabled")]
    LocationDisabled,
    /// A foreground operation was cancelled or its activity paused.
    #[error("Android operation cancelled")]
    Cancelled,
    /// The required Android component or installed handler is unavailable.
    #[error("Android capability unavailable on this device")]
    Unsupported,
    /// Activity has closed or native services are not attached.
    #[error("Android activity unavailable")]
    Unavailable,
    /// The bounded request queue or a platform picker is occupied.
    #[error("Android service busy")]
    Busy,
    /// Caller cancelled the picker or the operating system denied access.
    #[error("Android operation cancelled or denied")]
    Denied,
    /// Input failed validation or exceeded a documented limit.
    #[error("Android request invalid or too large")]
    Invalid,
    /// No response arrived before the request deadline.
    #[error("Android request timed out")]
    Timeout,
    /// Too many failed biometric attempts; retry later or use the device credential.
    #[error("Android biometric authentication temporarily locked")]
    Lockout,
    /// Biometrics are locked until the user unlocks the device with its credential.
    #[error("Android biometric authentication locked until device credential")]
    LockoutPermanent,
    /// No biometric or device credential satisfying the policy is enrolled.
    #[error("Android biometric not enrolled")]
    NotEnrolled,
    /// The required radio or service (Bluetooth, NFC, location) is switched off.
    #[error("Android capability disabled")]
    Disabled,
    /// The referenced connection, tag, region or product no longer exists.
    #[error("Android resource not found")]
    NotFound,
    /// A biometric-bound key was invalidated (new enrollment or lock-screen removal); recreate it.
    #[error("Android biometric key invalidated")]
    KeyInvalidated,
    /// The OS service failed. Sensitive platform exception text is withheld.
    #[error("Android platform operation failed")]
    Failed,
}
/// Supported native platform operations. Binary payloads use bytes, not file paths.
pub enum Request {
    /// Biometric/credential availability for a policy, without UI.
    BiometricStatus(super::BiometricPolicy),
    /// Show the system biometric prompt. Prefer Services::authenticate; dropping it dismisses the prompt.
    Authenticate(super::BiometricPrompt),
    /// Biometric-bound Keystore operation built by the typed `Services::biometric_*` methods.
    #[doc(hidden)]
    BiometricKey(Value),
    /// Default permission-free motion/environment sensor descriptors, at most 13.
    SensorInventory,
    /// One fresh foreground sample. Bounded timeout; pause cancels the listener.
    ReadSensor(super::SensorOptions),
    /// Schedule durable deferrable work; requires the opt-in service and worker export.
    EnqueueWork(WorkSpec),
    /// Read a job's durable state/input/progress/output, or null when absent.
    WorkInfo(u16),
    /// List bounded job metadata (without input/output/progress payloads).
    WorkList {
        /// Include the bounded terminal history as well as active jobs.
        include_finished: bool,
    },
    /// Cancel one logical job and fence out late worker progress/results.
    CancelWork(u16),
    /// Remove terminal ledger history only.
    PruneWork,
    /// Explicitly reconcile durable active jobs missing from the OS scheduler.
    ReconcileWork,
    /// Permission/provider status without acquiring coordinates or prompting.
    LocationStatus,
    /// Open system location settings; never silently enables location.
    LocationSettings,
    /// Foreground cached sample, or null if no sample meets the age/provider policy.
    LastLocation {
        /// Provider preference, subject to permission.
        accuracy: LocationAccuracy,
        /// Maximum age, 0–86,400,000 ms; zero never returns a cached sample.
        maximum_age_ms: u64,
    },
    /// Foreground one-shot location with cache and timeout policy.
    CurrentLocation(LocationOptions),
    /// Bounded foreground updates. Prefer Services::watch_location for automatic stop-on-drop.
    WatchLocation(LocationWatchOptions),
    /// Stop a native watch token, idempotently.
    StopLocation(u64),
    /// System photo/video selection, returning `{items:[metadata], source}`.
    PickMedia(MediaPicker),
    /// Delegate capture to a camera app. Requires the opt-in TaimMediaProvider manifest entry.
    CaptureMedia(MediaCapture),
    /// List at most 64 owned capture files, including files interrupted by abrupt process death.
    CapturedMedia,
    /// Inspect a granted content URI without loading its complete contents.
    MediaInfo(String),
    /// Attempt to retain a selected URI's read grant; fails if its provider disallows persistence.
    RetainMedia(String),
    /// Release this app's persisted grants for a URI; does not delete external media.
    ReleaseMedia(String),
    /// Delete only a framework-owned capture file, revoking temporary grants first.
    DeleteCapturedMedia(String),
    /// Open an installed native viewer for a granted image/video URI.
    PreviewMedia(String),
    /// Read foreground clipboard text/HTML.
    ClipboardRead {
        #[doc = "HTML selection or optional HTML representation."]
        html: bool,
    },
    /// Write clipboard text or HTML, with an optional sensitive-content hint.
    ClipboardWrite {
        #[doc = "UTF-8 text payload."]
        text: String,
        #[doc = "HTML selection or optional HTML representation."]
        html: Option<String>,
        #[doc = "Request the platform sensitive-clipboard hint."]
        sensitive: bool,
    },
    /// Request manifest-declared runtime permissions.
    Permissions(Vec<String>),
    /// Inspect permissions without opening a system prompt; response matches Permissions.
    PermissionStatus(Vec<String>),
    /// Inspect OS-managed special access without opening settings.
    SpecialAccessStatus(super::SpecialAccess),
    /// Open special-access settings; opening the page does not grant access.
    SpecialAccessSettings(super::SpecialAccess),
    /// Open this app's system settings page. Returns after dispatch, not after changes.
    AppSettings,
    /// Create/update a channel's name/description; user-controlled importance is preserved.
    NotificationChannel(NotificationChannel),
    /// Delete an app channel. Android remembers user choices if the ID is recreated.
    NotificationChannelDelete(String),
    /// Post/update a local notification. Missing permission or a blocked channel returns Denied.
    NotificationPost(Notification),
    /// Cancel one nonnegative app-local notification ID.
    NotificationCancel(i32),
    /// Query app notification permission, active IDs and an optional channel's status.
    NotificationStatus {
        /// Optional existing channel ID to inspect.
        channel: Option<String>,
    },
    /// Open app/channel notification settings; does not request permission automatically.
    NotificationSettings {
        /// Optional existing channel ID whose settings to open.
        channel: Option<String>,
    },
    /// Open an external HTTP(S), mailto, tel or geo URL.
    OpenUrl(String),
    /// System text share sheet.
    ShareText {
        #[doc = "UTF-8 text payload."]
        text: String,
        #[doc = "Chooser title shown to the user."]
        title: String,
    },
    /// Share an existing granted content URI through the native chooser.
    ShareDocument {
        #[doc = "Content URI or allowed external URL."]
        uri: String,
        #[doc = "MIME type of the native content."]
        mime_type: String,
        #[doc = "Chooser title shown to the user."]
        title: String,
    },
    /// SAF picker, returning a content URI or null on cancellation.
    OpenDocument {
        #[doc = "Accepted MIME types."]
        mime_types: Vec<String>,
    },
    /// SAF create-document picker.
    CreateDocument {
        #[doc = "MIME type of the native content."]
        mime_type: String,
        #[doc = "Asset path or suggested document filename."]
        name: String,
    },
    /// Read at most `MAX_TRANSFER` bytes using ContentResolver.
    ReadDocument {
        #[doc = "Content URI or allowed external URL."]
        uri: String,
        #[doc = "Maximum bytes to read, capped at MAX_TRANSFER."]
        limit: usize,
    },
    /// Replace content through an existing URI grant.
    WriteDocument {
        #[doc = "Content URI or allowed external URL."]
        uri: String,
        #[doc = "Binary payload, bounded by the operation limit."]
        bytes: Vec<u8>,
    },
    /// Read a packaged asset, bounded like a content URI read.
    ReadAsset {
        #[doc = "Asset path or suggested document filename."]
        name: String,
        #[doc = "Maximum bytes to read, capped at MAX_TRANSFER."]
        limit: usize,
    },
    /// Native long-press, keyboard-tap or selection haptic feedback.
    Haptic(Haptic),
    /// Store encrypted app-private bytes using an Android Keystore key.
    SecurePut {
        #[doc = "App-private secure-store key, at most 128 characters."]
        key: String,
        #[doc = "Binary payload, bounded by the operation limit."]
        bytes: Vec<u8>,
    },
    /// Retrieve encrypted app-private bytes, or null when absent.
    SecureGet(String),
    /// Remove a secure-store entry.
    SecureDelete(String),
    /// SDK, locale, density, font scale and device metadata.
    DeviceInfo,
    /// App-private files/cache/no-backup directories.
    Directories,
    /// Current launch/deep-link intent, including cold launches.
    LaunchIntent,
    /// Invoke an explicitly registered Java/Kotlin plugin channel.
    Plugin {
        #[doc = "Explicit host-registered channel name."]
        channel: String,
        #[doc = "Plugin method name."]
        method: String,
        #[doc = "JSON object passed to the registered plugin."]
        arguments: Value,
    },
}
/// Feedback follows the user's system haptics setting.
#[derive(Debug, Clone, Copy)]
pub enum Haptic {
    /// Long press feedback.
    LongPress,
    /// Software keyboard tap feedback.
    KeyboardTap,
    /// Selection/change feedback.
    Selection,
}
/// Native lifecycle and navigation notifications.
#[derive(Debug, Clone, PartialEq)]
pub enum PlatformEvent {
    /// Observed permission changes after requests or Activity resume, at most 128 watched names.
    PermissionsChanged(Vec<super::PermissionStatus>),
    /// Native hierarchy slot event. ID/generation zero indicates a scene-level error.
    NativeView {
        /// Rust slot identity.
        id: u64,
        /// Native instance generation, changed after disposal/recreation.
        generation: u64,
        /// Bounded factory/lifecycle/error object.
        event: Value,
    },
    /// Foreground update from a native watch token, never reused within this activity.
    Location {
        /// Native watch token.
        watch: u64,
        /// Validated sample.
        fix: LocationFix,
    },
    /// Terminal watch notification; the host has already removed its provider listener.
    LocationEnded {
        /// Native watch token.
        watch: u64,
        /// Stop reason.
        reason: LocationEnd,
    },
    /// Activity foreground/background state.
    Lifecycle(String),
    /// A cold or warm intent; callers decide which URI/action to accept.
    Intent {
        #[doc = "Android intent action."]
        action: String,
        #[doc = "Content URI or allowed external URL."]
        uri: Option<String>,
        #[doc = "MIME type of the native content."]
        mime_type: Option<String>,
        /// Optional local-notification tap metadata, including cold launches.
        notification: Option<NotificationTap>,
    },
    /// Predictive-back animation phase and normalized progress.
    Back {
        #[doc = "started, progressed, cancelled or invoked."]
        phase: String,
        #[doc = "Normalized gesture progress in the range zero to one."]
        progress: f32,
        #[doc = "Android swipe-edge identifier."]
        edge: i32,
    },
    /// Android memory pressure level.
    MemoryPressure(i32),
    /// Renderer/device resources were rebuilt; custom GPU integrations must refresh.
    GraphicsRestored,
    /// Event from a registered host plugin.
    Plugin {
        #[doc = "Explicit host-registered channel name."]
        channel: String,
        #[doc = "Plugin JSON event value."]
        event: Value,
    },
}

impl Request {
    fn wire(self) -> Result<Value, ServiceError> {
        use base64::Engine as _;
        let permission_status = matches!(&self, Self::PermissionStatus(_));
        let work_op = if matches!(&self, Self::WorkInfo(_)) {
            "workInfo"
        } else {
            "workCancel"
        };
        let media_op = match &self {
            Self::MediaInfo(_) => "mediaInfo",
            Self::RetainMedia(_) => "mediaRetain",
            Self::ReleaseMedia(_) => "mediaRelease",
            Self::DeleteCapturedMedia(_) => "mediaDelete",
            Self::PreviewMedia(_) => "mediaPreview",
            _ => "",
        };
        let encode = |bytes: Vec<u8>, max: usize| {
            if bytes.len() > max {
                Err(ServiceError::Invalid)
            } else {
                Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
            }
        };
        let value = match self {
            Self::EnqueueWork(spec) => spec.wire().ok_or(ServiceError::Invalid)?,
            Self::WorkInfo(id) | Self::CancelWork(id) => {
                if !(1..=4095).contains(&id) {
                    return Err(ServiceError::Invalid);
                }
                json!({"op":work_op,"id":id})
            }
            Self::WorkList { include_finished } => {
                json!({"op":"workList","finished":include_finished})
            }
            Self::PruneWork => json!({"op":"workPrune"}),
            Self::ReconcileWork => json!({"op":"workReconcile"}),
            Self::LocationStatus => json!({"op":"locationStatus"}),
            Self::BiometricStatus(policy) => super::biometric::status_wire(policy),
            Self::Authenticate(prompt) => prompt.wire().ok_or(ServiceError::Invalid)?,
            Self::BiometricKey(value) => {
                let op = value["op"].as_str().unwrap_or("");
                if !matches!(
                    op,
                    "biometricKeyCreate"
                        | "biometricKeyDelete"
                        | "biometricPublicKey"
                        | "biometricSign"
                        | "biometricEncrypt"
                        | "biometricDecrypt"
                ) {
                    return Err(ServiceError::Invalid);
                }
                value
            }
            Self::SensorInventory => json!({"op":"sensorInventory"}),
            Self::ReadSensor(options) => options.wire().ok_or(ServiceError::Invalid)?,
            Self::LocationSettings => json!({"op":"locationSettings"}),
            Self::CurrentLocation(options) => options.wire().ok_or(ServiceError::Invalid)?,
            Self::WatchLocation(options) => options.wire().ok_or(ServiceError::Invalid)?,
            Self::LastLocation {
                accuracy,
                maximum_age_ms,
            } => {
                let mut value = LocationOptions {
                    accuracy,
                    maximum_age_ms,
                    timeout_ms: 1,
                }
                .wire()
                .ok_or(ServiceError::Invalid)?;
                value["op"] = json!("locationLast");
                value
            }
            Self::StopLocation(watch) => {
                if watch == 0 || watch > i64::MAX as u64 {
                    return Err(ServiceError::Invalid);
                }
                json!({"op":"locationStop","watch":watch})
            }
            Self::PickMedia(picker) => picker.wire().ok_or(ServiceError::Invalid)?,
            Self::CaptureMedia(capture) => capture.wire().ok_or(ServiceError::Invalid)?,
            Self::CapturedMedia => json!({"op":"mediaList"}),
            Self::MediaInfo(uri)
            | Self::RetainMedia(uri)
            | Self::ReleaseMedia(uri)
            | Self::DeleteCapturedMedia(uri)
            | Self::PreviewMedia(uri) => {
                if !super::media::content_uri(&uri) {
                    return Err(ServiceError::Invalid);
                }
                json!({"op":media_op,"uri":uri})
            }
            Self::ClipboardRead { html } => json!({"op":"clipboardRead","html":html}),
            Self::ClipboardWrite {
                text,
                html,
                sensitive,
            } => json!({"op":"clipboardWrite","text":text,"html":html,"sensitive":sensitive}),
            Self::SpecialAccessStatus(kind) => {
                json!({"op":"specialAccessStatus","kind":kind.wire()})
            }
            Self::SpecialAccessSettings(kind) => {
                json!({"op":"specialAccessSettings","kind":kind.wire()})
            }
            Self::Permissions(names) | Self::PermissionStatus(names) => {
                if names.is_empty()
                    || names.len() > 32
                    || names.iter().collect::<std::collections::HashSet<_>>().len() != names.len()
                    || names
                        .iter()
                        .any(|n| n.len() > 256 || !n.starts_with("android.permission."))
                {
                    return Err(ServiceError::Invalid);
                }
                json!({"op":if permission_status {"permissionStatus"} else {"permissions"},"names":names})
            }
            Self::AppSettings => json!({"op":"appSettings"}),
            Self::NotificationChannel(channel) => channel.wire().ok_or(ServiceError::Invalid)?,
            Self::NotificationPost(notification) => {
                notification.wire().ok_or(ServiceError::Invalid)?
            }
            Self::NotificationChannelDelete(id) => {
                if !super::notifications::valid_channel(&id) {
                    return Err(ServiceError::Invalid);
                }
                json!({"op":"notificationChannelDelete","id":id})
            }
            Self::NotificationCancel(id) => {
                if id < 0 {
                    return Err(ServiceError::Invalid);
                }
                json!({"op":"notificationCancel","id":id})
            }
            Self::NotificationStatus { channel } => {
                if channel
                    .as_ref()
                    .is_some_and(|id| !super::notifications::valid_channel(id))
                {
                    return Err(ServiceError::Invalid);
                }
                json!({"op":"notificationStatus","channel":channel})
            }
            Self::NotificationSettings { channel } => {
                if channel
                    .as_ref()
                    .is_some_and(|id| !super::notifications::valid_channel(id))
                {
                    return Err(ServiceError::Invalid);
                }
                json!({"op":"notificationSettings","channel":channel})
            }
            Self::OpenUrl(uri) => json!({"op":"openUrl","uri":uri}),
            Self::ShareText { text, title } => json!({"op":"share","text":text,"title":title}),
            Self::ShareDocument {
                uri,
                mime_type,
                title,
            } => json!({"op":"shareDocument","uri":uri,"type":mime_type,"title":title}),
            Self::OpenDocument { mime_types } => json!({"op":"openDocument","types":mime_types}),
            Self::CreateDocument { mime_type, name } => {
                json!({"op":"createDocument","type":mime_type,"name":name})
            }
            Self::ReadDocument { uri, limit } => {
                json!({"op":"readDocument","uri":uri,"limit":limit.min(MAX_TRANSFER)})
            }
            Self::WriteDocument { uri, bytes } => {
                json!({"op":"writeDocument","uri":uri,"data":encode(bytes,MAX_TRANSFER)?})
            }
            Self::ReadAsset { name, limit } => {
                json!({"op":"asset","name":name,"limit":limit.min(MAX_TRANSFER)})
            }
            Self::Haptic(kind) => {
                json!({"op":"haptic","kind":match kind {Haptic::LongPress=>0,Haptic::KeyboardTap=>1,Haptic::Selection=>2}})
            }
            Self::SecurePut { key, bytes } => {
                json!({"op":"securePut","key":key,"data":encode(bytes,65536)?})
            }
            Self::SecureGet(key) => json!({"op":"secureGet","key":key}),
            Self::SecureDelete(key) => json!({"op":"secureDelete","key":key}),
            Self::Plugin {
                channel,
                method,
                arguments,
            } => {
                if channel.is_empty()
                    || channel.len() > 128
                    || method.is_empty()
                    || method.len() > 128
                    || !arguments.is_object()
                {
                    return Err(ServiceError::Invalid);
                }
                json!({"op":"plugin","channel":channel,"method":method,"arguments":arguments})
            }
            Self::DeviceInfo => json!({"op":"device"}),
            Self::Directories => json!({"op":"directories"}),
            Self::LaunchIntent => json!({"op":"intent"}),
        };
        if value.to_string().len() > 6 * 1024 * 1024 {
            return Err(ServiceError::Invalid);
        }
        Ok(value)
    }
}

/// A lifecycle-scoped service client, available while `run_android` owns the activity.
#[derive(Clone)]
pub struct Services {
    pub(super) bridge: Weak<Bridge>,
}
/// Gets the current host without installing a strong global activity reference.
pub fn services() -> Option<Services> {
    bridge::current().map(|bridge| Services {
        bridge: std::sync::Arc::downgrade(&bridge),
    })
}
impl Services {
    /// Query current permission observations without showing UI.
    pub async fn permission_status(
        &self,
        names: Vec<String>,
    ) -> Result<Vec<super::PermissionStatus>, ServiceError> {
        super::permissions::decode(self.request(Request::PermissionStatus(names)).await?)
            .ok_or(ServiceError::Failed)
    }
    /// Request eligible manifest-declared runtime permissions; special access uses settings instead.
    pub async fn request_permissions(
        &self,
        names: Vec<String>,
    ) -> Result<Vec<super::PermissionStatus>, ServiceError> {
        super::permissions::decode(self.request(Request::Permissions(names)).await?)
            .ok_or(ServiceError::Failed)
    }

    /// Acquire one validated foreground sample with a bounded cache/deadline policy.
    pub async fn current_location(
        &self,
        options: LocationOptions,
    ) -> Result<LocationFix, ServiceError> {
        LocationFix::from_value(self.request(Request::CurrentLocation(options)).await?)
            .ok_or(ServiceError::Failed)
    }
    /// Subscribe before starting updates, so early native samples are retained in the bounded stream.
    pub async fn watch_location(
        &self,
        options: LocationWatchOptions,
    ) -> Result<LocationSubscription, ServiceError> {
        let events = self.events()?;
        let result = self.request(Request::WatchLocation(options)).await?;
        let watch = result["watch"]
            .as_u64()
            .filter(|id| *id > 0 && *id <= i64::MAX as u64)
            .ok_or(ServiceError::Failed)?;
        Ok(LocationSubscription {
            client: self.clone(),
            watch,
            events,
            active: true,
        })
    }
    /// Dispatches a typed request. Responses use documented JSON primitives/maps;
    /// binary results are decoded with `bytes`. Interactive calls have a 120s deadline.
    pub async fn request(&self, request: Request) -> Result<Value, ServiceError> {
        let bridge = self.bridge.upgrade().ok_or(ServiceError::Unavailable)?;
        let cancellable = matches!(
            &request,
            Request::CurrentLocation(_)
                | Request::ReadSensor(_)
                | Request::Authenticate(_)
                | Request::BiometricKey(_)
        );
        let (id, receiver) = bridge.request_service(request.wire()?)?;
        let mut cancellation = ForegroundRequestCancellation {
            bridge: Arc::downgrade(&bridge),
            id,
            armed: cancellable,
        };
        drop(bridge);
        let result = receiver.await;
        cancellation.armed = false;
        result.map_err(|_| ServiceError::Unavailable)?
    }
    /// Query whether a prompt with `policy` can succeed, without showing UI.
    pub async fn biometric_status(
        &self,
        policy: super::BiometricPolicy,
    ) -> Result<super::BiometricStatus, ServiceError> {
        super::BiometricStatus::from_value(&self.request(Request::BiometricStatus(policy)).await?)
            .ok_or(ServiceError::Failed)
    }
    /// Show the system prompt. Cancellation by the user or the negative button is
    /// `Denied`; backgrounding is `Cancelled`. Dropping the future dismisses the prompt.
    /// Success is local user presence only, not a server credential.
    pub async fn authenticate(
        &self,
        prompt: super::BiometricPrompt,
    ) -> Result<super::BiometricAuthenticator, ServiceError> {
        super::BiometricAuthenticator::from_value(
            &self.request(Request::Authenticate(prompt)).await?,
        )
        .ok_or(ServiceError::Failed)
    }
    /// Acquire and validate one foreground sensor snapshot. Dropping this future
    /// posts native cancellation, as for CurrentLocation.
    pub async fn read_sensor(
        &self,
        options: super::SensorOptions,
    ) -> Result<super::SensorReading, ServiceError> {
        let sensor = options.sensor;
        let reading =
            super::SensorReading::from_value(self.request(Request::ReadSensor(options)).await?)
                .ok_or(ServiceError::Failed)?;
        if reading.sensor != sensor {
            return Err(ServiceError::Failed);
        }
        Ok(reading)
    }
    /// Subscribes to a bounded stream. Saturated/closed listeners disconnect;
    /// initial intent/lifecycle are replayed so cold-launch events cannot be lost.
    pub fn events(&self) -> Result<mpsc::Receiver<PlatformEvent>, ServiceError> {
        self.bridge
            .upgrade()
            .ok_or(ServiceError::Unavailable)?
            .services
            .subscribe()
    }
    /// Decode a binary response. Malformed or oversized platform replies fail closed.
    pub fn bytes(value: &Value) -> Result<Vec<u8>, ServiceError> {
        use base64::Engine as _;
        let encoded = value.as_str().ok_or(ServiceError::Failed)?;
        if encoded.len() > MAX_TRANSFER * 2 {
            return Err(ServiceError::Invalid);
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| ServiceError::Failed)?;
        if bytes.len() > MAX_TRANSFER {
            return Err(ServiceError::Invalid);
        }
        Ok(bytes)
    }
}

struct ForegroundRequestCancellation {
    bridge: Weak<Bridge>,
    id: u64,
    armed: bool,
}
impl Drop for ForegroundRequestCancellation {
    fn drop(&mut self) {
        if self.armed
            && let Some(bridge) = self.bridge.upgrade()
        {
            bridge.cancel_location_request(self.id);
        }
    }
}

/// Foreground location stream guard. Drop posts an idempotent native listener removal.
pub struct LocationSubscription {
    client: Services,
    watch: u64,
    events: mpsc::Receiver<PlatformEvent>,
    active: bool,
}
impl LocationSubscription {
    /// Native session identity, scoped to the owning activity.
    pub fn id(&self) -> u64 {
        self.watch
    }
    /// Wait for the next sample or terminal reason. Further calls after termination return None.
    pub async fn next(&mut self) -> Option<Result<LocationFix, LocationEnd>> {
        use futures::StreamExt as _;
        if !self.active {
            return None;
        }
        while let Some(event) = self.events.next().await {
            match event {
                PlatformEvent::Location { watch, fix } if watch == self.watch => {
                    return Some(Ok(fix));
                }
                PlatformEvent::LocationEnded { watch, reason } if watch == self.watch => {
                    self.active = false;
                    return Some(Err(reason));
                }
                _ => {}
            }
        }
        self.cancel();
        Some(Err(LocationEnd::Unavailable))
    }
    /// Stop the provider session explicitly. Dropping an unfinished stop future still triggers Drop cleanup.
    pub async fn stop(mut self) -> Result<(), ServiceError> {
        let _ = self
            .client
            .request(Request::StopLocation(self.watch))
            .await?;
        self.active = false;
        Ok(())
    }
    fn cancel(&mut self) {
        if self.active {
            self.active = false;
            if let Some(bridge) = self.client.bridge.upgrade() {
                bridge.stop_location(self.watch);
            }
        }
    }
}
impl Drop for LocationSubscription {
    fn drop(&mut self) {
        self.cancel();
    }
}

type Reply = oneshot::Sender<Result<Value, ServiceError>>;
pub(super) type ServiceReceiver = oneshot::Receiver<Result<Value, ServiceError>>;
#[derive(Default)]
pub(super) struct Timer {
    epoch: Mutex<u64>,
    changed: Condvar,
}
impl Timer {
    pub fn epoch(&self) -> u64 {
        *self
            .epoch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn notify(&self) {
        let mut epoch = self
            .epoch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *epoch = epoch.wrapping_add(1);
        self.changed.notify_one();
    }
    pub fn wait(&self, epoch: u64, delay: Option<Duration>) -> u64 {
        let current = self
            .epoch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = if let Some(delay) = delay {
            self.changed
                .wait_timeout_while(current, delay, |current| *current == epoch)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0
        } else {
            self.changed
                .wait_while(current, |current| *current == epoch)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        };
        *current
    }
}
#[derive(Default)]
pub(super) struct State {
    pending: Mutex<HashMap<u64, (Instant, Reply)>>,
    listeners: Mutex<Vec<mpsc::Sender<PlatformEvent>>>,
    replay: Mutex<(Option<PlatformEvent>, Option<PlatformEvent>)>,
    pub timer: Arc<Timer>,
}
impl State {
    pub fn begin(
        &self,
        id: u64,
    ) -> Result<oneshot::Receiver<Result<Value, ServiceError>>, ServiceError> {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        pending.retain(|_, (_, send)| !send.is_canceled());
        if pending.len() >= 8 {
            return Err(ServiceError::Busy);
        }
        let (send, receive) = oneshot::channel();
        let _ = pending.insert(id, (Instant::now() + Duration::from_secs(120), send));
        drop(pending);
        self.timer.notify();
        Ok(receive)
    }
    pub fn complete(&self, id: u64, result: Result<Value, ServiceError>) {
        let reply = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&id);
        self.timer.notify();
        if let Some((_, send)) = reply {
            let _ = send.send(result);
        }
    }
    pub fn expire(&self) -> Option<Duration> {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let stale: Vec<_> = pending
            .iter()
            .filter(|(_, (deadline, send))| *deadline <= Instant::now() || send.is_canceled())
            .map(|(id, _)| *id)
            .collect();
        let mut replies = Vec::new();
        for id in stale {
            if let Some((_, send)) = pending.remove(&id) {
                replies.push(send);
            }
        }
        let next = pending
            .values()
            .map(|(deadline, _)| deadline.saturating_duration_since(Instant::now()))
            .min();
        drop(pending);
        for send in replies {
            let _ = send.send(Err(ServiceError::Timeout));
        }
        next
    }
    fn subscribe(&self) -> Result<mpsc::Receiver<PlatformEvent>, ServiceError> {
        let mut listeners = self
            .listeners
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        listeners.retain(|listener| !listener.is_closed());
        if listeners.len() >= 8 {
            return Err(ServiceError::Busy);
        }
        let (mut send, receive) = mpsc::channel(32);
        let replay = self
            .replay
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for event in [&replay.0, &replay.1].into_iter().flatten() {
            let _ = send.try_send(event.clone());
        }
        listeners.push(send);
        Ok(receive)
    }
    pub fn event(&self, event: PlatformEvent) {
        {
            let mut replay = self
                .replay
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match &event {
                PlatformEvent::Intent { .. } => replay.0 = Some(event.clone()),
                PlatformEvent::Lifecycle(_) => replay.1 = Some(event.clone()),
                _ => {}
            }
        }
        self.listeners
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain_mut(|send| send.try_send(event.clone()).is_ok());
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.timer.notify();
        for (_, (_, send)) in self
            .pending
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .drain()
        {
            let _ = send.send(Err(ServiceError::Unavailable));
        }
    }
}
