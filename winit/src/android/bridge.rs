//! The only JNI boundary in the Android shell. AndroidApp owns the JVM/global
//! activity reference; clones are held until each UI-thread closure completes.
#![allow(unsafe_code)]

use super::geometry::Insets;
use crate::wake::Wake;
use jni::objects::{JClass, JIntArray, JObject, JString, JValue};
use jni::{EnvUnowned, JavaVM, jni_sig, jni_str};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use taim_core::input_method::{EditorState, utf8_to_utf16, utf16_to_utf8};
use taim_core::{Rectangle, widget};
use winit::event_loop::EventLoopProxy;
use winit::platform::android::activity::AndroidApp;

const MAX_TEXT_BYTES: usize = 1024 * 1024;
const MAX_TEXT_UTF16: usize = 262144;
static NEXT: AtomicU64 = AtomicU64::new(1);
static REGISTRY: OnceLock<Mutex<HashMap<u64, Weak<Bridge>>>> = OnceLock::new();

use super::semantics::SemanticCommand;

#[derive(Default)]
pub(crate) struct Pending {
    pub native_blur: bool,
    pub native_focus: Option<(u64, u64, Option<Rectangle>)>,
    pub insets: Option<(Insets, bool, bool)>,
    pub edits: Vec<(u64, u64, EditorState)>,
    pub submit: Option<u64>,
    pub operations: Vec<(u64, taim_core::Event)>,
    pub semantics: Vec<SemanticCommand>,
    pub back: u8,
    pub pressure: Option<i32>,
}
#[derive(Default)]
struct Control {
    native_scene: Option<String>,
    native_scene_posted: bool,
    focus: Option<widget::Id>,
    state: Option<EditorState>,
    composing: bool,
    applied_revision: u64,
    cursor: Option<Rectangle>,
    dark: Option<bool>,
    secure: bool,
    preferences: Option<(Insets, bool, bool)>,
    saved: Option<String>,
    back_enabled: Option<bool>,
    autofill_focus: Option<i32>,
}

pub(crate) struct Bridge {
    app: AndroidApp,
    token: u64,
    semantics: Mutex<super::semantics::Snapshot>,
    pub(super) services: super::services::State,
    owner: AtomicU64,
    proxy: EventLoopProxy<Wake>,
    pending: Mutex<Pending>,
    queued: AtomicBool,
    control: Mutex<Control>,
}

impl Bridge {
    pub fn native_key(
        &self,
        id: u64,
        generation: u64,
        key: i32,
        pressed: bool,
        repeat: bool,
        text: &str,
    ) {
        let token = self.token;
        let text = text.to_owned();
        self.call(move |env, activity| {
            let text = JString::new(env, text)?;
            let _ = env.call_method(
                activity,
                jni_str!("dispatchTaimNativeKey"),
                jni_sig!("(JJJIZZLjava/lang/String;)V"),
                &[
                    JValue::Long(token as i64),
                    JValue::Long(id as i64),
                    JValue::Long(generation as i64),
                    JValue::Int(key),
                    JValue::Bool(pressed),
                    JValue::Bool(repeat),
                    JValue::Object(text.as_ref()),
                ],
            )?;
            Ok(())
        });
    }
    pub fn native_touch(&self, id: u64, phase: i32, x: f32, y: f32) {
        let token = self.token;
        self.call(move |env, activity| {
            let _ = env.call_method(
                activity,
                jni_str!("dispatchTaimNativeTouch"),
                jni_sig!("(JJIFF)V"),
                &[
                    JValue::Long(token as i64),
                    JValue::Long(id as i64),
                    JValue::Int(phase),
                    JValue::Float(x),
                    JValue::Float(y),
                ],
            )?;
            Ok(())
        });
    }
    pub fn publish_native_views(&self, scene: serde_json::Value) {
        let encoded = scene.to_string();
        let mut control = self
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if control.native_scene.as_ref() == Some(&encoded) {
            return;
        }
        control.native_scene = Some(encoded);
        if control.native_scene_posted {
            return;
        }
        control.native_scene_posted = true;
        drop(control);
        let token = self.token;
        self.call(move |env, activity| {
            let Some(bridge) = lookup(token as i64) else {
                return Ok(());
            };
            let mut control = bridge
                .control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let encoded = control.native_scene.clone().unwrap_or_default();
            control.native_scene_posted = false;
            drop(control);
            let payload = JString::new(env, encoded)?;
            let _ = env.call_method(
                activity,
                jni_str!("setTaimNativeViews"),
                jni_sig!("(JLjava/lang/String;)V"),
                &[JValue::Long(token as i64), JValue::Object(payload.as_ref())],
            )?;
            Ok(())
        });
    }
    pub fn new(app: AndroidApp, proxy: EventLoopProxy<Wake>) -> Arc<Self> {
        let bridge = Arc::new(Self {
            app,
            token: NEXT.fetch_add(1, Ordering::Relaxed),
            semantics: Mutex::default(),
            services: super::services::State::default(),
            owner: AtomicU64::new(0),
            proxy,
            pending: Mutex::new(Pending::default()),
            queued: AtomicBool::new(false),
            control: Mutex::new(Control::default()),
        });
        let _ = REGISTRY
            .get_or_init(Mutex::default)
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(bridge.token, Arc::downgrade(&bridge));
        let watchdog = Arc::downgrade(&bridge);
        let timer = bridge.services.timer.clone();
        let _ = std::thread::Builder::new()
            .name("taim-android-deadlines".into())
            .spawn(move || {
                let mut epoch = timer.epoch();
                loop {
                    let Some(bridge) = watchdog.upgrade() else {
                        break;
                    };
                    let delay = bridge.services.expire();
                    drop(bridge);
                    epoch = timer.wait(epoch, delay);
                }
            });
        let weak = Arc::downgrade(&bridge);
        taim_graphics::text::android::set_color_glyph_rasterizer(Arc::new(move |glyph| {
            weak.upgrade()?.rasterize(glyph)
        }));
        let token = bridge.token;
        bridge.call(move |env, activity| {
            let _ = env.call_method(
                activity,
                jni_str!("attachTaim"),
                jni_sig!("(J)V"),
                &[JValue::Long(token as i64)],
            )?;
            Ok(())
        });
        bridge
    }

    fn call(
        &self,
        callback: impl for<'env> FnOnce(&mut jni::Env<'env>, &JObject<'env>) -> jni::errors::Result<()>
        + Send
        + 'static,
    ) {
        let app = self.app.clone();
        self.app.run_on_java_main_thread(Box::new(move || {
            // SAFETY: AndroidApp keeps these JVM and global activity references
            // alive. Create a local JNI reference for this attached-thread frame.
            let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
            let raw = app.activity_as_ptr() as jni::sys::jobject;
            let result = vm.attach_current_thread(|env| {
                let borrowed = unsafe { env.as_cast_raw::<JObject<'_>>(&raw)? };
                let activity = env.new_local_ref(borrowed.as_ref())?;
                let result = callback(env, &activity);
                if env.exception_check() {
                    env.exception_clear();
                }
                result
            });
            if let Err(error) = result {
                log::error!(
                    "Android host bridge: {error}; package com.theaimartco.taim.TaimActivity"
                );
            }
        }));
    }

    pub(super) fn stop_location(&self, watch: u64) {
        let token = self.token;
        self.call(move |env, activity| {
            let _ = env.call_method(
                activity,
                jni_str!("stopTaimLocation"),
                jni_sig!("(JJ)V"),
                &[JValue::Long(token as i64), JValue::Long(watch as i64)],
            )?;
            Ok(())
        });
    }

    fn rasterize(
        &self,
        glyph: taim_graphics::text::android::ColorGlyph,
    ) -> Option<taim_graphics::text::cosmic_text::SwashImage> {
        use taim_graphics::text::cosmic_text::{Placement, SwashContent, SwashImage};
        // SAFETY: self owns AndroidApp for the full attached-thread scope.
        let vm = unsafe { JavaVM::from_raw(self.app.vm_as_ptr().cast()) };
        let raw = self.app.activity_as_ptr() as jni::sys::jobject;
        let result = vm.attach_current_thread(|env| -> jni::errors::Result<Option<SwashImage>> {
            let result = (|| -> jni::errors::Result<Option<SwashImage>> {
                let borrowed = unsafe { env.as_cast_raw::<JObject<'_>>(&raw)? };
                let activity = env.new_local_ref(borrowed.as_ref())?;
                let file = JString::new(env, glyph.file.to_string_lossy())?;
                let result = env
                    .call_method(
                        &activity,
                        jni_str!("rasterizeTaimGlyph"),
                        jni_sig!("(Ljava/lang/String;IIFFF)[I"),
                        &[
                            JValue::Object(file.as_ref()),
                            JValue::Int(glyph.index as i32),
                            JValue::Int(i32::from(glyph.glyph)),
                            JValue::Float(glyph.size),
                            JValue::Float(glyph.x),
                            JValue::Float(glyph.y),
                        ],
                    )?
                    .l()?;
                if result.is_null() {
                    return Ok(None);
                }
                let array = env.cast_local::<JIntArray<'_>>(result)?;
                let length = array.len(env)?;
                if !(4..=1024 * 1024 + 4).contains(&length) {
                    return Ok(None);
                }
                let mut header = [0; 4];
                array.get_region(env, 0, &mut header)?;
                let [left, top, width, height] = header;
                if width <= 0
                    || height <= 0
                    || width > 1024
                    || height > 1024
                    || length != 4 + (width * height) as usize
                {
                    return Ok(None);
                }
                let mut pixels = vec![0; length - 4];
                array.get_region(env, 4, &mut pixels)?;
                let mut data = Vec::with_capacity(pixels.len() * 4);
                for pixel in pixels {
                    let [alpha, red, green, blue] = (pixel as u32).to_be_bytes();
                    data.extend_from_slice(&[red, green, blue, alpha]);
                }
                Ok(Some(SwashImage {
                    content: SwashContent::Color,
                    placement: Placement {
                        left,
                        top,
                        width: width as u32,
                        height: height as u32,
                    },
                    data,
                    ..Default::default()
                }))
            })();
            if env.exception_check() {
                env.exception_clear();
            }
            result
        });
        match result {
            Ok(image) => image,
            Err(error) => {
                log::debug!("Android color glyph unavailable: {error}");
                None
            }
        }
    }

    pub fn publish_semantics(
        &self,
        tree: &taim_accessibility::Tree,
        origin: taim_core::Point,
        scale: f32,
        clip: Rectangle,
        editor: Option<&(widget::Id, taim_core::input_method::EditorState)>,
    ) {
        let payload = self
            .semantics
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .update_with_editor(tree, origin, scale, clip, editor);
        if let Some(payload) = payload {
            let token = self.token;
            self.call(move |env, activity| {
                let text = JString::new(env, payload)?;
                let _ = env.call_method(
                    activity,
                    jni_str!("setTaimSemantics"),
                    jni_sig!("(JLjava/lang/String;)V"),
                    &[JValue::Long(token as i64), JValue::Object(text.as_ref())],
                )?;
                Ok(())
            });
        }
    }

    pub fn autofill_focus(&self, target: Option<widget::Id>) {
        let id = target
            .and_then(|target| {
                self.semantics
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .autofill_id(&target)
            })
            .unwrap_or(-1);
        let mut control = self
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if control.autofill_focus == Some(id) {
            return;
        }
        control.autofill_focus = Some(id);
        drop(control);
        self.call(move |env, activity| {
            let _ = env.call_method(
                activity,
                jni_str!("setTaimAutofillFocus"),
                jni_sig!("(I)V"),
                &[JValue::Int(id)],
            )?;
            Ok(())
        });
    }

    pub fn hover(&self, x: f32, y: f32, exit: bool) {
        self.call(move |env, activity| {
            let _ = env.call_method(
                activity,
                jni_str!("hoverTaim"),
                jni_sig!("(FFZ)V"),
                &[JValue::Float(x), JValue::Float(y), JValue::Bool(exit)],
            )?;
            Ok(())
        });
    }

    pub(super) fn request_service(
        &self,
        payload: serde_json::Value,
    ) -> Result<(u64, super::services::ServiceReceiver), super::ServiceError> {
        static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);
        let id = NEXT_REQUEST.fetch_add(1, Ordering::Relaxed);
        let receive = self.services.begin(id)?;
        let token = self.token;
        self.call(move |env, activity| {
            let text = JString::new(env, payload.to_string())?;
            let result = env.call_method(
                activity,
                jni_str!("requestTaimService"),
                jni_sig!("(JJLjava/lang/String;)V"),
                &[
                    JValue::Long(token as i64),
                    JValue::Long(id as i64),
                    JValue::Object(text.as_ref()),
                ],
            );
            if result.is_err()
                && let Some(bridge) = lookup(token as i64)
            {
                bridge
                    .services
                    .complete(id, Err(super::ServiceError::Unavailable));
            }
            let _ = result?;
            Ok(())
        });
        Ok((id, receive))
    }

    pub(super) fn cancel_location_request(&self, id: u64) {
        let token = self.token;
        self.call(move |env, activity| {
            let _ = env.call_method(
                activity,
                jni_str!("cancelTaimLocationRequest"),
                jni_sig!("(JJ)V"),
                &[JValue::Long(token as i64), JValue::Long(id as i64)],
            )?;
            Ok(())
        });
    }

    pub fn restored_state(&self) -> Option<String> {
        let vm = unsafe { JavaVM::from_raw(self.app.vm_as_ptr().cast()) };
        let raw = self.app.activity_as_ptr() as jni::sys::jobject;
        let result = vm.attach_current_thread(|env| -> jni::errors::Result<Option<String>> {
            let result = (|| {
                let borrowed = unsafe { env.as_cast_raw::<JObject<'_>>(&raw)? };
                let object = env.new_local_ref(borrowed.as_ref())?;
                let value = env
                    .call_method(
                        &object,
                        jni_str!("getTaimRestoration"),
                        jni_sig!("()Ljava/lang/String;"),
                        &[],
                    )?
                    .l()?;
                if value.is_null() {
                    return Ok(None);
                }
                let text = env.cast_local::<JString<'_>>(value)?;
                if string_units(env, &text)? > 1024 * 1024 {
                    return Ok(None);
                }
                Ok(Some(text.try_to_string(env)?))
            })();
            if env.exception_check() {
                env.exception_clear();
            }
            result
        });
        match result {
            Ok(state) => state,
            Err(error) => {
                log::warn!("Android restoration read failed: {error}");
                None
            }
        }
    }

    pub fn checkpoint(&self, state: String) {
        let mut control = self
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if control.saved.as_ref() == Some(&state) {
            return;
        }
        control.saved = Some(state.clone());
        drop(control);
        let token = self.token;
        self.call(move |env, activity| {
            let text = JString::new(env, state)?;
            let _ = env.call_method(
                activity,
                jni_str!("setTaimRestoration"),
                jni_sig!("(JLjava/lang/String;)V"),
                &[JValue::Long(token as i64), JValue::Object(text.as_ref())],
            )?;
            Ok(())
        });
    }

    pub fn back_enabled(&self, enabled: bool) {
        let mut control = self
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if control.back_enabled == Some(enabled) {
            return;
        }
        control.back_enabled = Some(enabled);
        drop(control);
        self.call(move |env, activity| {
            let _ = env.call_method(
                activity,
                jni_str!("setTaimBackEnabled"),
                jni_sig!("(Z)V"),
                &[JValue::Bool(enabled)],
            )?;
            Ok(())
        });
    }
    pub fn graphics_restored(&self) {
        self.services.event(super::PlatformEvent::GraphicsRestored);
    }
    pub fn finish(&self) {
        self.call(|env, activity| {
            let _ = env.call_method(
                activity,
                jni_str!("finishAfterTransition"),
                jni_sig!("()V"),
                &[],
            )?;
            Ok(())
        });
    }

    fn wake(&self) {
        if !self.queued.swap(true, Ordering::AcqRel) {
            let _ = self.proxy.send_event(Wake::Android);
        }
    }
    /// Acknowledge only after the view tree has consumed this native snapshot.
    pub fn acknowledge_edit(&self, owner: u64, revision: u64) {
        let mut control = self
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.owner() == owner {
            control.applied_revision = revision;
            // A prior queued snapshot may have been rejected by the UI thread.
            control.state = None;
        }
    }
    pub fn drain(&self) -> Pending {
        self.queued.store(false, Ordering::Release);
        std::mem::take(
            &mut *self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
    pub fn refresh_insets(&self) {
        self.control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .preferences = None;
        let token = self.token;
        self.call(move |env, activity| {
            let _ = env.call_method(
                activity,
                jni_str!("attachTaim"),
                jni_sig!("(J)V"),
                &[JValue::Long(token as i64)],
            )?;
            Ok(())
        });
    }
    pub fn owner(&self) -> u64 {
        self.owner.load(Ordering::Acquire)
    }

    pub fn show_editor(&self) {
        let token = self.token;
        let owner = self.owner();
        self.call(move |env, activity| {
            let _ = env.call_method(
                activity,
                jni_str!("showTaimEditor"),
                jni_sig!("(JJ)V"),
                &[JValue::Long(token as i64), JValue::Long(owner as i64)],
            )?;
            Ok(())
        });
    }

    pub fn appearance(&self, dark: bool) {
        let mut control = self
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if control.dark == Some(dark) {
            return;
        }
        control.dark = Some(dark);
        self.call(move |env, activity| {
            let _ = env.call_method(
                activity,
                jni_str!("setTaimAppearance"),
                jni_sig!("(Z)V"),
                &[JValue::Bool(dark)],
            )?;
            Ok(())
        });
    }

    pub fn sync_editor(
        &self,
        editor: Option<(widget::Id, EditorState)>,
        cursor: Rectangle,
        secure: bool,
    ) {
        // Disable oversized contexts instead of letting Android's input filter
        // truncate a full snapshot and accidentally replace the application text.
        let editor = editor.filter(|(_, state)| {
            state.content.len() <= MAX_TEXT_BYTES
                && state.content.encode_utf16().count() <= MAX_TEXT_UTF16
                && (state.multiline || !state.content.contains(['\r', '\n']))
        });
        let mut control = self
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let identity = editor.as_ref().map(|(id, _)| id.clone());
        let changed_focus = identity != control.focus || secure != control.secure;
        control.secure = secure;
        if changed_focus {
            control.focus = identity;
            control.state = None;
            control.composing = false;
            control.applied_revision = 0;
            let _ = self.owner.fetch_add(1, Ordering::AcqRel);
        }
        if control.composing && !changed_focus {
            return;
        }
        let state = editor.map(|(_, state)| state);
        if !changed_focus && state == control.state && control.cursor == Some(cursor) {
            return;
        }
        control.state = state.clone();
        control.cursor = Some(cursor);
        let enabled = state.is_some();
        let state = state.unwrap_or(EditorState {
            content: String::new(),
            selection: 0..0,
            composing: None,
            multiline: false,
        });
        if state.content.len() > MAX_TEXT_BYTES {
            log::warn!("Android editor context exceeds 1 MiB limit");
            return;
        }
        let start = utf8_to_utf16(&state.content, state.selection.start) as i32;
        let end = utf8_to_utf16(&state.content, state.selection.end) as i32;
        let token = self.token;
        let owner = self.owner();
        let revision = control.applied_revision;
        self.call(move |env, activity| {
            let text = JString::new(env, &state.content)?;
            let _ = env.call_method(
                activity,
                jni_str!("setTaimEditor"),
                jni_sig!("(JJJZLjava/lang/String;IIZZIIII)V"),
                &[
                    JValue::Long(token as i64),
                    JValue::Long(owner as i64),
                    JValue::Long(revision as i64),
                    JValue::Bool(enabled),
                    JValue::Object(text.as_ref()),
                    JValue::Int(start),
                    JValue::Int(end),
                    JValue::Bool(state.multiline),
                    JValue::Bool(secure),
                    JValue::Int(cursor.x as i32),
                    JValue::Int(cursor.y as i32),
                    JValue::Int(cursor.width.max(1.0) as i32),
                    JValue::Int(cursor.height.max(1.0) as i32),
                ],
            )?;
            Ok(())
        });
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        if let Some(registry) = REGISTRY.get() {
            let _ = registry
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&self.token);
        }
    }
}
fn string_units(env: &mut jni::Env<'_>, text: &JString<'_>) -> jni::errors::Result<usize> {
    Ok(env
        .call_method(text, jni_str!("length"), jni_sig!("()I"), &[])?
        .i()?
        .max(0) as usize)
}

fn lookup(token: i64) -> Option<Arc<Bridge>> {
    REGISTRY
        .get()?
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&(token as u64))?
        .upgrade()
}

// SAFETY: signatures exactly match the static Java declarations. EnvUnowned
// guards the JNI local frame and catches Rust unwinding at each boundary.
#[unsafe(no_mangle)]
extern "system" fn Java_com_theaimartco_taim_TaimActivity_nativeInsets(
    mut env: EnvUnowned<'_>,
    _class: JClass<'_>,
    token: i64,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    dark: bool,
    reduced: bool,
) {
    env.with_env(|_env| -> jni::errors::Result<()> {
        if let Some(bridge) = lookup(token) {
            let preferences = (
                Insets {
                    left: left.max(0) as u32,
                    top: top.max(0) as u32,
                    right: right.max(0) as u32,
                    bottom: bottom.max(0) as u32,
                },
                dark,
                reduced,
            );
            let mut control = bridge
                .control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if control.preferences == Some(preferences) {
                return Ok(());
            }
            control.preferences = Some(preferences);
            drop(control);
            bridge
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insets = Some(preferences);
            bridge.wake();
        }
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}
#[unsafe(no_mangle)]
extern "system" fn Java_com_theaimartco_taim_TaimActivity_nativeEdit(
    mut env: EnvUnowned<'_>,
    _class: JClass<'_>,
    token: i64,
    owner: i64,
    revision: i64,
    text: JString<'_>,
    start: i32,
    end: i32,
    composing_start: i32,
    composing_end: i32,
    multiline: bool,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        let Some(bridge) = lookup(token).filter(|bridge| bridge.owner() == owner as u64) else {
            return Ok(());
        };
        if env
            .call_method(&text, jni_str!("length"), jni_sig!("()I"), &[])?
            .i()? as usize
            > MAX_TEXT_UTF16
        {
            return Ok(());
        }
        let content = text.try_to_string(env)?;
        let selection = utf16_to_utf8(&content, start.max(0) as usize)
            ..utf16_to_utf8(&content, end.max(0) as usize);
        let composing = (composing_start >= 0 && composing_end > composing_start).then(|| {
            utf16_to_utf8(&content, composing_start as usize)
                ..utf16_to_utf8(&content, composing_end as usize)
        });
        let mut control = bridge
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if bridge.owner() != owner as u64 || control.secure {
            return Ok(());
        }
        control.composing = composing.is_some();
        drop(control);
        let next = (
            owner as u64,
            revision as u64,
            EditorState {
                content,
                selection,
                composing,
                multiline,
            }
            .normalized(),
        );
        let mut pending = bridge
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Coalesce only within the same composition phase. A committed snapshot
        // followed by a new preedit must reach the view in that order.
        if pending
            .edits
            .last()
            .is_some_and(|(previous_owner, _, state)| {
                *previous_owner == next.0 && state.composing.is_some() == next.2.composing.is_some()
            })
        {
            let _ = pending.edits.pop();
        }
        pending.edits.push(next);
        drop(pending);
        bridge.wake();
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}
#[unsafe(no_mangle)]
extern "system" fn Java_com_theaimartco_taim_TaimActivity_nativeSubmit(
    mut env: EnvUnowned<'_>,
    _class: JClass<'_>,
    token: i64,
    owner: i64,
) {
    env.with_env(|_env| -> jni::errors::Result<()> {
        if let Some(bridge) = lookup(token).filter(|bridge| bridge.owner() == owner as u64) {
            bridge
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .submit = Some(owner as u64);
            bridge.wake();
        }
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_theaimartco_taim_TaimActivity_nativeSecure(
    mut env: EnvUnowned<'_>,
    _class: JClass<'_>,
    token: i64,
    owner: i64,
    kind: i32,
    text: JString<'_>,
    count: i32,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        let Some(bridge) = lookup(token).filter(|bridge| bridge.owner() == owner as u64) else {
            return Ok(());
        };
        if !bridge
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .secure
        {
            return Ok(());
        }
        use taim_core::{Event, input_method, keyboard};
        let event = match kind {
            0 | 1 => {
                if env
                    .call_method(&text, jni_str!("length"), jni_sig!("()I"), &[])?
                    .i()?
                    > 262144
                {
                    return Ok(());
                }
                let content = text.try_to_string(env)?;
                Event::InputMethod(if kind == 0 {
                    input_method::Event::Commit(content)
                } else {
                    let end = content.len();
                    input_method::Event::Preedit(content, Some(end..end))
                })
            }
            2 | 3 => {
                let key = keyboard::Key::Named(if kind == 2 {
                    keyboard::key::Named::Backspace
                } else {
                    keyboard::key::Named::Delete
                });
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: key.clone(),
                    modified_key: key,
                    physical_key: keyboard::key::Physical::Unidentified(
                        keyboard::key::NativeCode::Unidentified,
                    ),
                    location: keyboard::Location::Standard,
                    modifiers: keyboard::Modifiers::empty(),
                    text: None,
                    repeat: false,
                })
            }
            8 => {
                let key = keyboard::Key::Character("a".into());
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: key.clone(),
                    modified_key: key,
                    physical_key: keyboard::key::Physical::Unidentified(
                        keyboard::key::NativeCode::Unidentified,
                    ),
                    location: keyboard::Location::Standard,
                    modifiers: keyboard::Modifiers::CTRL,
                    text: None,
                    repeat: false,
                })
            }
            4..=7 => Event::InputMethod(input_method::Event::DeleteSurrounding {
                before: if kind % 2 == 0 {
                    count.clamp(0, 256) as usize
                } else {
                    0
                },
                after: if kind % 2 == 1 {
                    count.clamp(0, 256) as usize
                } else {
                    0
                },
                codepoints: kind >= 6,
            }),
            _ => return Ok(()),
        };
        let repeats = if matches!(kind, 2 | 3) {
            count.clamp(0, 256)
        } else {
            1
        };
        let mut pending = bridge
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for _ in 0..repeats {
            pending.operations.push((owner as u64, event.clone()));
        }
        drop(pending);
        bridge.wake();
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_theaimartco_taim_TaimActivity_nativeSemanticAction(
    mut env: EnvUnowned<'_>,
    _class: JClass<'_>,
    token: i64,
    payload: JString<'_>,
) -> bool {
    env.with_env(|env| -> jni::errors::Result<bool> {
        let Some(bridge) = lookup(token) else {
            return Ok(false);
        };
        if string_units(env, &payload)? > 262144 {
            return Ok(false);
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&payload.try_to_string(env)?)
        else {
            return Ok(false);
        };
        let snapshot = bridge
            .semantics
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut pending = bridge
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if pending.semantics.len() >= 128 {
            return Ok(false);
        }
        let Some(command) = snapshot.command(&value) else {
            return Ok(false);
        };
        pending.semantics.push(command);
        drop(pending);
        drop(snapshot);
        bridge.wake();
        Ok(true)
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

pub(super) fn current() -> Option<Arc<Bridge>> {
    REGISTRY
        .get()?
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .filter_map(|(token, bridge)| bridge.upgrade().map(|bridge| (*token, bridge)))
        .max_by_key(|(token, _)| *token)
        .map(|(_, bridge)| bridge)
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_theaimartco_taim_TaimActivity_nativeServiceResponse(
    mut env: EnvUnowned<'_>,
    _class: JClass<'_>,
    token: i64,
    id: i64,
    payload: JString<'_>,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        let Some(bridge) = lookup(token) else {
            return Ok(());
        };
        if string_units(env, &payload)? > 6 * 1024 * 1024 {
            bridge
                .services
                .complete(id as u64, Err(super::ServiceError::Invalid));
            return Ok(());
        }
        let result = serde_json::from_str::<serde_json::Value>(&payload.try_to_string(env)?).ok();
        let result = match result {
            Some(value) if value["ok"] == true => Ok(value["value"].clone()),
            Some(value) => Err(match value["error"].as_str() {
                Some("locationDisabled") => super::ServiceError::LocationDisabled,
                Some("cancelledForeground") => super::ServiceError::Cancelled,
                Some("unsupported") => super::ServiceError::Unsupported,
                Some("unavailable") => super::ServiceError::Unavailable,
                Some("busy") => super::ServiceError::Busy,
                Some("denied" | "cancelled") => super::ServiceError::Denied,
                Some("invalid" | "tooLarge") => super::ServiceError::Invalid,
                Some("timeout") => super::ServiceError::Timeout,
                Some("lockout") => super::ServiceError::Lockout,
                Some("lockoutPermanent") => super::ServiceError::LockoutPermanent,
                Some("notEnrolled") => super::ServiceError::NotEnrolled,
                Some("disabled") => super::ServiceError::Disabled,
                Some("notFound") => super::ServiceError::NotFound,
                Some("keyInvalidated") => super::ServiceError::KeyInvalidated,
                _ => super::ServiceError::Failed,
            }),
            None => Err(super::ServiceError::Failed),
        };
        bridge.services.complete(id as u64, result);
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_theaimartco_taim_TaimActivity_nativePlatformEvent(
    mut env: EnvUnowned<'_>,
    _class: JClass<'_>,
    token: i64,
    payload: JString<'_>,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        let Some(bridge) = lookup(token) else {
            return Ok(());
        };
        if string_units(env, &payload)? > 65536 {
            return Ok(());
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&payload.try_to_string(env)?)
        else {
            return Ok(());
        };
        use super::PlatformEvent;
        let event = match value["type"].as_str() {
            Some("permissionsChanged") => {
                let Some(statuses) = super::permissions::decode(value["permissions"].clone())
                else {
                    return Ok(());
                };
                PlatformEvent::PermissionsChanged(statuses)
            }
            Some("nativeView") => PlatformEvent::NativeView {
                id: value["id"].as_u64().unwrap_or(0),
                generation: value["generation"].as_u64().unwrap_or(0),
                event: value["event"].clone(),
            },
            Some("nativeViewFocus") => {
                let Some(id) = value["id"].as_u64().filter(|id| *id > 0) else {
                    return Ok(());
                };
                let rect = value["rect"]
                    .as_array()
                    .filter(|r| r.len() == 4)
                    .and_then(|r| {
                        Some(Rectangle {
                            x: r[0].as_f64()? as f32,
                            y: r[1].as_f64()? as f32,
                            width: r[2].as_f64()? as f32,
                            height: r[3].as_f64()? as f32,
                        })
                    });
                let rect = rect.filter(|r| {
                    [r.x, r.y, r.width, r.height].iter().all(|n| n.is_finite())
                        && r.width >= 0.0
                        && r.height >= 0.0
                });
                let mut pending = bridge
                    .pending
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                pending.native_blur |= rect.is_some();
                pending.native_focus = Some((id, value["generation"].as_u64().unwrap_or(0), rect));
                drop(pending);
                bridge.wake();
                return Ok(());
            }
            Some("location") => {
                let Some(watch) = value["watch"].as_u64().filter(|id| *id > 0) else {
                    return Ok(());
                };
                let Some(fix) = super::LocationFix::from_value(value["fix"].clone()) else {
                    return Ok(());
                };
                PlatformEvent::Location { watch, fix }
            }
            Some("locationEnded") => {
                let Some(watch) = value["watch"].as_u64().filter(|id| *id > 0) else {
                    return Ok(());
                };
                PlatformEvent::LocationEnded {
                    watch,
                    reason: super::LocationEnd::parse(value["reason"].as_str().unwrap_or("failed")),
                }
            }
            Some("intent") => PlatformEvent::Intent {
                action: value["action"].as_str().unwrap_or("").to_owned(),
                uri: value["uri"].as_str().map(str::to_owned),
                mime_type: value["mime"].as_str().map(str::to_owned),
                notification: value["notification"]["id"]
                    .as_i64()
                    .and_then(|id| i32::try_from(id).ok())
                    .filter(|id| *id >= 0)
                    .map(|id| super::NotificationTap {
                        id,
                        payload: value["notification"]["payload"]
                            .as_str()
                            .filter(|s| s.len() <= 4096)
                            .map(str::to_owned),
                    }),
            },
            Some("lifecycle") => {
                PlatformEvent::Lifecycle(value["state"].as_str().unwrap_or("").to_owned())
            }
            Some("back") => {
                let phase = value["phase"].as_str().unwrap_or("").to_owned();
                if phase == "invoked" {
                    {
                        let mut pending = bridge
                            .pending
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        pending.back = pending.back.saturating_add(1);
                    }
                    bridge.wake();
                }
                PlatformEvent::Back {
                    phase,
                    progress: value["progress"]
                        .as_f64()
                        .filter(|n| n.is_finite())
                        .unwrap_or(0.0)
                        .clamp(0.0, 1.0) as f32,
                    edge: value["edge"].as_i64().unwrap_or(0) as i32,
                }
            }
            Some("plugin") => PlatformEvent::Plugin {
                channel: value["channel"].as_str().unwrap_or("").to_owned(),
                event: value["event"].clone(),
            },
            Some("memory") => {
                let level = value["level"].as_i64().unwrap_or(80) as i32;
                bridge
                    .pending
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .pressure = Some(level);
                bridge.wake();
                PlatformEvent::MemoryPressure(level)
            }
            _ => return Ok(()),
        };
        bridge.services.event(event);
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

/// Fixed-label native frame trace; no application text enters system tracing.
#[cfg(feature = "measurement")]
pub(crate) struct FrameTrace;
#[cfg(feature = "measurement")]
#[link(name = "android")]
unsafe extern "C" {
    fn ATrace_beginSection(name: *const std::ffi::c_char);
    fn ATrace_endSection();
}
#[cfg(feature = "measurement")]
impl FrameTrace {
    pub fn begin() -> Self {
        unsafe {
            ATrace_beginSection(c"TaimFrame".as_ptr());
        }
        Self
    }
}
#[cfg(feature = "measurement")]
impl Drop for FrameTrace {
    fn drop(&mut self) {
        unsafe {
            ATrace_endSection();
        }
    }
}
