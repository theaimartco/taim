//! What an application asks of the operating system: open a link, share
//! text, vibrate, and where to keep its files.
//!
//! On Android they go through the activity over JNI; on iOS through UIKit;
//! in a browser through the Web APIs; on desktops through the system's own
//! tools. Where a platform has no equivalent, they return
//! [`Error::Unsupported`] rather than pretending.
//!
//! | | Android | iOS | Web | Windows, macOS, Linux |
//! |---|---|---|---|---|
//! | [`open_url`] | yes | yes (main thread) | new tab (after a tap) | yes |
//! | [`share_text`] | share sheet | unsupported | Web Share, where offered | unsupported |
//! | [`vibrate`] | yes | fixed length (iPhone) | where offered (after a tap) | unsupported |
//! | [`data_dir`] | app files | app sandbox | unsupported (`taim::storage` uses `localStorage`) | per-user data |
//!
//! Calls may come from any thread, except `open_url` on iOS, which UIKit
//! requires on the main thread, where views' handlers run.

use std::path::PathBuf;
use std::time::Duration;

/// Why a platform request did not happen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// This platform has no such service.
    #[error("not supported on this platform")]
    Unsupported,
    /// The platform refused or failed.
    #[error("the platform request failed: {0}")]
    Failed(String),
}

/// Opens `url` in the application the system chooses for it: a browser for
/// `https:`, the dialer for `tel:`, mail for `mailto:`.
pub fn open_url(url: &str) -> Result<(), Error> {
    // A leading `-` would reach the desktop opener as an option.
    if url.is_empty() || url.starts_with('-') || url.chars().any(char::is_control) {
        return Err(Error::Failed("not a URL".to_owned()));
    }
    imp::open_url(url)
}

/// Offers `text` to other applications through the system's share sheet.
pub fn share_text(text: &str) -> Result<(), Error> {
    imp::share_text(text)
}

/// Vibrates for `duration` (clamped to five seconds). On Android the
/// application's manifest must ask for `android.permission.VIBRATE`.
pub fn vibrate(duration: Duration) -> Result<(), Error> {
    imp::vibrate(duration.min(Duration::from_secs(5)))
}

/// A directory private to this application where it can keep files, created
/// if it does not exist: the app's internal files directory on Android, the
/// platform's per-user data directory (named after the executable) on
/// desktops.
pub fn data_dir() -> Result<PathBuf, Error> {
    let directory = imp::data_dir()?;
    std::fs::create_dir_all(&directory).map_err(|error| Error::Failed(error.to_string()))?;
    Ok(directory)
}

#[cfg(target_os = "android")]
mod imp {
    use super::Error;

    use jni::JNIEnv;
    use jni::objects::{JObject, JValue};
    use std::path::PathBuf;
    use std::time::Duration;

    /// `Intent.FLAG_ACTIVITY_NEW_TASK`, needed to start an activity from a
    /// context that may not be one.
    const NEW_TASK: i32 = 0x1000_0000;

    /// Runs `body` with a JNI environment and the application's context,
    /// inside a local frame, clearing any Java exception it raised.
    fn with_context<T>(
        body: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>) -> jni::errors::Result<T>,
    ) -> Result<T, Error> {
        let android = ndk_context::android_context();
        if android.vm().is_null() || android.context().is_null() {
            return Err(Error::Unsupported);
        }

        // SAFETY: android-activity publishes the process's JavaVM and the
        // activity through ndk-context before `android_main` runs; both
        // outlive the process's Rust code.
        #[allow(unsafe_code)]
        let vm = unsafe { jni::JavaVM::from_raw(android.vm().cast()) }
            .map_err(|error| Error::Failed(error.to_string()))?;
        #[allow(unsafe_code)]
        let context = unsafe { JObject::from_raw(android.context().cast()) };

        let mut env = vm
            .attach_current_thread_permanently()
            .map_err(|error| Error::Failed(error.to_string()))?;
        let result = env.with_local_frame(16, |env| body(env, &context));

        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_describe();
            let _ = env.exception_clear();
        }
        result.map_err(|error| Error::Failed(error.to_string()))
    }

    fn start(
        env: &mut JNIEnv<'_>,
        context: &JObject<'_>,
        intent: &JObject<'_>,
    ) -> jni::errors::Result<()> {
        let _ = env.call_method(
            intent,
            "addFlags",
            "(I)Landroid/content/Intent;",
            &[JValue::Int(NEW_TASK)],
        )?;
        let _ = env.call_method(
            context,
            "startActivity",
            "(Landroid/content/Intent;)V",
            &[JValue::Object(intent)],
        )?;
        Ok(())
    }

    pub fn open_url(url: &str) -> Result<(), Error> {
        with_context(|env, context| {
            let text = env.new_string(url)?;
            let uri = env
                .call_static_method(
                    "android/net/Uri",
                    "parse",
                    "(Ljava/lang/String;)Landroid/net/Uri;",
                    &[JValue::Object(&text)],
                )?
                .l()?;
            let action = env.new_string("android.intent.action.VIEW")?;
            let intent = env.new_object(
                "android/content/Intent",
                "(Ljava/lang/String;Landroid/net/Uri;)V",
                &[JValue::Object(&action), JValue::Object(&uri)],
            )?;
            start(env, context, &intent)
        })
    }

    pub fn share_text(text: &str) -> Result<(), Error> {
        with_context(|env, context| {
            let action = env.new_string("android.intent.action.SEND")?;
            let intent = env.new_object(
                "android/content/Intent",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&action)],
            )?;
            let kind = env.new_string("text/plain")?;
            let _ = env.call_method(
                &intent,
                "setType",
                "(Ljava/lang/String;)Landroid/content/Intent;",
                &[JValue::Object(&kind)],
            )?;
            let extra = env.new_string("android.intent.extra.TEXT")?;
            let value = env.new_string(text)?;
            let _ = env.call_method(
                &intent,
                "putExtra",
                "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;",
                &[JValue::Object(&extra), JValue::Object(&value)],
            )?;
            let chooser = env
                .call_static_method(
                    "android/content/Intent",
                    "createChooser",
                    "(Landroid/content/Intent;Ljava/lang/CharSequence;)Landroid/content/Intent;",
                    &[JValue::Object(&intent), JValue::Object(&JObject::null())],
                )?
                .l()?;
            start(env, context, &chooser)
        })
    }

    pub fn vibrate(duration: Duration) -> Result<(), Error> {
        let millis = i64::try_from(duration.as_millis())
            .unwrap_or(i64::MAX)
            .max(1);

        with_context(|env, context| {
            let name = env.new_string("vibrator")?;
            let vibrator = env
                .call_method(
                    context,
                    "getSystemService",
                    "(Ljava/lang/String;)Ljava/lang/Object;",
                    &[JValue::Object(&name)],
                )?
                .l()?;
            if vibrator.is_null() {
                return Err(jni::errors::Error::NullPtr("Vibrator"));
            }
            // API 26: VibrationEffect.createOneShot(ms, DEFAULT_AMPLITUDE).
            let effect = env
                .call_static_method(
                    "android/os/VibrationEffect",
                    "createOneShot",
                    "(JI)Landroid/os/VibrationEffect;",
                    &[JValue::Long(millis), JValue::Int(-1)],
                )?
                .l()?;
            let _ = env.call_method(
                &vibrator,
                "vibrate",
                "(Landroid/os/VibrationEffect;)V",
                &[JValue::Object(&effect)],
            )?;
            Ok(())
        })
    }

    pub fn data_dir() -> Result<PathBuf, Error> {
        with_context(|env, context| {
            let files = env
                .call_method(context, "getFilesDir", "()Ljava/io/File;", &[])?
                .l()?;
            let path = env
                .call_method(&files, "getAbsolutePath", "()Ljava/lang/String;", &[])?
                .l()?;
            let path: String = env.get_string(&path.into())?.into();
            Ok(PathBuf::from(path))
        })
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
mod imp {
    use super::Error;

    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    pub fn open_url(url: &str) -> Result<(), Error> {
        let mut command = if cfg!(target_os = "macos") {
            let mut command = Command::new("open");
            let _ = command.arg(url);
            command
        } else if cfg!(target_os = "windows") {
            // Not `cmd /C start`: cmd would split the URL at `&` and run
            // what follows as a command. The URL protocol handler takes
            // the URL as one argument, with no shell in between.
            let mut command = Command::new("rundll32");
            let _ = command.args(["url.dll,FileProtocolHandler", url]);
            command
        } else {
            let mut command = Command::new("xdg-open");
            let _ = command.arg(url);
            command
        };

        let _child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| Error::Failed(error.to_string()))?;
        Ok(())
    }

    pub fn share_text(_text: &str) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    pub fn vibrate(_duration: Duration) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    pub fn data_dir() -> Result<PathBuf, Error> {
        let name = std::env::current_exe()
            .ok()
            .and_then(|path| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "taim-app".to_owned());
        let home = || std::env::var_os("HOME").map(PathBuf::from);

        let base = if cfg!(target_os = "windows") {
            std::env::var_os("APPDATA").map(PathBuf::from)
        } else if cfg!(target_os = "macos") {
            home().map(|home| home.join("Library/Application Support"))
        } else {
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .or_else(|| home().map(|home| home.join(".local/share")))
        };

        base.map(|base| base.join(name))
            .ok_or_else(|| Error::Failed("no home directory".to_owned()))
    }
}

#[cfg(target_os = "ios")]
// UIKit and AudioToolbox are C and Objective-C APIs; each call says why
// it is sound.
#[allow(unsafe_code)]
mod imp {
    use super::Error;

    use std::path::PathBuf;
    use std::time::Duration;

    use objc2_foundation::{MainThreadMarker, NSDictionary, NSString, NSURL};
    use objc2_ui_kit::UIApplication;

    /// Hands `url` to the system (Safari, Phone, Mail). UIKit must be
    /// asked from the main thread, where views' handlers run.
    pub fn open_url(url: &str) -> Result<(), Error> {
        let Some(mtm) = MainThreadMarker::new() else {
            return Err(Error::Failed(
                "open_url must be called from the main thread on iOS".to_owned(),
            ));
        };

        let string = NSString::from_str(url);
        // SAFETY: `URLWithString:` takes any string and returns nil for
        // one that is not a URL.
        let Some(url) = (unsafe { NSURL::URLWithString(&string) }) else {
            return Err(Error::Failed("not a URL".to_owned()));
        };

        let application = UIApplication::sharedApplication(mtm);
        let options = NSDictionary::new();
        // SAFETY: on the main thread, with a valid URL, empty options and
        // no completion handler, as UIKit documents.
        unsafe {
            application.openURL_options_completionHandler(&url, &options, None);
        }
        Ok(())
    }

    /// Presenting a share sheet needs the application's view controller,
    /// which TAIM does not own on iOS yet.
    pub fn share_text(_text: &str) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    #[link(name = "AudioToolbox", kind = "framework")]
    unsafe extern "C" {
        fn AudioServicesPlaySystemSound(sound: u32);
    }

    /// The system vibration (`kSystemSoundID_Vibrate`). Its length is fixed
    /// by iOS; `duration` is ignored. iPads do not vibrate.
    pub fn vibrate(_duration: Duration) -> Result<(), Error> {
        const VIBRATE: u32 = 4095;
        // SAFETY: plays a built-in system sound; any thread may call it.
        unsafe { AudioServicesPlaySystemSound(VIBRATE) };
        Ok(())
    }

    /// `Library/Application Support` in the application's sandbox, which
    /// iOS backs up and keeps until the app is deleted.
    pub fn data_dir() -> Result<PathBuf, Error> {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("Library/Application Support"))
            .ok_or_else(|| Error::Failed("no sandbox home directory".to_owned()))
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::Error;

    use std::path::PathBuf;
    use std::time::Duration;

    fn window() -> Result<web_sys::Window, Error> {
        web_sys::window().ok_or(Error::Unsupported)
    }

    fn failed(error: wasm_bindgen::JsValue) -> Error {
        Error::Failed(format!("{error:?}"))
    }

    /// Opens a new tab (or the handler for `mailto:`, `tel:`). Browsers
    /// block it outside a user gesture such as a click.
    pub fn open_url(url: &str) -> Result<(), Error> {
        let opened = window()?
            .open_with_url_and_target_and_features(url, "_blank", "noopener")
            .map_err(failed)?;
        // `noopener` makes `open` return null even when it worked, so only
        // an exception counts as a failure.
        let _ = opened;
        Ok(())
    }

    /// The Web Share API, where the browser has it (most mobile browsers).
    pub fn share_text(text: &str) -> Result<(), Error> {
        let navigator = window()?.navigator();
        let has_share = js_sys::Reflect::has(&navigator, &"share".into()).unwrap_or(false);
        if !has_share {
            return Err(Error::Unsupported);
        }

        let data = web_sys::ShareData::new();
        data.set_text(text);
        // The promise settles after the user picks a target or cancels;
        // neither needs an answer here.
        let _promise = navigator.share_with_data(&data);
        Ok(())
    }

    /// The Vibration API (Android browsers; not Safari).
    pub fn vibrate(duration: Duration) -> Result<(), Error> {
        let navigator = window()?.navigator();
        let has_vibrate = js_sys::Reflect::has(&navigator, &"vibrate".into()).unwrap_or(false);
        if !has_vibrate {
            return Err(Error::Unsupported);
        }
        let milliseconds = u32::try_from(duration.as_millis()).unwrap_or(u32::MAX);
        if navigator.vibrate_with_duration(milliseconds) {
            Ok(())
        } else {
            // Browsers refuse until the user has tapped the page.
            Err(Error::Failed(
                "the browser refused: vibration needs a tap on the page first".to_owned(),
            ))
        }
    }

    /// A page has no file system; [`web_storage`](super::web_storage) keeps
    /// settings instead.
    pub fn data_dir() -> Result<PathBuf, Error> {
        Err(Error::Unsupported)
    }
}

/// The page's `localStorage`, where `taim::storage` keeps settings on the
/// web: per origin, kept across visits, a few megabytes at most.
#[cfg(target_arch = "wasm32")]
pub mod web_storage {
    use super::Error;

    fn storage() -> Result<web_sys::Storage, Error> {
        web_sys::window()
            .and_then(|window| window.local_storage().ok().flatten())
            .ok_or(Error::Unsupported)
    }

    /// The value saved under `key`, if any.
    pub fn read(key: &str) -> Result<Option<String>, Error> {
        storage()?
            .get_item(key)
            .map_err(|error| Error::Failed(format!("{error:?}")))
    }

    /// Saves `value` under `key`. Fails when the browser's quota is full or
    /// storage is disabled (some private windows).
    pub fn write(key: &str, value: &str) -> Result<(), Error> {
        storage()?
            .set_item(key, value)
            .map_err(|error| Error::Failed(format!("{error:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_with_control_characters_is_refused_before_reaching_the_system() {
        assert!(matches!(
            open_url("https://x\n--evil"),
            Err(Error::Failed(_))
        ));
        assert!(matches!(open_url(""), Err(Error::Failed(_))));
        assert!(matches!(open_url("--help"), Err(Error::Failed(_))));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_data_dir_follows_xdg_and_is_created() {
        let base = std::env::temp_dir().join(format!("taim-data-{}", std::process::id()));
        // SAFETY of the environment: this test is the only one that reads
        // XDG_DATA_HOME in this process.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("XDG_DATA_HOME", &base);
        }

        let directory = data_dir().expect("a data directory");
        assert!(directory.starts_with(&base));
        assert!(directory.is_dir());

        let _ = std::fs::remove_dir_all(&base);
    }
}
