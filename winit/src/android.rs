//! Android process setup: logging to logcat and reporting panics there.

use std::sync::Once;

/// Sends `log` records to logcat under the `taim` tag, and panics too,
/// which otherwise vanish: an Android process has no terminal, so a panic
/// on the `android_main` thread would just look like the app closing.
///
/// Leaves an application's own logger in place if it installed one first.
pub(crate) fn init_logging() {
    static ONCE: Once = Once::new();

    ONCE.call_once(|| {
        android_logger::init_once(
            android_logger::Config::default()
                .with_tag("taim")
                .with_max_level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                }),
        );

        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            log::error!("panic: {info}");
            previous(info);
        }));
    });
}

/// TalkBack for one window: AccessKit's adapter, injected as the
/// accessibility delegate of the activity's decor view.
pub(crate) struct TalkBack {
    adapter: accesskit_android::InjectingAdapter,
    /// TalkBack (or another accessibility service) asked for the tree.
    /// Until then nothing is built.
    active: std::sync::Arc<std::sync::atomic::AtomicBool>,
    targets: crate::talkback::Targets,
}

impl TalkBack {
    /// Attaches to the activity's decor view. `None`, with a log record,
    /// if the Java side cannot be reached.
    pub fn attach(
        android: &crate::AndroidApp,
        window: crate::core::window::Id,
        proxy: winit::event_loop::EventLoopProxy<crate::wake::Wake>,
    ) -> Option<Self> {
        use jni::objects::JObject;
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;

        // SAFETY: android-activity hands out the process's JavaVM and the
        // activity's global reference; both outlive the event loop.
        #[allow(unsafe_code)]
        let vm = unsafe { jni::JavaVM::from_raw(android.vm_as_ptr().cast()) }.ok()?;
        #[allow(unsafe_code)]
        let activity = unsafe { JObject::from_raw(android.activity_as_ptr().cast()) };

        let mut env = vm.attach_current_thread_permanently().ok()?;
        let decor = (|| -> jni::errors::Result<JObject<'_>> {
            let window = env
                .call_method(&activity, "getWindow", "()Landroid/view/Window;", &[])?
                .l()?;
            env.call_method(&window, "getDecorView", "()Landroid/view/View;", &[])?
                .l()
        })();

        let decor = match decor {
            Ok(decor) if !decor.is_null() => decor,
            other => {
                let _ = env.exception_clear();
                log::warn!("TalkBack unavailable: no decor view ({:?})", other.err());
                return None;
            }
        };

        let active = Arc::new(AtomicBool::new(false));
        let adapter = accesskit_android::InjectingAdapter::new(
            &mut env,
            &decor,
            Activation {
                active: active.clone(),
                window,
                proxy: proxy.clone(),
            },
            Act { window, proxy },
        );

        Some(Self {
            adapter,
            active,
            targets: crate::talkback::Targets::default(),
        })
    }

    /// Whether an accessibility service is reading the tree.
    pub fn is_active(&self) -> bool {
        self.active.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Publishes the window's semantic tree.
    pub fn publish(&mut self, window: &crate::core::semantics::Node, scale: f32) {
        let (update, targets) = crate::talkback::tree_update(window, scale);
        self.targets = targets;
        self.adapter.update_if_active(|| update);
    }

    /// The node an action names, as last published.
    pub fn target(&self, node: accesskit::NodeId) -> Option<crate::talkback::Target> {
        self.targets.get(&node).copied()
    }
}

/// Answers TalkBack's first request for the tree. It is asked on the UI
/// thread while the tree lives on the event loop's, so it wakes the event
/// loop to publish the full tree at the next frame, as AccessKit allows.
struct Activation {
    active: std::sync::Arc<std::sync::atomic::AtomicBool>,
    window: crate::core::window::Id,
    proxy: winit::event_loop::EventLoopProxy<crate::wake::Wake>,
}

impl accesskit::ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
        self.active
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = self
            .proxy
            .send_event(crate::wake::Wake::Describe(self.window));
        None
    }
}

/// Hands TalkBack's actions (activate, scroll) to the event loop.
struct Act {
    window: crate::core::window::Id,
    proxy: winit::event_loop::EventLoopProxy<crate::wake::Wake>,
}

impl accesskit::ActionHandler for Act {
    fn do_action(&mut self, request: accesskit::ActionRequest) {
        let _ = self
            .proxy
            .send_event(crate::wake::Wake::Accessibility(self.window, request));
    }
}
