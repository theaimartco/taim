//! Secrets in the keychain: generic-password items, kept on this device.
//!
//! These calls block. Reading an item whose [`KeychainAccess`] prompts shows
//! Face ID or the passcode and blocks until the user answers, so call them
//! from a background task, never from a view's handler.

use std::ffi::c_void;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, msg_send_id};

use super::ffi;
use super::types::{KeychainAccess, ServiceError, keychain_status, limits, validate_keychain};

#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    static kSecClass: *const AnyObject;
    static kSecClassGenericPassword: *const AnyObject;
    static kSecAttrService: *const AnyObject;
    static kSecAttrAccount: *const AnyObject;
    static kSecValueData: *const AnyObject;
    static kSecReturnData: *const AnyObject;
    static kSecMatchLimit: *const AnyObject;
    static kSecMatchLimitOne: *const AnyObject;
    static kSecAttrAccessible: *const AnyObject;
    static kSecAttrAccessControl: *const AnyObject;
    static kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly: *const AnyObject;
    static kSecAttrAccessibleWhenUnlockedThisDeviceOnly: *const AnyObject;
    static kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly: *const AnyObject;
    static kSecUseOperationPrompt: *const AnyObject;
    #[cfg(target_os = "macos")]
    static kSecUseDataProtectionKeychain: *const AnyObject;

    fn SecItemAdd(attributes: *const AnyObject, result: *mut *const c_void) -> i32;
    fn SecItemCopyMatching(query: *const AnyObject, result: *mut *const c_void) -> i32;
    fn SecItemDelete(query: *const AnyObject) -> i32;
    fn SecAccessControlCreateWithFlags(
        allocator: *const c_void,
        protection: *const AnyObject,
        flags: usize,
        error: *mut *mut c_void,
    ) -> *const AnyObject;
    fn CFRelease(object: *const c_void);
}

const NOT_FOUND: i32 = -25300;

/// Which keychain a query addresses. iOS has only the data-protection
/// keychain; macOS also has the older login keychain, used when the app is
/// not signed with a keychain access group.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Store {
    DataProtection,
    #[cfg_attr(target_os = "ios", allow(dead_code))]
    Login,
}

/// Runs `body` against the data-protection keychain and, on a Mac whose
/// signature lacks a keychain access group, against the login keychain.
fn attempt<T>(body: impl Fn(Store) -> Result<T, ServiceError>) -> Result<T, ServiceError> {
    match body(Store::DataProtection) {
        #[cfg(target_os = "macos")]
        Err(ServiceError::MissingEntitlement) => body(Store::Login),
        result => result,
    }
}

/// Like [`attempt`], but on a Mac also consults the login keychain when the
/// data-protection keychain has no item: searching it without an access
/// group reports "not found" rather than a missing entitlement.
fn attempt_lookup<T>(
    body: impl Fn(Store) -> Result<T, ServiceError>,
    #[cfg_attr(target_os = "ios", allow(unused_variables))] found: impl Fn(&T) -> bool,
) -> Result<T, ServiceError> {
    match body(Store::DataProtection) {
        #[cfg(target_os = "macos")]
        Err(ServiceError::MissingEntitlement) => body(Store::Login),
        #[cfg(target_os = "macos")]
        Ok(value) if !found(&value) => body(Store::Login),
        result => result,
    }
}

/// A mutable dictionary for a query.
struct Query(Retained<AnyObject>);

impl Query {
    fn new(service: &str, account: &str, store: Store) -> Self {
        // SAFETY: NSMutableDictionary is toll-free bridged to the
        // CFMutableDictionary the Security framework takes; the keys are
        // its exported constants.
        unsafe {
            let dictionary: Retained<AnyObject> =
                msg_send_id![objc2::class!(NSMutableDictionary), dictionary];
            let query = Self(dictionary);
            query.set(kSecClass, kSecClassGenericPassword);
            query.set(
                kSecAttrService,
                Retained::as_ptr(&ffi::string(service)).cast(),
            );
            query.set(
                kSecAttrAccount,
                Retained::as_ptr(&ffi::string(account)).cast(),
            );
            #[cfg(target_os = "macos")]
            if store == Store::DataProtection {
                let yes: Retained<AnyObject> =
                    msg_send_id![objc2::class!(NSNumber), numberWithBool: true];
                query.set(kSecUseDataProtectionKeychain, Retained::as_ptr(&yes));
            }
            #[cfg(target_os = "ios")]
            let _ = store;
            query
        }
    }

    /// # Safety
    /// `key` and `value` must be live Objective-C (or bridged CF) objects.
    unsafe fn set(&self, key: *const AnyObject, value: *const AnyObject) {
        // SAFETY: the dictionary copies the key and retains the value.
        unsafe {
            let _: () = msg_send![&self.0, setObject: value, forKey: key];
        }
    }

    fn as_ptr(&self) -> *const AnyObject {
        Retained::as_ptr(&self.0)
    }
}

/// Stores `secret` under `service`/`account`, replacing any existing item.
///
/// The existing item is kept when the new one cannot be stored: a trial
/// copy is added under a staging account first, so a missing passcode or
/// entitlement fails before anything is deleted.
pub fn keychain_set(
    service: &str,
    account: &str,
    secret: &[u8],
    access: KeychainAccess,
) -> Result<(), ServiceError> {
    validate_keychain(service, account, Some(secret))?;
    let staging = format!("{account}.taim-staging");
    let _ = delete_item(service, &staging);
    add_item(service, &staging, secret, access)?;
    let _ = delete_item(service, &staging);
    let _ = delete_item(service, account)?;
    add_item(service, account, secret, access)
}

fn delete_item(service: &str, account: &str) -> Result<bool, ServiceError> {
    attempt_lookup(
        |store| {
            objc2::rc::autoreleasepool(|_| {
                let query = Query::new(service, account, store);
                // SAFETY: a query dictionary built from Security constants.
                let status = unsafe { SecItemDelete(query.as_ptr()) };
                if status == NOT_FOUND {
                    return Ok(false);
                }
                keychain_status(status).map(|()| true)
            })
        },
        |existed| *existed,
    )
}

fn add_item(
    service: &str,
    account: &str,
    secret: &[u8],
    access: KeychainAccess,
) -> Result<(), ServiceError> {
    attempt(|store| {
        objc2::rc::autoreleasepool(|_| {
            // The login keychain has no per-item access control.
            if store == Store::Login && access.prompts() {
                return Err(ServiceError::MissingEntitlement);
            }
            let query = Query::new(service, account, store);
            // SAFETY: Security framework constants and objects created here;
            // the access-control object is released after the dictionary
            // retained it.
            unsafe {
                let data: Retained<AnyObject> = msg_send_id![
                    objc2::class!(NSData),
                    dataWithBytes: secret.as_ptr().cast::<c_void>(),
                    length: secret.len()
                ];
                query.set(kSecValueData, Retained::as_ptr(&data));
                match access.access_control_flags() {
                    None => {
                        let protection = match access {
                            KeychainAccess::WhenUnlocked => {
                                kSecAttrAccessibleWhenUnlockedThisDeviceOnly
                            }
                            _ => kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
                        };
                        if store == Store::DataProtection {
                            query.set(kSecAttrAccessible, protection);
                        }
                    }
                    Some(flags) => {
                        let control = SecAccessControlCreateWithFlags(
                            std::ptr::null(),
                            kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
                            flags,
                            std::ptr::null_mut(),
                        );
                        if control.is_null() {
                            return Err(ServiceError::Failed);
                        }
                        query.set(kSecAttrAccessControl, control);
                        CFRelease(control.cast());
                    }
                }
                let status = SecItemAdd(query.as_ptr(), std::ptr::null_mut());
                // A passcode is required for prompting items.
                if status == -25293 && access.prompts() {
                    return Err(ServiceError::NotEnrolled);
                }
                keychain_status(status)
            }
        })
    })
}

/// Reads the secret under `service`/`account`; `None` if there is none.
/// `prompt` is the reason shown when the item asks for Face ID or the passcode.
pub fn keychain_get(
    service: &str,
    account: &str,
    prompt: Option<&str>,
) -> Result<Option<Vec<u8>>, ServiceError> {
    validate_keychain(service, account, None)?;
    if prompt.is_some_and(|prompt| prompt.len() > limits::PROMPT_REASON) {
        return Err(ServiceError::Invalid);
    }
    attempt_lookup(
        |store| {
            objc2::rc::autoreleasepool(|_| {
                let query = Query::new(service, account, store);
                let mut result: *const c_void = std::ptr::null();
                // SAFETY: as in `keychain_set`. On success the result is a +1
                // CFData (bridged NSData) owned here and released below.
                unsafe {
                    let yes: Retained<AnyObject> =
                        msg_send_id![objc2::class!(NSNumber), numberWithBool: true];
                    query.set(kSecReturnData, Retained::as_ptr(&yes));
                    query.set(kSecMatchLimit, kSecMatchLimitOne);
                    if let Some(prompt) = prompt {
                        query.set(
                            kSecUseOperationPrompt,
                            Retained::as_ptr(&ffi::string(prompt)).cast(),
                        );
                    }
                    let status = SecItemCopyMatching(query.as_ptr(), &mut result);
                    if status == NOT_FOUND {
                        return Ok(None);
                    }
                    keychain_status(status)?;
                    if result.is_null() {
                        return Err(ServiceError::Failed);
                    }
                    let data = result.cast::<AnyObject>();
                    let length: usize = msg_send![data, length];
                    let bytes: *const c_void = msg_send![data, bytes];
                    let bytes = bytes.cast::<u8>();
                    let secret = if length == 0 || bytes.is_null() {
                        Vec::new()
                    } else {
                        std::slice::from_raw_parts(bytes, length).to_vec()
                    };
                    CFRelease(result);
                    Ok(Some(secret))
                }
            })
        },
        Option::is_some,
    )
}

/// Deletes the item under `service`/`account`. Returns whether one existed.
pub fn keychain_delete(service: &str, account: &str) -> Result<bool, ServiceError> {
    validate_keychain(service, account, None)?;
    delete_item(service, account)
}
