//! Face ID, Touch ID and Optic ID through LocalAuthentication.

use futures::channel::oneshot;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{msg_send, msg_send_id};

use super::ffi::{self, Shared};
use super::types::{BiometricPolicy, BiometricStatus, Biometry, ServiceError, la_error, limits};

#[link(name = "LocalAuthentication", kind = "framework")]
unsafe extern "C" {
    static LAErrorDomain: *const AnyObject;
}

fn context() -> Result<Retained<AnyObject>, ServiceError> {
    // Referencing the framework's symbol keeps it linked.
    // SAFETY: a constant NSString exported by LocalAuthentication.
    let _ = unsafe { LAErrorDomain };
    let class = ffi::class("LAContext").ok_or(ServiceError::Unsupported)?;
    // SAFETY: `new` on LAContext returns a retained context.
    let context: Option<Retained<AnyObject>> = unsafe { msg_send_id![class, new] };
    context.ok_or(ServiceError::Failed)
}

/// Whether a prompt with `policy` could succeed now, without showing UI.
/// Any thread may call it.
pub fn biometric_status(policy: BiometricPolicy) -> BiometricStatus {
    objc2::rc::autoreleasepool(|_| {
        let context = match context() {
            Ok(context) => context,
            Err(error) => {
                return BiometricStatus {
                    biometry: Biometry::None,
                    available: Err(error),
                };
            }
        };
        let mut error: *mut AnyObject = std::ptr::null_mut();
        // SAFETY: LAContext is documented as usable from any thread; the
        // error out-parameter receives an autoreleased NSError or stays nil.
        let (can, raw_biometry) = unsafe {
            let can: bool = msg_send![&context, canEvaluatePolicy: policy.raw(), error: &mut error];
            let raw: isize = msg_send![&context, biometryType];
            (can, raw)
        };
        let available = if can {
            Ok(())
        } else {
            // SAFETY: `error` is null or the NSError LocalAuthentication set.
            Err(unsafe { ffi::error_code(error) }.map_or(ServiceError::Failed, la_error))
        };
        BiometricStatus {
            biometry: Biometry::from_raw(raw_biometry),
            available,
        }
    })
}

/// Cancels the prompt when the waiting future is dropped.
struct Evaluation(Shared);

impl Drop for Evaluation {
    fn drop(&mut self) {
        // SAFETY: LAContext is thread-safe and `invalidate` is idempotent; it
        // dismisses a prompt still showing.
        unsafe {
            let _: () = msg_send![&self.0.0, invalidate];
        }
    }
}

/// Shows the system biometric prompt with `reason` (Touch ID shows it; Face
/// ID shows the Info.plist's `NSFaceIDUsageDescription`).
///
/// Succeeds when the user authenticated. Cancelling is
/// [`ServiceError::Denied`]; the system interrupting is
/// [`ServiceError::Cancelled`]. Dropping the future dismisses the prompt.
/// Success proves local presence only; it is not a server credential.
pub async fn authenticate(reason: &str, policy: BiometricPolicy) -> Result<(), ServiceError> {
    if reason.is_empty() || reason.len() > limits::PROMPT_REASON {
        return Err(ServiceError::Invalid);
    }
    let (sender, receiver) = oneshot::channel::<Result<(), ServiceError>>();
    let evaluation = objc2::rc::autoreleasepool(|_| -> Result<Evaluation, ServiceError> {
        let context = context()?;
        let sender = std::sync::Mutex::new(Some(sender));
        let reply = block2::RcBlock::new(move |success: Bool, error: *mut AnyObject| {
            let result = if success.as_bool() {
                Ok(())
            } else {
                // SAFETY: LocalAuthentication passes nil or an NSError.
                Err(unsafe { ffi::error_code(error) }.map_or(ServiceError::Failed, la_error))
            };
            if let Some(sender) = sender.lock().ok().and_then(|mut slot| slot.take()) {
                let _ = sender.send(result);
            }
        });
        let reason = ffi::string(reason);
        // SAFETY: documented callable from any thread; LocalAuthentication
        // copies the block and calls it once on a private queue.
        unsafe {
            let _: () = msg_send![
                &context,
                evaluatePolicy: policy.raw(),
                localizedReason: &*reason,
                reply: &*reply
            ];
        }
        // SAFETY (Shared): LAContext is thread-safe.
        Ok(Evaluation(Shared(context)))
    })?;
    let result = receiver.await.map_err(|_| ServiceError::Cancelled)?;
    drop(evaluation);
    result
}
