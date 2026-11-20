//! Headless worker JNI boundary; never constructs an Activity, renderer or UI event loop.
#![allow(unsafe_code)]
use super::work::{WorkContext, WorkHost, WorkInput, WorkResult};
pub use jni;
use jni::objects::{JClass, JString, JValue};
use jni::{Env, EnvUnowned, jni_sig, jni_str};
use serde_json::{Value, json};

#[link(name = "android")]
unsafe extern "C" {
    fn android_setsocknetwork(network: u64, fd: i32) -> i32;
}
struct Host<'env, 'local, 'class> {
    env: &'env mut Env<'local>,
    class: &'env JClass<'class>,
    token: i64,
    network: Option<u64>,
}
impl WorkHost for Host<'_, '_, '_> {
    fn bind_socket(&mut self, fd: i32) -> bool {
        // SAFETY: `network` is a handle the platform gave this worker and
        // `fd` a socket the caller owns; the call only binds it.
        self.network
            .is_some_and(|network| unsafe { android_setsocknetwork(network, fd) == 0 })
    }
    fn stopped(&mut self) -> bool {
        self.env
            .call_static_method(
                self.class,
                jni_str!("isCancelled"),
                jni_sig!("(J)Z"),
                &[JValue::Long(self.token)],
            )
            .and_then(jni::objects::JValueOwned::z)
            .unwrap_or(true)
    }
    fn progress(&mut self, value: Value) -> bool {
        let result = (|| {
            let text = JString::new(self.env, value.to_string())?;
            self.env
                .call_static_method(
                    self.class,
                    jni_str!("setProgress"),
                    jni_sig!("(JLjava/lang/String;)Z"),
                    &[JValue::Long(self.token), JValue::Object(text.as_ref())],
                )?
                .z()
        })();
        result.unwrap_or(false)
    }
}
/// Implementation detail used by the background_worker! export macro.
#[doc(hidden)]
pub fn run(
    mut env: EnvUnowned<'_>,
    class: JClass<'_>,
    payload: JString<'_>,
    token: i64,
    handler: fn(&mut WorkContext<'_>) -> WorkResult,
) -> jni::sys::jstring {
    env.with_env(|env| -> jni::errors::Result<jni::sys::jstring> {
        let length = env
            .call_method(&payload, jni_str!("length"), jni_sig!("()I"), &[])?
            .i()?;
        let input = if (0..=32768).contains(&length) {
            serde_json::from_str::<WorkInput>(&payload.try_to_string(env)?).ok()
        } else {
            None
        };
        let outcome = match input {
            Some(input)
                if (1..=4095).contains(&input.id)
                    && input.generation.len() == 36
                    && input.attempt > 0
                    && input.attempt <= 16
                    && input.input.is_object()
                    && input.input.to_string().len() <= 16 * 1024 =>
            {
                let mut host = Host {
                    env,
                    class: &class,
                    token,
                    network: input.network_handle,
                };
                let mut context = WorkContext::new(input, &mut host);
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(&mut context)))
                    .unwrap_or_else(|_| WorkResult::Failure(json!({"code":"panic"})))
                    .wire()
            }
            _ => WorkResult::Failure(json!({"code":"invalidInput"})).wire(),
        };
        if env.exception_check() {
            env.exception_clear();
        }
        Ok(JString::new(env, outcome.to_string())?.into_raw())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Export exactly one headless Rust background worker dispatcher in the app's cdylib.
#[macro_export]
macro_rules! android_background_worker {
    ($handler:path) => {
        #[cfg(target_os = "android")]
        #[unsafe(no_mangle)]
        pub extern "system" fn Java_com_theaimartco_taim_TaimWorkService_nativeRunWork(
            env: $crate::android::__work_bridge::jni::EnvUnowned<'_>,
            class: $crate::android::__work_bridge::jni::objects::JClass<'_>,
            input: $crate::android::__work_bridge::jni::objects::JString<'_>,
            token: $crate::android::__work_bridge::jni::sys::jlong,
        ) -> $crate::android::__work_bridge::jni::sys::jstring {
            $crate::android::__work_bridge::run(env, class, input, token, $handler)
        }
    };
}
