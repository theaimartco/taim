//! What an application asks of the operating system: open a link, share
//! text, vibrate, and where to keep its files.
//!
//! Every function can be called from any thread. On Android they go
//! through the activity over JNI; on desktops through the system's own
//! tools. Where a platform has no equivalent, they return
//! [`Error::Unsupported`] rather than pretending.

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

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
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
            // `start`'s first quoted argument is a window title.
            let mut command = Command::new("cmd");
            let _ = command.args(["/C", "start", "", url]);
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

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::Error;

    use std::path::PathBuf;
    use std::time::Duration;

    pub fn open_url(_url: &str) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    pub fn share_text(_text: &str) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    pub fn vibrate(_duration: Duration) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    pub fn data_dir() -> Result<PathBuf, Error> {
        Err(Error::Unsupported)
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
