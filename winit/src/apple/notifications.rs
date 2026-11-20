//! Local notifications through UserNotifications.
//!
//! The shell installs the notification-center delegate before UIKit starts,
//! so a tap that launched the app is kept until the app
//! subscribes with [`notification_events`].

use std::cell::RefCell;
use std::sync::Mutex;

use futures::channel::{mpsc, oneshot};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, NSObject};
use objc2::{ClassType, DeclaredClass, declare_class, msg_send, msg_send_id, mutability};
use objc2_foundation::MainThreadMarker;

use super::ffi;
use super::types::{
    LocalNotification, NotificationEvent, NotificationOptions, PAYLOAD_KEY, PermissionState,
    ServiceError, limits,
};

#[link(name = "UserNotifications", kind = "framework")]
unsafe extern "C" {
    static UNErrorDomain: *const AnyObject;
}

/// Events kept for the first subscriber, such as the tap that launched the app.
const BACKLOG: usize = 16;
/// Events buffered per subscriber before it is disconnected.
const SUBSCRIBER_BUFFER: usize = 32;

struct Listeners {
    subscribers: Vec<mpsc::Sender<NotificationEvent>>,
    backlog: Vec<NotificationEvent>,
}

static LISTENERS: Mutex<Listeners> = Mutex::new(Listeners {
    subscribers: Vec::new(),
    backlog: Vec::new(),
});

fn publish(event: NotificationEvent) {
    let mut listeners = LISTENERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    listeners
        .subscribers
        .retain_mut(|subscriber| subscriber.try_send(event.clone()).is_ok());
    if listeners.subscribers.is_empty() && listeners.backlog.len() < BACKLOG {
        listeners.backlog.push(event);
    }
}

/// What the user does with notifications: opening (including the tap that
/// launched the app), dismissing, and arrivals while the app is in front.
pub fn notification_events() -> mpsc::Receiver<NotificationEvent> {
    let (mut sender, receiver) = mpsc::channel(SUBSCRIBER_BUFFER);
    let mut listeners = LISTENERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for event in std::mem::take(&mut listeners.backlog) {
        let _ = sender.try_send(event);
    }
    listeners.subscribers.push(sender);
    receiver
}

fn center() -> Result<Retained<AnyObject>, ServiceError> {
    // SAFETY: referencing an exported constant keeps the framework linked.
    let _ = unsafe { UNErrorDomain };
    // Outside an app bundle (`cargo run` on a Mac) the center raises an
    // Objective-C exception instead of returning.
    if ffi::bundle_identifier().is_none() {
        return Err(ServiceError::Unavailable);
    }
    let class = ffi::class("UNUserNotificationCenter").ok_or(ServiceError::Unsupported)?;
    // SAFETY: `currentNotificationCenter` is thread-safe in a bundled app.
    let center: Option<Retained<AnyObject>> =
        unsafe { msg_send_id![class, currentNotificationCenter] };
    center.ok_or(ServiceError::Unavailable)
}

/// Reads `identifier` and the payload of a `UNNotification`.
///
/// # Safety
/// `notification` must point to a `UNNotification`.
unsafe fn describe(notification: *const AnyObject) -> Option<(String, Option<String>)> {
    // SAFETY: UNNotification → request → identifier/content.userInfo.
    unsafe {
        let request: *const AnyObject = msg_send![notification, request];
        if request.is_null() {
            return None;
        }
        let identifier: *const AnyObject = msg_send![request, identifier];
        let id = ffi::read_string(identifier)?;
        let content: *const AnyObject = msg_send![request, content];
        let payload = if content.is_null() {
            None
        } else {
            let info: *const AnyObject = msg_send![content, userInfo];
            if info.is_null() {
                None
            } else {
                let key = ffi::string(PAYLOAD_KEY);
                let value: *const AnyObject = msg_send![info, objectForKey: &*key];
                let is_string: bool =
                    !value.is_null() && msg_send![value, isKindOfClass: objc2::class!(NSString)];
                if is_string {
                    ffi::read_string(value)
                } else {
                    None
                }
            }
        };
        Some((id, payload))
    }
}

declare_class!(
    struct NotificationDelegate;

    // SAFETY: NSObject has no subclassing requirements; the delegate holds
    // no state and does not implement Drop.
    unsafe impl ClassType for NotificationDelegate {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "TaimNotificationDelegate";
    }

    impl DeclaredClass for NotificationDelegate {}

    unsafe impl NotificationDelegate {
        #[method(userNotificationCenter:willPresentNotification:withCompletionHandler:)]
        fn will_present(
            &self,
            _center: *mut AnyObject,
            notification: *mut AnyObject,
            handler: &block2::Block<dyn Fn(usize)>,
        ) {
            // SAFETY: UserNotifications passes a UNNotification.
            if let Some((id, payload)) = unsafe { describe(notification) } {
                publish(NotificationEvent::Presented { id, payload });
            }
            // Banner, list and sound: show it as if the app were in the background.
            handler.call(((1 << 4) | (1 << 3) | (1 << 1),));
        }

        #[method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:)]
        fn did_receive(
            &self,
            _center: *mut AnyObject,
            response: *mut AnyObject,
            handler: &block2::Block<dyn Fn()>,
        ) {
            // SAFETY: UNNotificationResponse → notification/actionIdentifier.
            unsafe {
                let notification: *const AnyObject = msg_send![response, notification];
                let action: *const AnyObject = msg_send![response, actionIdentifier];
                let action = ffi::read_string(action).unwrap_or_default();
                if !notification.is_null()
                    && let Some((id, payload)) = describe(notification)
                {
                    publish(NotificationEvent::response(&action, id, payload));
                }
            }
            handler.call(());
        }
    }
);

thread_local! {
    static DELEGATE: RefCell<Option<Retained<NotificationDelegate>>> = const { RefCell::new(None) };
}

/// Becomes the notification center's delegate. The shell calls this before
/// starting UIKit's run loop, as Apple requires for launch taps.
pub(crate) fn install(_mtm: MainThreadMarker) {
    DELEGATE.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let Ok(center) = center() else {
            return;
        };
        // SAFETY: a plain NSObject subclass; the center holds the delegate
        // weakly, so it is kept alive in DELEGATE for the process.
        unsafe {
            let delegate: Retained<NotificationDelegate> =
                msg_send_id![NotificationDelegate::alloc(), init];
            let _: () = msg_send![&center, setDelegate: &*delegate];
            *slot.borrow_mut() = Some(delegate);
        }
    });
}

/// The app's notification permission, without asking.
pub async fn notification_permission() -> Result<PermissionState, ServiceError> {
    let (sender, receiver) = oneshot::channel();
    objc2::rc::autoreleasepool(|_| -> Result<(), ServiceError> {
        let center = center()?;
        let sender = Mutex::new(Some(sender));
        let block = block2::RcBlock::new(move |settings: *mut AnyObject| {
            let state = if settings.is_null() {
                PermissionState::NotDetermined
            } else {
                // SAFETY: UNNotificationSettings.authorizationStatus.
                PermissionState::from_notification(unsafe {
                    msg_send![settings, authorizationStatus]
                })
            };
            if let Some(sender) = sender.lock().ok().and_then(|mut slot| slot.take()) {
                let _ = sender.send(state);
            }
        });
        // SAFETY: the center copies the block and calls it once.
        unsafe {
            let _: () = msg_send![&center, getNotificationSettingsWithCompletionHandler: &*block];
        }
        Ok(())
    })?;
    receiver.await.map_err(|_| ServiceError::Cancelled)
}

/// The result of a UserNotifications completion handler's `NSError`.
/// `UNErrorCodeNotificationsNotAllowed` (1) means the user has not allowed
/// notifications.
///
/// # Safety
/// `error` must be null or point to an `NSError`.
unsafe fn completion(error: *mut AnyObject) -> Result<(), ServiceError> {
    // SAFETY: forwarded from the caller.
    match unsafe { ffi::error_code(error) } {
        None => Ok(()),
        Some(1) => Err(ServiceError::Denied),
        Some(_) => Err(ServiceError::Failed),
    }
}

/// Asks for permission to notify (only the first time; afterwards iOS
/// answers with the user's earlier choice). Returns whether it is granted.
pub async fn request_notification_permission(
    options: NotificationOptions,
) -> Result<PermissionState, ServiceError> {
    let (sender, receiver) = oneshot::channel();
    objc2::rc::autoreleasepool(|_| -> Result<(), ServiceError> {
        let center = center()?;
        let sender = Mutex::new(Some(sender));
        let block = block2::RcBlock::new(move |_granted: Bool, error: *mut AnyObject| {
            // SAFETY: UserNotifications passes null or an NSError.
            let result = unsafe { completion(error) };
            if let Some(sender) = sender.lock().ok().and_then(|mut slot| slot.take()) {
                let _ = sender.send(result);
            }
        });
        // SAFETY: as above.
        unsafe {
            let _: () = msg_send![
                &center,
                requestAuthorizationWithOptions: options.raw(),
                completionHandler: &*block
            ];
        }
        Ok(())
    })?;
    receiver.await.map_err(|_| ServiceError::Cancelled)??;
    notification_permission().await
}

/// Schedules `notification`. Scheduling the same id again replaces it.
/// Without permission it is accepted but never shown; check
/// [`notification_permission`] first.
pub async fn schedule_notification(notification: LocalNotification) -> Result<(), ServiceError> {
    notification.validate()?;
    let (sender, receiver) = oneshot::channel();
    objc2::rc::autoreleasepool(|_| -> Result<(), ServiceError> {
        let center = center()?;
        let content_class =
            ffi::class("UNMutableNotificationContent").ok_or(ServiceError::Unsupported)?;
        // SAFETY: building UserNotifications value objects from validated
        // strings; each setter copies its argument.
        unsafe {
            let content: Retained<AnyObject> = msg_send_id![content_class, new];
            let _: () = msg_send![&content, setTitle: &*ffi::string(&notification.title)];
            let _: () = msg_send![&content, setBody: &*ffi::string(&notification.body)];
            if let Some(subtitle) = &notification.subtitle {
                let _: () = msg_send![&content, setSubtitle: &*ffi::string(subtitle)];
            }
            if let Some(thread) = &notification.thread {
                let _: () = msg_send![&content, setThreadIdentifier: &*ffi::string(thread)];
            }
            if notification.sound {
                let sound: Retained<AnyObject> =
                    msg_send_id![objc2::class!(UNNotificationSound), defaultSound];
                let _: () = msg_send![&content, setSound: &*sound];
            }
            if let Some(badge) = notification.badge {
                let number: Retained<AnyObject> =
                    msg_send_id![objc2::class!(NSNumber), numberWithUnsignedInt: badge];
                let _: () = msg_send![&content, setBadge: &*number];
            }
            if let Some(payload) = &notification.payload {
                let key = ffi::string(PAYLOAD_KEY);
                let value = ffi::string(payload);
                let info: Retained<AnyObject> = msg_send_id![
                    objc2::class!(NSDictionary),
                    dictionaryWithObject: &*value,
                    forKey: &*key
                ];
                let _: () = msg_send![&content, setUserInfo: &*info];
            }
            let trigger: Option<Retained<AnyObject>> = match notification.interval() {
                Some(seconds) => msg_send_id![
                    objc2::class!(UNTimeIntervalNotificationTrigger),
                    triggerWithTimeInterval: seconds,
                    repeats: false
                ],
                None => None,
            };
            let trigger_ptr = trigger.as_ref().map_or(std::ptr::null(), Retained::as_ptr);
            let request: Retained<AnyObject> = msg_send_id![
                objc2::class!(UNNotificationRequest),
                requestWithIdentifier: &*ffi::string(&notification.id),
                content: &*content,
                trigger: trigger_ptr
            ];
            let sender = Mutex::new(Some(sender));
            let block = block2::RcBlock::new(move |error: *mut AnyObject| {
                // SAFETY: UserNotifications passes null or an NSError.
                let result = completion(error);
                if let Some(sender) = sender.lock().ok().and_then(|mut slot| slot.take()) {
                    let _ = sender.send(result);
                }
            });
            let _: () = msg_send![
                &center,
                addNotificationRequest: &*request,
                withCompletionHandler: &*block
            ];
        }
        Ok(())
    })?;
    receiver.await.map_err(|_| ServiceError::Cancelled)?
}

/// Removes pending and delivered notifications with these ids.
pub fn cancel_notifications(ids: &[&str]) -> Result<(), ServiceError> {
    if ids.len() > 256
        || ids
            .iter()
            .any(|id| id.is_empty() || id.len() > limits::NOTIFICATION_ID)
    {
        return Err(ServiceError::Invalid);
    }
    objc2::rc::autoreleasepool(|_| {
        let center = center()?;
        // SAFETY: an NSMutableArray of NSStrings handed to thread-safe
        // removal methods.
        unsafe {
            let array: Retained<AnyObject> = msg_send_id![objc2::class!(NSMutableArray), array];
            for id in ids {
                let _: () = msg_send![&array, addObject: &*ffi::string(id)];
            }
            let _: () =
                msg_send![&center, removePendingNotificationRequestsWithIdentifiers: &*array];
            let _: () = msg_send![&center, removeDeliveredNotificationsWithIdentifiers: &*array];
        }
        Ok(())
    })
}

/// Removes every pending and delivered notification of this app.
pub fn cancel_all_notifications() -> Result<(), ServiceError> {
    objc2::rc::autoreleasepool(|_| {
        let center = center()?;
        // SAFETY: thread-safe UNUserNotificationCenter methods.
        unsafe {
            let _: () = msg_send![&center, removeAllPendingNotificationRequests];
            let _: () = msg_send![&center, removeAllDeliveredNotifications];
        }
        Ok(())
    })
}

/// Sets the app icon's badge (0 clears it). iOS 16 and later.
pub async fn set_badge(count: u32) -> Result<(), ServiceError> {
    let (sender, receiver) = oneshot::channel();
    objc2::rc::autoreleasepool(|_| -> Result<(), ServiceError> {
        let center = center()?;
        // SAFETY: `respondsToSelector:` guards the iOS 16 method.
        unsafe {
            let supported: bool = msg_send![&center, respondsToSelector: objc2::sel!(setBadgeCount:withCompletionHandler:)];
            if !supported {
                return Err(ServiceError::Unsupported);
            }
            let sender = Mutex::new(Some(sender));
            let block = block2::RcBlock::new(move |error: *mut AnyObject| {
                // SAFETY: UserNotifications passes null or an NSError.
                let result = completion(error);
                if let Some(sender) = sender.lock().ok().and_then(|mut slot| slot.take()) {
                    let _ = sender.send(result);
                }
            });
            let _: () = msg_send![
                &center,
                setBadgeCount: count as isize,
                withCompletionHandler: &*block
            ];
        }
        Ok(())
    })?;
    receiver.await.map_err(|_| ServiceError::Cancelled)?
}
