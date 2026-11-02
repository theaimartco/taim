//! The Android clipboard: `android.content.ClipboardManager`, over JNI.
//!
//! Runs on the clipboard worker thread, which attaches itself to the Java VM
//! once. Plain text and HTML are supported; Android exposes no file list or
//! image through the primary clip that a native application can use without
//! a content provider, so those report [`Error::ContentNotAvailable`].

use super::*;

use jni::JNIEnv;
use jni::objects::{JObject, JString, JValue};

pub(super) struct Native {
    vm: jni::JavaVM,
    /// The application's `Context` (a global reference android-activity
    /// keeps alive for the life of the process), as an address so the
    /// worker can own it across threads.
    context: usize,
}

pub(super) fn connect() -> Option<Native> {
    let android = ndk_context::android_context();

    if android.vm().is_null() || android.context().is_null() {
        log::warn!("no Android context; the clipboard is unavailable");
        return None;
    }

    // SAFETY: android-activity publishes the process's JavaVM through
    // ndk-context before `android_main` runs, and it outlives the process.
    #[allow(unsafe_code)]
    let vm = unsafe { jni::JavaVM::from_raw(android.vm().cast()) }.ok()?;

    Some(Native {
        vm,
        context: android.context() as usize,
    })
}

pub(super) fn read_content(native: &mut Native, kind: Kind) -> Result<Content, Error> {
    let html = match kind {
        Kind::Text => false,
        Kind::Html => true,
        _ => return Err(Error::ContentNotAvailable),
    };

    with_manager(native, |env, context, manager| {
        let clip = env
            .call_method(
                manager,
                "getPrimaryClip",
                "()Landroid/content/ClipData;",
                &[],
            )?
            .l()?;
        if clip.is_null() {
            return Ok(None);
        }

        let count = env.call_method(&clip, "getItemCount", "()I", &[])?.i()?;
        if count < 1 {
            return Ok(None);
        }

        let item = env
            .call_method(
                &clip,
                "getItemAt",
                "(I)Landroid/content/ClipData$Item;",
                &[JValue::Int(0)],
            )?
            .l()?;

        let text = if html {
            env.call_method(
                &item,
                "coerceToHtmlText",
                "(Landroid/content/Context;)Ljava/lang/String;",
                &[JValue::Object(context)],
            )?
            .l()?
        } else {
            let sequence = env
                .call_method(
                    &item,
                    "coerceToText",
                    "(Landroid/content/Context;)Ljava/lang/CharSequence;",
                    &[JValue::Object(context)],
                )?
                .l()?;
            if sequence.is_null() {
                return Ok(None);
            }
            env.call_method(&sequence, "toString", "()Ljava/lang/String;", &[])?
                .l()?
        };

        if text.is_null() {
            return Ok(None);
        }

        let text = JString::from(text);
        let text: String = env.get_string(&text)?.into();
        Ok(Some(text))
    })?
    .map(|text| {
        if html {
            Content::Html(text)
        } else {
            Content::Text(text)
        }
    })
    .ok_or(Error::ContentNotAvailable)
}

pub(super) fn write_content(native: &mut Native, content: Content) -> Result<(), Error> {
    let (text, html) = match content {
        Content::Text(text) => (text, None),
        Content::Html(html) => (html.clone(), Some(html)),
        _ => return Err(Error::ClipboardUnavailable),
    };

    with_manager(native, |env, _context, manager| {
        let label = env.new_string("TAIM")?;
        let plain = env.new_string(&text)?;

        let clip = match &html {
            None => env
                .call_static_method(
                    "android/content/ClipData",
                    "newPlainText",
                    "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;",
                    &[JValue::Object(&label), JValue::Object(&plain)],
                )?
                .l()?,
            Some(html) => {
                let html = env.new_string(html)?;
                env.call_static_method(
                    "android/content/ClipData",
                    "newHtmlText",
                    "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;Ljava/lang/String;)Landroid/content/ClipData;",
                    &[
                        JValue::Object(&label),
                        JValue::Object(&plain),
                        JValue::Object(&html),
                    ],
                )?
                .l()?
            }
        };

        let _ = env.call_method(
            manager,
            "setPrimaryClip",
            "(Landroid/content/ClipData;)V",
            &[JValue::Object(&clip)],
        )?;
        Ok(())
    })
}

/// Runs `use_manager` with the `ClipboardManager`, inside a local reference
/// frame so a long-lived worker never accumulates references, and turns a
/// thrown Java exception into an error after clearing it.
fn with_manager<T>(
    native: &mut Native,
    use_manager: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>, &JObject<'_>) -> jni::errors::Result<T>,
) -> Result<T, Error> {
    let mut env = native
        .vm
        .attach_current_thread_permanently()
        .map_err(java_error)?;

    // SAFETY: the context is a global reference that android-activity keeps
    // alive; it is borrowed here, never deleted.
    #[allow(unsafe_code)]
    let context = unsafe { JObject::from_raw(native.context as jni::sys::jobject) };

    let result = env.with_local_frame(16, |env| {
        let service = env.new_string("clipboard")?;
        let manager = env
            .call_method(
                &context,
                "getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                &[JValue::Object(&service)],
            )?
            .l()?;

        if manager.is_null() {
            return Err(jni::errors::Error::NullPtr("ClipboardManager"));
        }

        use_manager(env, &context, &manager)
    });

    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }

    result.map_err(java_error)
}

fn java_error(error: jni::errors::Error) -> Error {
    Error::Unknown {
        description: Arc::new(format!("Android clipboard: {error}")),
    }
}
