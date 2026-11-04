//! One open window: the platform window, its drawing surface, and the view
//! tree it shows.
use crate::conversion;
use crate::core::time::Instant;
use crate::core::{Point, Rectangle, Size, keyboard, mouse, theme, window};
use crate::frame_clock::Scheduler;
use crate::graphics::{Compositor as _, Viewport, compositor};
use crate::wake::ChangeSignal;
use crate::{Compositor, Renderer};

use taim_core::Renderer as _;
use taim_core::theme::Base as _;
use taim_view::{Response, Root, Theme};

use std::sync::Arc;
use winit::dpi::PhysicalPosition;
use winit::event::WindowEvent;

/// Shell-local response keeps async paste authorization attached to the originating event.
#[derive(Default)]
pub(crate) struct Dispatch {
    pub response: Response,
    pub paste: Option<taim_view::PasteToken>,
}
impl std::ops::Deref for Dispatch {
    type Target = Response;
    fn deref(&self) -> &Response {
        &self.response
    }
}

type Accessibility = taim_accessibility::Session<taim_accessibility::atspi::adapter::Adapter>;

#[cfg(feature = "accessibility")]
type AccessibilityBatch = (
    Vec<Dispatch>,
    Vec<(taim_accessibility::atspi::store::ActionRequest, bool)>,
);

/// A window and everything the shell keeps for it.
pub(crate) struct Surface {
    pub id: window::Id,
    pub raw: Arc<winit::window::Window>,
    pub exit_on_close: bool,
    pub changes: ChangeSignal,
    pub root: Root,

    target: Option<<Compositor as compositor::Compositor>::Surface>,
    renderer: Renderer,
    #[cfg(target_os = "android")]
    renderer_settings: taim_core::renderer::Settings,
    #[cfg(target_os = "android")]
    present_failures: u8,
    viewport: Viewport,
    content: Rectangle,
    #[cfg(target_os = "android")]
    android: Arc<crate::android::Bridge>,
    #[cfg(target_os = "android")]
    insets: crate::android::geometry::Insets,
    #[cfg(target_os = "android")]
    pan_y: f32,
    #[cfg(target_os = "android")]
    native_focus: Option<(u64, u64, Rectangle)>,
    #[cfg(target_os = "android")]
    native_touches: std::collections::HashSet<u64>,
    /// The surface must be reconfigured to the viewport before drawing.
    resized: bool,
    /// Emit one opt-in diagnostic after a target successfully presents.
    #[cfg(all(target_os = "android", feature = "measurement"))]
    awaiting_present: bool,
    /// Sizes may have changed since the last layout.
    stale_layout: bool,
    clock: Scheduler,

    look: Look,
    pointer: Option<PhysicalPosition<f64>>,
    modifiers: keyboard::Modifiers,
    cursor: mouse::Interaction,
    hidden: Hidden,
    focused: bool,
    #[cfg(not(target_os = "android"))]
    ime: Option<(Rectangle, taim_core::input_method::Purpose, f64)>,

    accessibility: Accessibility,
    published: taim_accessibility::Tree,
    /// Something that can change the semantic tree happened since it was
    /// last published. A pointer merely moving over the window does not.
    semantics_stale: bool,
    #[cfg(feature = "accessibility")]
    accessibility_actions:
        std::sync::mpsc::Receiver<taim_accessibility::atspi::store::ActionRequest>,
    #[cfg(feature = "accessibility")]
    _bridge: taim_accessibility::atspi::bridge::Bridge,
}

/// The resolved theme and the colors derived from it.
struct Look {
    theme: Theme,
    style: theme::Style,
    /// The light or dark mode last asked of the platform window.
    mode: theme::Mode,
}

/// Reasons nothing can be seen, so no frames are drawn.
#[derive(Default)]
struct Hidden {
    occluded: bool,
    empty: bool,
}

impl Surface {
    pub fn new(
        id: window::Id,
        raw: Arc<winit::window::Window>,
        root: Root,
        compositor: &mut Compositor,
        renderer_settings: taim_core::renderer::Settings,
        exit_on_close: bool,
        changes: ChangeSignal,
        system: theme::Mode,
        #[cfg(target_os = "android")] android: Arc<crate::android::Bridge>,
    ) -> Self {
        let physical = raw.inner_size();
        let viewport = Viewport::with_physical_size(
            Size::new(physical.width, physical.height),
            zoomed(raw.scale_factor(), root.scale()),
        );

        let accessibility = Accessibility::new(taim_accessibility::atspi::adapter::Adapter::new());

        #[cfg(feature = "accessibility")]
        let accessibility_actions = {
            let (sender, receiver) = std::sync::mpsc::sync_channel(256);
            let wake = changes.clone();
            accessibility
                .adapter()
                .store()
                .set_action_handler(move |request| {
                    if sender.try_send(request).is_ok() {
                        wake.raise();
                        true
                    } else {
                        false
                    }
                });
            receiver
        };

        #[cfg(feature = "accessibility")]
        let _bridge = taim_accessibility::atspi::bridge::Bridge::spawn(
            accessibility.adapter().store().clone(),
            accessibility.adapter().events().clone(),
            raw.title(),
        );

        let wake = changes.clone();
        root.on_change(move || wake.raise());

        let mut surface = Self {
            id,
            target: Some(compositor.create_surface(raw.clone(), physical.width, physical.height)),
            renderer: compositor.create_renderer(renderer_settings),
            #[cfg(target_os = "android")]
            renderer_settings,
            #[cfg(target_os = "android")]
            present_failures: 0,
            raw,
            exit_on_close,
            changes,
            root,
            content: Rectangle::with_size(viewport.logical_size()),
            viewport,
            #[cfg(target_os = "android")]
            android,
            #[cfg(target_os = "android")]
            insets: crate::android::geometry::Insets::default(),
            #[cfg(target_os = "android")]
            pan_y: 0.0,
            #[cfg(target_os = "android")]
            native_focus: None,
            #[cfg(target_os = "android")]
            native_touches: std::collections::HashSet::new(),
            resized: false,
            #[cfg(all(target_os = "android", feature = "measurement"))]
            awaiting_present: true,
            stale_layout: true,
            clock: Scheduler::new(),
            look: Look {
                theme: Theme::default(system),
                style: Theme::default(system).base(),
                mode: theme::Mode::None,
            },
            pointer: None,
            modifiers: keyboard::Modifiers::empty(),
            cursor: mouse::Interaction::None,
            hidden: Hidden::default(),
            focused: false,
            #[cfg(not(target_os = "android"))]
            ime: None,
            accessibility,
            published: taim_accessibility::Tree::new(),
            semantics_stale: true,

            #[cfg(feature = "accessibility")]
            accessibility_actions,
            #[cfg(feature = "accessibility")]
            _bridge,
        };

        surface.hidden.empty = physical.width == 0 || physical.height == 0;
        surface.refresh_visibility();
        let _ = surface.root.settings_changed();
        surface.restyle(system);

        #[cfg(feature = "hinting")]
        surface.renderer.hint(surface.viewport.scale_factor());

        surface
    }

    /// Releases the native presentation target before Android destroys it.
    #[cfg(target_os = "android")]
    pub fn suspend(&mut self) {
        self.native_touches.clear();
        self.native_focus = None;
        self.clock.suspend();
        // Taking and dropping the target here must finish before the callback
        // returns. A repeated suspend has no native resources left to release.
        if self.target.take().is_none() {
            return;
        }
        log::info!("Android presentation surface released");
        self.pointer = None;
        self.modifiers = keyboard::Modifiers::empty();
        self.focused = false;
        // Cancel presses, drags and composition before returning to Android.
        let _ = self.handle(&taim_core::Event::Window(window::Event::Unfocused));
    }

    /// Recreates presentation resources without rebuilding application views.
    #[cfg(target_os = "android")]
    pub fn resume(&mut self, compositor: &mut Compositor) {
        if self.target.is_some() {
            return;
        }
        let physical = self.raw.inner_size();
        self.target =
            Some(compositor.create_surface(self.raw.clone(), physical.width, physical.height));
        #[cfg(feature = "measurement")]
        {
            self.awaiting_present = true;
        }
        log::info!("Android presentation surface recreated");
        self.hidden.empty = physical.width == 0 || physical.height == 0;
        self.hidden.occluded = false;
        self.fit(Size::new(physical.width, physical.height));
        self.stale_layout = true;
        self.refresh_visibility();
        self.request_frame();
    }

    /// Asks for a frame as soon as possible.
    pub fn request_frame(&mut self) {
        self.clock.request_redraw_at(Instant::now());
        self.raw.request_redraw();
    }

    /// When this window next needs the event loop, if it is waiting.
    pub fn deadline(&self) -> Option<Instant> {
        self.clock.deadline()
    }

    /// Lets the frame clock catch up with `now`, asking the platform for a
    /// frame if one is due.
    pub fn tick(&mut self, now: Instant) {
        self.clock.tick(now);

        if self.clock.should_produce_frame(now) {
            self.raw.request_redraw();
        }
    }

    /// Folds a platform event into the window's own state. Returns the
    /// TAIM event the views should see, if any.
    pub fn observe(
        &mut self,
        mut event: WindowEvent,
        system: theme::Mode,
    ) -> Option<taim_core::Event> {
        match &event {
            WindowEvent::Resized(size) => {
                self.hidden.empty = size.width == 0 || size.height == 0;
                self.refresh_visibility();
                self.fit(Size::new(size.width, size.height));
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = self.viewport.physical_size();
                self.fit(size);
            }
            WindowEvent::Occluded(occluded) => {
                self.hidden.occluded = *occluded;
                self.refresh_visibility();
            }
            WindowEvent::Focused(focused) => {
                self.focused = *focused;
                self.sync_input_method();
                self.request_frame();
            }
            WindowEvent::ThemeChanged(_) => self.restyle(system),
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = Some(*position);
                #[cfg(target_os = "android")]
                self.android
                    .hover(position.x as f32, position.y as f32, false);
            }
            WindowEvent::Touch(touch) => self.pointer = Some(touch.location),
            WindowEvent::CursorLeft { .. } => {
                self.pointer = None;
                #[cfg(target_os = "android")]
                self.android.hover(0.0, 0.0, true);
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = conversion::modifiers(modifiers.state());
            }
            _ => {}
        }

        #[cfg(target_os = "android")]
        if let WindowEvent::KeyboardInput { event, .. } = &event
            && let Some((id, generation, _)) = self.native_focus
        {
            self.android.native_key(
                id,
                generation,
                crate::android::keys::android_key(event.physical_key).unwrap_or(0),
                event.state == winit::event::ElementState::Pressed,
                event.repeat,
                event.text.as_deref().unwrap_or(""),
            );
            return None;
        }
        #[cfg(target_os = "android")]
        if let WindowEvent::Touch(touch) = &event {
            use winit::event::TouchPhase;
            if touch.phase == TouchPhase::Started {
                let scale = self.viewport.scale_factor();
                let origin = self.paint_origin();
                let point = Point::new(
                    touch.location.x as f32 / scale - origin.x,
                    touch.location.y as f32 / scale - origin.y,
                );
                if !self.native_touches.is_empty()
                    || (self.content.contains(Point::new(
                        touch.location.x as f32 / scale,
                        touch.location.y as f32 / scale,
                    )) && self
                        .root
                        .native_views()
                        .iter()
                        .any(|slot| slot.visible.is_some_and(|clip| clip.contains(point))))
                {
                    let _ = self.native_touches.insert(touch.id);
                } else {
                    self.android.native_touch(
                        touch.id,
                        4,
                        touch.location.x as f32,
                        touch.location.y as f32,
                    );
                }
            }
            if self.native_touches.contains(&touch.id) {
                let phase = match touch.phase {
                    TouchPhase::Started => 0,
                    TouchPhase::Moved => 1,
                    TouchPhase::Ended => 2,
                    TouchPhase::Cancelled => 3,
                };
                self.android.native_touch(
                    touch.id,
                    phase,
                    touch.location.x as f32,
                    touch.location.y as f32,
                );
                if matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                    let _ = self.native_touches.remove(&touch.id);
                }
                return None;
            }
        }
        let origin = self.paint_origin();
        let scale = f64::from(self.viewport.scale_factor());
        match &mut event {
            WindowEvent::Touch(touch) => {
                touch.location.x -= f64::from(origin.x) * scale;
                touch.location.y -= f64::from(origin.y) * scale;
            }
            WindowEvent::CursorMoved { position, .. } => {
                position.x -= f64::from(origin.x) * scale;
                position.y -= f64::from(origin.y) * scale;
            }
            _ => {}
        }
        conversion::window_event(event, self.viewport.scale_factor(), self.modifiers)
    }

    /// The pointer in logical coordinates.
    pub fn pointer(&self) -> Option<Point> {
        self.pointer.map(|at| {
            let at = conversion::cursor_position(at, self.viewport.scale_factor());
            let origin = self.paint_origin();
            Point::new(at.x - origin.x, at.y - origin.y)
        })
    }

    fn dispatch(&mut self, response: Response) -> Dispatch {
        let paste = if response.paste && self.focused {
            self.root.begin_paste()
        } else {
            None
        };
        Dispatch { response, paste }
    }
    pub fn accept_paste(&mut self, token: &taim_view::PasteToken) -> bool {
        self.focused && self.root.accept_paste(token)
    }

    /// Routes `event` to the views.
    pub fn handle(&mut self, event: &taim_core::Event) -> Dispatch {
        // Pending rebuilds must be applied before hit testing or routing keys.
        if self.stale_layout {
            self.root.layout(&self.renderer, self.content.size());
            self.stale_layout = false;
        }
        let response =
            self.root
                .handle_with_renderer(&self.renderer, event, self.pointer(), Instant::now());

        if response.paint
            || response.captured
            || !matches!(
                event,
                taim_core::Event::Mouse(mouse::Event::CursorMoved { .. })
            )
        {
            self.semantics_stale = true;
        }

        self.apply_response(&response);
        // iOS: show the on-screen keyboard while a text view has focus. Android
        // drives its own editor proxy through the host bridge instead.
        #[cfg(target_os = "ios")]
        self.raw.set_ime_allowed(self.root.wants_text_input());
        #[cfg(target_os = "android")]
        if response.captured
            && matches!(
                event,
                taim_core::Event::Touch(taim_core::touch::Event::FingerPressed { .. })
                    | taim_core::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            )
        {
            self.android.show_editor();
        }
        self.dispatch(response)
    }

    fn apply_response(&mut self, response: &Response) {
        if response.layout {
            self.stale_layout = true;
        }
        if response.animate {
            self.clock.request_animation();
        }
        if response.paint {
            self.request_frame();
        }
        self.update_cursor();
        self.sync_input_method();
    }

    /// Picks up state changes the views were told about.
    pub fn absorb_changes(&mut self, system: theme::Mode) -> Vec<Dispatch> {
        self.changes.lower();

        if self.root.settings_changed() {
            self.restyle(system);
            let size = self.viewport.physical_size();
            self.fit(size);
            self.stale_layout = true;
        }

        let (paint, layout) = self.root.take_changes();
        #[cfg(target_os = "android")]
        self.checkpoint();
        if layout {
            self.stale_layout = true;
        }
        if paint || layout {
            self.semantics_stale = true;
            self.request_frame();
        }
        #[cfg(feature = "accessibility")]
        {
            let (responses, completed) = self.perform_accessibility_actions();
            let (paint, layout) = self.root.take_changes();
            self.stale_layout |= layout;
            if paint || layout {
                self.semantics_stale = true;
                self.request_frame();
            }
            if self.semantics_stale {
                if self.stale_layout {
                    self.root.layout(&self.renderer, self.content.size());
                    self.stale_layout = false;
                }
                self.publish_semantics();
            }
            for (request, executed) in completed {
                request.complete(executed);
            }
            responses
        }
        #[cfg(not(feature = "accessibility"))]
        Vec::new()
    }

    #[cfg(feature = "accessibility")]
    fn perform_accessibility_actions(&mut self) -> AccessibilityBatch {
        let mut responses = Vec::new();
        let mut completed = Vec::new();
        // Limit each event-loop turn; a busy assistive client must not starve input.
        for _ in 0..32 {
            let Ok(request) = self.accessibility_actions.try_recv() else {
                return (responses, completed);
            };
            if request.is_cancelled() {
                continue;
            }
            let (paint, layout) = self.root.take_changes();
            self.stale_layout |= layout;
            if paint || layout {
                self.semantics_stale = true;
                self.request_frame();
            }
            if self.stale_layout {
                self.root.layout(&self.renderer, self.content.size());
                self.stale_layout = false;
            }
            let response = self.root.accessibility_action_with_renderer(
                &self.renderer,
                &request.action,
                Instant::now(),
            );
            self.semantics_stale |= response.captured || response.paint;
            self.apply_response(&response);
            completed.push((request, response.captured));
            if response.copy.is_some() || response.paste {
                responses.push(self.dispatch(response));
            }
        }
        // Re-arm even if producers filled the queue before this wake was lowered.
        self.changes.raise();
        (responses, completed)
    }

    /// Re-resolves the theme after the application or the desktop changed
    /// it.
    #[cfg(target_os = "android")]
    pub fn set_system_motion_reduced(&mut self, reduced: bool) {
        if self.root.set_system_motion_reduced(reduced) {
            self.request_frame();
        }
    }

    pub fn restyle(&mut self, system: theme::Mode) {
        let theme = self.root.theme().unwrap_or_else(|| Theme::default(system));
        let wanted = match theme.mode() {
            theme::Mode::None => Theme::default(system).mode(),
            mode => mode,
        };

        if wanted != self.look.mode {
            self.raw.set_theme(conversion::window_theme(wanted));
            self.look.mode = wanted;
        }

        #[cfg(target_os = "android")]
        self.android.appearance(wanted == theme::Mode::Dark);
        self.renderer.set_light(theme.light());
        self.look.style = theme.base();
        log::info!(
            "Taim theme {}: background={:?}, primary={:?}",
            theme.name(),
            self.look.style.background_color,
            theme.palette().primary.base.color
        );
        self.root.set_theme(theme.clone());
        self.look.theme = theme;
        self.stale_layout = true;
        self.request_frame();
    }

    /// Draws and presents one frame.
    pub fn draw(&mut self, compositor: &mut Compositor) -> Result<(), crate::Error> {
        #[cfg(all(target_os = "android", feature = "measurement"))]
        let _trace = crate::android::FrameTrace::begin();
        #[cfg(target_os = "android")]
        if compositor.is_device_lost() {
            return Err(crate::Error::PresentationFailed(
                compositor::SurfaceError::Lost,
            ));
        }
        let refresh = self
            .raw
            .current_monitor()
            .and_then(|monitor| monitor.refresh_rate_millihertz());
        self.clock
            .set_animation_interval(animation_interval(refresh));
        let now = Instant::now();
        let started = std::time::Instant::now();

        if self.clock.is_suspended() || self.target.is_none() {
            return Ok(());
        }
        // A frame the platform asked for (an expose) is drawn even if the
        // clock had nothing scheduled.
        let _ = self.clock.begin_frame(now);

        if self.resized {
            let size = self.viewport.physical_size();
            compositor.configure_surface(
                self.target.as_mut().expect("active surface"),
                size.width,
                size.height,
            );
            #[cfg(feature = "hinting")]
            self.renderer.hint(self.viewport.scale_factor());
            self.resized = false;
            self.stale_layout = true;
        }

        let (_, layout) = self.root.take_changes();
        if layout || self.stale_layout {
            self.root.layout(&self.renderer, self.content.size());
            self.stale_layout = false;
        }

        let bounds = Rectangle::with_size(self.viewport.logical_size());
        self.renderer.reset(bounds);
        #[cfg(target_os = "android")]
        {
            self.update_pan();
            let slots = self.root.native_views();
            self.android
                .publish_native_views(crate::android::native_views::scene(
                    slots,
                    self.paint_origin(),
                    self.viewport.scale_factor(),
                    self.content,
                ));
        }
        let origin = self.paint_origin();
        let pointer = self.pointer();
        self.renderer.with_layer(self.content, |renderer| {
            renderer.with_transformation(
                taim_core::Transformation::translate(origin.x, origin.y),
                |renderer| {
                    self.root
                        .paint(renderer, self.look.style.text_color, pointer, now);
                },
            );
        });
        self.sync_input_method();
        #[cfg(target_os = "android")]
        self.checkpoint();
        let painted = started.elapsed();
        self.publish_semantics();
        self.update_cursor();
        let described = started.elapsed();

        self.clock.submit(now);
        let presented = compositor.present(
            &mut self.renderer,
            self.target.as_mut().expect("active surface"),
            &self.viewport,
            self.look.style.background_color,
            || self.raw.pre_present_notify(),
        );
        self.clock.feedback_received();

        if trace_frames() {
            let total = started.elapsed();
            eprintln!(
                "frame {:?}: tree {:>5} us | semantics {:>5} us | present {:>6} us | total {:>6} us",
                self.id,
                painted.as_micros(),
                (described - painted).as_micros(),
                (total - described).as_micros(),
                total.as_micros(),
            );
        }

        #[cfg(target_os = "android")]
        {
            if let Err(error @ (compositor::SurfaceError::Lost | compositor::SurfaceError::Other)) =
                &presented
            {
                self.present_failures = self.present_failures.saturating_add(1);
                if self.present_failures >= 3 {
                    return Err(crate::Error::PresentationFailed(error.clone()));
                }
            } else if presented.is_ok() {
                self.present_failures = 0;
            }
        }
        match presented {
            Ok(()) => {
                #[cfg(all(target_os = "android", feature = "measurement"))]
                if std::mem::take(&mut self.awaiting_present) {
                    let size = self.viewport.physical_size();
                    log::info!(
                        "TAIM_ANDROID_PRESENT width={} height={}",
                        size.width,
                        size.height
                    );
                }
                #[cfg(all(feature = "measurement", not(target_arch = "wasm32")))]
                crate::measurement::first_present();
            }
            Err(compositor::SurfaceError::OutOfMemory) => {
                return Err(crate::Error::PresentationFailed(
                    compositor::SurfaceError::OutOfMemory,
                ));
            }
            Err(compositor::SurfaceError::Lost) => {
                let size = self.viewport.physical_size();
                self.target =
                    Some(compositor.create_surface(self.raw.clone(), size.width, size.height));
                #[cfg(all(target_os = "android", feature = "measurement"))]
                {
                    self.awaiting_present = true;
                }
                self.request_frame();
            }
            Err(compositor::SurfaceError::Outdated) => {
                self.resized = true;
                self.request_frame();
            }
            Err(compositor::SurfaceError::Occluded) => {
                self.hidden.occluded = true;
                self.refresh_visibility();
            }
            Err(error) => {
                log::warn!("presenting {:?} failed with {error:?}; retrying", self.id);
                self.request_frame();
            }
        }

        // Keep frames coming while anything still moves.
        if self.root.is_animating(now) {
            self.clock.request_animation();
        } else {
            self.clock.stop_animating();
        }

        if let Some(deadline) = self.root.redraw_at() {
            self.clock.request_redraw_at(deadline);
        }

        if self.clock.should_produce_frame(Instant::now()) {
            self.raw.request_redraw();
        }
        Ok(())
    }

    /// Diffs the views' semantic tree against what assistive technology
    /// last saw and sends the difference.
    fn publish_semantics(&mut self) {
        if !(cfg!(feature = "accessibility") || cfg!(target_os = "android"))
            || !self.semantics_stale
        {
            return;
        }
        self.semantics_stale = false;

        let window = self.root.describe(self.content.size());
        let next = taim_accessibility::Tree::from_forest(&[window]);
        let changes = self.published.diff(&next);
        self.published = next;

        self.accessibility.publish(&changes);
        #[cfg(target_os = "android")]
        {
            let geometry = (
                self.paint_origin(),
                self.viewport.scale_factor(),
                self.content,
            );
            {
                let editor = self.root.editor_state().and_then(|(_, state)| {
                    self.root.focused_semantic_identity().map(|id| (id, state))
                });
                self.android.publish_semantics(
                    &self.published,
                    geometry.0,
                    geometry.1,
                    geometry.2,
                    editor.as_ref(),
                );

                self.android.autofill_focus(if self.focused {
                    self.root.focused_semantic_identity()
                } else {
                    None
                });
            }
        }
        let focused = self
            .published
            .find(|entry| entry.state.contains(taim_core::semantics::State::FOCUSED))
            .map(|entry| &entry.id);
        self.accessibility.focus_changed(focused);
    }

    #[cfg(target_os = "android")]
    pub fn checkpoint(&mut self) {
        if let Ok(snapshot) = self.root.restoration_snapshot() {
            self.android.checkpoint(snapshot);
        }
        self.android.back_enabled(self.root.handles_back());
    }
    #[cfg(target_os = "android")]
    pub fn native_view_focus(&mut self, blur: bool, focus: Option<(u64, u64, Option<Rectangle>)>) {
        if blur {
            let response = self.root.release_focus(Instant::now());
            self.apply_response(&response);
        }
        if let Some((id, generation, rect)) = focus {
            if let Some(rect) = rect {
                self.native_focus = Some((id, generation, rect));
            } else if self
                .native_focus
                .as_ref()
                .is_some_and(|(active, instance, _)| *active == id && *instance == generation)
            {
                self.native_focus = None;
            }
        }
        self.semantics_stale = true;
        self.request_frame();
        self.sync_input_method();
    }

    #[cfg(target_os = "android")]
    pub fn native_action(&mut self, action: &taim_core::semantics::Action) -> Dispatch {
        let response =
            self.root
                .accessibility_action_with_renderer(&self.renderer, action, Instant::now());
        self.semantics_stale = true;
        self.apply_response(&response);
        self.dispatch(response)
    }

    #[cfg(target_os = "android")]
    fn text_target_allowed(&mut self, target: &taim_core::widget::Id, autofill: bool) -> bool {
        let tree =
            taim_accessibility::Tree::from_forest(&[self.root.describe(self.content.size())]);
        tree.get(target).is_some_and(|entry| {
            crate::android::semantics::text_target_allowed(
                entry.role,
                entry.state,
                entry.autofill.is_some(),
                autofill,
            )
        })
    }
    #[cfg(target_os = "android")]
    pub fn semantic_text_command(
        &mut self,
        target: taim_core::widget::Id,
        command: &str,
    ) -> Dispatch {
        if command == "blur" {
            if self.root.focused_semantic_identity().as_ref() != Some(&target) {
                return Dispatch::default();
            }
            let response = self.root.release_focus(Instant::now());
            self.semantics_stale = true;
            self.apply_response(&response);
            return self.dispatch(response);
        }
        if !self.text_target_allowed(&target, false) {
            return Dispatch::default();
        }
        let response = self.native_action(&taim_core::semantics::Action {
            target: target.clone(),
            request: taim_core::semantics::Request::Focus,
        });
        if !response.captured || self.root.focused_semantic_identity().as_ref() != Some(&target) {
            return response;
        }
        if !self.text_target_allowed(&target, false) {
            return Dispatch::default();
        }
        let letter = match command {
            "copy" => "c",
            "cut" => "x",
            "paste" => "v",
            _ => return Dispatch::default(),
        };
        let key = keyboard::Key::Character(letter.into());
        self.handle(&taim_core::Event::Keyboard(keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::CTRL,
            text: None,
            repeat: false,
        }))
    }
    #[cfg(target_os = "android")]
    pub fn semantic_selection(
        &mut self,
        target: taim_core::widget::Id,
        start: usize,
        end: usize,
    ) -> Dispatch {
        use taim_core::{Event, input_method, semantics};
        if !self.text_target_allowed(&target, false) {
            return Dispatch::default();
        }
        let response = self.native_action(&semantics::Action {
            target: target.clone(),
            request: semantics::Request::Focus,
        });
        if !response.captured {
            return response;
        }
        if !self.text_target_allowed(&target, false) {
            return Dispatch::default();
        }
        if self.root.focused_semantic_identity().as_ref() != Some(&target) {
            return Dispatch::default();
        }
        if let Some((_, mut state)) = self.root.editor_state() {
            let length = state.content.encode_utf16().count();
            if start > length || end > length {
                return Dispatch::default();
            }
            state.selection = input_method::utf16_to_utf8(&state.content, start)
                ..input_method::utf16_to_utf8(&state.content, end);
            state.composing = None;
            return self.handle(&Event::InputMethod(input_method::Event::Edit(state)));
        }
        Dispatch::default()
    }
    #[cfg(target_os = "android")]
    pub fn autofill(
        &mut self,
        target: taim_core::widget::Id,
        text: String,
        autofill: bool,
    ) -> Dispatch {
        use taim_core::{Event, input_method, semantics};
        // Focus routing revalidates modal scope and the current live node.
        if !self.text_target_allowed(&target, autofill) {
            return Dispatch::default();
        }
        let focus = self.native_action(&semantics::Action {
            target: target.clone(),
            request: semantics::Request::Focus,
        });
        if !self.text_target_allowed(&target, autofill) {
            return Dispatch::default();
        }
        if !focus.captured || self.root.focused_semantic_identity().as_ref() != Some(&target) {
            return Dispatch::default();
        }
        let _ = self.root.handle(
            &Event::InputMethod(input_method::Event::Closed),
            None,
            Instant::now(),
        );
        let key = keyboard::Key::Character("a".into());
        let _ = self.root.handle(
            &Event::Keyboard(keyboard::Event::KeyPressed {
                key: key.clone(),
                modified_key: key,
                physical_key: keyboard::key::Physical::Unidentified(
                    keyboard::key::NativeCode::Unidentified,
                ),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::CTRL,
                text: None,
                repeat: false,
            }),
            None,
            Instant::now(),
        );
        self.handle(&Event::InputMethod(input_method::Event::Commit(text)))
    }

    #[cfg(target_os = "android")]
    pub fn back(&mut self) -> Dispatch {
        let response = self.root.back(Instant::now());
        self.semantics_stale = true;
        self.apply_response(&response);
        self.checkpoint();
        self.dispatch(response)
    }

    #[cfg(target_os = "android")]
    pub fn release_memory(&mut self, compositor: &mut Compositor) {
        self.renderer = compositor.create_renderer(self.renderer_settings);
        // Renderer construction restores backend defaults, not the application's
        // lighting. Keep memory-pressure recovery visually equivalent to startup.
        self.renderer.set_light(self.look.theme.light());
        #[cfg(feature = "hinting")]
        self.renderer.hint(self.viewport.scale_factor());
        self.present_failures = 0;
        self.resized = true;
        self.stale_layout = true;
        self.semantics_stale = true;
        self.checkpoint();
        if self.target.is_some() {
            self.request_frame();
        }
    }

    #[cfg(target_os = "android")]
    pub fn recreate_graphics(&mut self, compositor: &mut Compositor) {
        let active = self.target.take().is_some();
        self.release_memory(compositor);
        if active {
            let size = self.viewport.physical_size();
            self.target =
                Some(compositor.create_surface(self.raw.clone(), size.width, size.height));
        }
        self.request_frame();
    }

    /// Matches the viewport to a new surface size or scale.
    fn fit(&mut self, physical: Size<u32>) {
        let scale = zoomed(self.raw.scale_factor(), self.root.scale());

        if self.viewport.update(physical, scale) {
            self.resized = true;
            self.content = Rectangle::with_size(self.viewport.logical_size());
            #[cfg(target_os = "android")]
            {
                self.content = self.insets.area(physical, scale);
            }
            self.stale_layout = true;
            self.request_frame();
        }
    }

    fn paint_origin(&self) -> Point {
        #[cfg(target_os = "android")]
        {
            Point::new(self.content.x, self.content.y - self.pan_y)
        }
        #[cfg(not(target_os = "android"))]
        self.content.position()
    }

    #[cfg(target_os = "android")]
    pub fn set_insets(&mut self, insets: crate::android::geometry::Insets) {
        if self.insets == insets {
            return;
        }
        self.insets = insets;
        self.semantics_stale = true;
        let content = insets.area(self.viewport.physical_size(), self.viewport.scale_factor());
        if content.height > self.content.height {
            self.pan_y = 0.0;
        }
        self.content = content;
        self.stale_layout = true;
        self.request_frame();
        log::debug!("Android safe content: {:?}", self.content);
    }

    #[cfg(target_os = "android")]
    fn update_pan(&mut self) {
        let previous = self.pan_y;
        if !self.focused {
            self.pan_y = 0.0;
            return;
        }
        let cursor = if let taim_core::input_method::InputMethod::Enabled { cursor, .. } =
            self.root.input_method()
        {
            Some(cursor)
        } else {
            self.native_focus.and_then(|(id, _, rect)| {
                self.root
                    .native_views()
                    .into_iter()
                    .find(|v| v.view.id == id && v.visible.is_some())
                    .map(|slot| {
                        Rectangle::new(
                            slot.bounds.position()
                                + (rect.position() - Point::ORIGIN) / self.viewport.scale_factor(),
                            rect.size() / self.viewport.scale_factor(),
                        )
                    })
            })
        };
        if let Some(cursor) = cursor {
            let margin = 8.0;
            if cursor.y + cursor.height > self.content.height + self.pan_y - margin {
                self.pan_y = (cursor.y + cursor.height - self.content.height + margin).max(0.0);
            } else if cursor.y < self.pan_y + margin {
                self.pan_y = (cursor.y - margin).max(0.0);
            }
        } else {
            self.pan_y = 0.0;
        }
        self.semantics_stale |= previous != self.pan_y;
    }

    fn refresh_visibility(&mut self) {
        if self.target.is_none() || self.hidden.occluded || self.hidden.empty {
            self.clock.suspend();
        } else {
            self.clock.resume();
        }
    }

    /// Shows the cursor the view under the pointer wants.
    fn update_cursor(&mut self) {
        let wanted = self.root.cursor(self.pointer());
        if wanted == self.cursor {
            return;
        }

        match conversion::mouse_interaction(wanted) {
            Some(icon) => {
                self.raw.set_cursor(icon);
                if self.cursor == mouse::Interaction::Hidden {
                    self.raw.set_cursor_visible(true);
                }
            }
            None => self.raw.set_cursor_visible(false),
        }
        self.cursor = wanted;
    }

    /// Keeps OS composition and candidate placement aligned with the
    /// focused view. Coordinates passed to winit include application zoom;
    /// winit applies the monitor scale itself.
    fn sync_input_method(&mut self) {
        use taim_core::input_method::InputMethod;
        let requested = if self.focused {
            self.root.input_method()
        } else {
            InputMethod::Disabled
        };
        #[cfg(target_os = "android")]
        {
            let secure = matches!(
                requested,
                InputMethod::Enabled {
                    purpose: taim_core::input_method::Purpose::Secure,
                    ..
                }
            );
            let editor = if secure {
                self.root.focused_identity().map(|id| {
                    (
                        id,
                        taim_core::input_method::EditorState {
                            content: String::new(),
                            selection: 0..0,
                            composing: None,
                            multiline: false,
                        },
                    )
                })
            } else if requested.is_enabled() {
                self.root.editor_state()
            } else {
                None
            };
            let cursor = match requested {
                InputMethod::Enabled { cursor, .. } => {
                    let origin = self.paint_origin();
                    Rectangle::new(
                        Point::new(cursor.x + origin.x, cursor.y + origin.y),
                        cursor.size(),
                    ) * self.viewport.scale_factor()
                }
                InputMethod::Disabled => Rectangle::default(),
            };
            self.android.sync_editor(editor, cursor, secure);
            self.android.autofill_focus(if self.focused {
                self.root.focused_semantic_identity()
            } else {
                None
            });
        }
        #[cfg(not(target_os = "android"))]
        {
            let zoom = f64::from(self.viewport.scale_factor()) / self.raw.scale_factor();
            let next = match requested {
                InputMethod::Enabled {
                    cursor, purpose, ..
                } => Some((cursor, purpose, zoom)),
                InputMethod::Disabled => None,
            };
            if next == self.ime {
                return;
            }
            if let Some((cursor, purpose, zoom)) = next {
                self.raw.set_ime_purpose(conversion::ime_purpose(purpose));
                self.raw.set_ime_cursor_area(
                    winit::dpi::LogicalPosition::new(
                        f64::from(cursor.x) * zoom,
                        f64::from(cursor.y) * zoom,
                    ),
                    winit::dpi::LogicalSize::new(
                        f64::from(cursor.width) * zoom,
                        f64::from(cursor.height) * zoom,
                    ),
                );
            }
            if next.is_some() != self.ime.is_some() {
                self.raw.set_ime_allowed(next.is_some());
            }
            self.ime = next;
        }
    }
}

/// Whether `TAIM_FRAME_TRACE` asks for one timing line per frame.
fn trace_frames() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("TAIM_FRAME_TRACE").is_some())
}

/// The platform scale times the application's zoom; never zero or
/// non-finite.
fn animation_interval(refresh: Option<u32>) -> std::time::Duration {
    let millihertz = refresh
        .filter(|rate| (1_000..=1_000_000).contains(rate))
        .unwrap_or(60_000);
    std::time::Duration::from_nanos(1_000_000_000_000 / u64::from(millihertz))
}

fn zoomed(platform: f64, zoom: f32) -> f32 {
    let usable = |scale: f32| scale.is_finite() && scale > 0.0;
    let zoom = if usable(zoom) { zoom } else { 1.0 };
    let scale = platform as f32 * zoom;

    if usable(scale) { scale } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::{animation_interval, zoomed};

    #[test]
    fn broken_scales_fall_back_to_one() {
        assert_eq!(zoomed(2.0, 1.5), 3.0);
        assert_eq!(zoomed(f64::NAN, 1.0), 1.0);
        assert_eq!(zoomed(1.0, 0.0), 1.0);
        assert_eq!(zoomed(f64::INFINITY, 1.0), 1.0);
    }
    #[test]
    fn animation_pacing_tracks_valid_monitor_rates_and_has_a_safe_fallback() {
        use std::time::Duration;
        assert_eq!(animation_interval(None), Duration::from_nanos(16_666_666));
        assert_eq!(animation_interval(Some(0)), animation_interval(None));
        assert_eq!(animation_interval(Some(u32::MAX)), animation_interval(None));
        assert_eq!(
            animation_interval(Some(120_000)),
            Duration::from_nanos(8_333_333)
        );
    }
}
