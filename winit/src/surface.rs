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

type Accessibility = taim_accessibility::Session<taim_accessibility::atspi::adapter::Adapter>;

/// A window and everything the shell keeps for it.
pub(crate) struct Surface {
    pub id: window::Id,
    pub raw: Arc<winit::window::Window>,
    pub exit_on_close: bool,
    pub changes: ChangeSignal,
    pub root: Root,

    /// The GPU surface. `None` while the platform has taken the native
    /// window away (an Android application in the background).
    target: Option<<Compositor as compositor::Compositor>::Surface>,
    renderer: Renderer,
    viewport: Viewport,
    /// The surface must be reconfigured to the viewport before drawing.
    resized: bool,
    /// Sizes may have changed since the last layout.
    stale_layout: bool,
    clock: Scheduler,

    look: Look,
    pointer: Option<PhysicalPosition<f64>>,
    modifiers: keyboard::Modifiers,
    cursor: mouse::Interaction,
    hidden: Hidden,
    /// The finger turned into pointer events; see [`Surface::touch`].
    fingers: crate::touch::Fingers,
    /// The pointer is currently a finger.
    touch: bool,
    /// The on-screen keyboard was last asked to be shown.
    text_input: bool,

    accessibility: Accessibility,
    published: taim_accessibility::Tree,
    /// Something that can change the semantic tree happened since it was
    /// last published. A pointer merely moving over the window does not.
    semantics_stale: bool,
    #[cfg(feature = "accessibility")]
    _bridge: taim_accessibility::atspi::bridge::Bridge,
    /// TalkBack, once attached.
    #[cfg(target_os = "android")]
    talkback: Option<crate::android::TalkBack>,
    /// The activity, for its input method's text buffer.
    #[cfg(target_os = "android")]
    android: Option<crate::AndroidApp>,
    /// What the input method's buffer has typed into the focused view.
    #[cfg(target_os = "android")]
    typing: crate::typing::Typing,
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
    /// The platform took the native window away.
    suspended: bool,
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
    ) -> Self {
        let physical = raw.inner_size();
        let viewport = Viewport::with_physical_size(
            Size::new(physical.width, physical.height),
            zoomed(raw.scale_factor(), root.scale()),
        );

        let accessibility = Accessibility::new(taim_accessibility::atspi::adapter::Adapter::new());

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
            raw,
            exit_on_close,
            changes,
            root,
            viewport,
            resized: false,
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
            fingers: crate::touch::Fingers::default(),
            touch: false,
            text_input: false,
            accessibility,
            published: taim_accessibility::Tree::new(),
            semantics_stale: true,
            #[cfg(feature = "accessibility")]
            _bridge,
            #[cfg(target_os = "android")]
            talkback: None,
            #[cfg(target_os = "android")]
            android: None,
            #[cfg(target_os = "android")]
            typing: crate::typing::Typing::default(),
        };

        surface.hidden.empty = physical.width == 0 || physical.height == 0;
        surface.refresh_visibility();
        let _ = surface.root.settings_changed();
        surface.restyle(system);

        #[cfg(feature = "hinting")]
        surface.renderer.hint(surface.viewport.scale_factor());

        surface
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

    /// Lets go of the GPU surface: the platform is taking the native window
    /// away. Nothing is drawn until [`Surface::resume`].
    pub fn suspend(&mut self) {
        self.target = None;
        self.fingers.reset();
        self.hidden.suspended = true;
        self.refresh_visibility();
    }

    /// Creates a new GPU surface for the native window the platform gave
    /// back, and draws.
    pub fn resume(&mut self, compositor: &mut Compositor) {
        let physical = self.raw.inner_size();
        self.target =
            Some(compositor.create_surface(self.raw.clone(), physical.width, physical.height));
        self.hidden.suspended = false;
        self.hidden.empty = physical.width == 0 || physical.height == 0;
        self.refresh_visibility();
        self.fit(Size::new(physical.width, physical.height));
        self.resized = true;
        self.stale_layout = true;
        self.request_frame();
    }

    /// Sets the window edges the platform covers, in physical pixels:
    /// top, right, bottom, left. Views are laid out inside them.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn set_insets(&mut self, top: f32, right: f32, bottom: f32, left: f32) {
        let scale = self.viewport.scale_factor();
        let logical = taim_core::Padding {
            top: top / scale,
            right: right / scale,
            bottom: bottom / scale,
            left: left / scale,
        };

        if self.root.set_insets(logical) {
            self.stale_layout = true;
            self.semantics_stale = true;
            self.request_frame();
        }
    }

    /// Folds a platform event into the window's own state. Returns the
    /// TAIM events the views should see, in order.
    pub fn observe(
        &mut self,
        event: WindowEvent,
        system: theme::Mode,
    ) -> [Option<taim_core::Event>; 3] {
        if let WindowEvent::Touch(contact) = event {
            return self.touch(contact);
        }

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
            WindowEvent::Focused(_) => self.request_frame(),
            WindowEvent::ThemeChanged(_) => self.restyle(system),
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = Some(*position);
                self.touch = false;
            }
            WindowEvent::CursorLeft { .. } => self.pointer = None,
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = conversion::modifiers(modifiers.state());
            }
            _ => {}
        }

        [
            conversion::window_event(event, self.viewport.scale_factor(), self.modifiers),
            None,
            None,
        ]
    }

    /// Turns the first finger on the screen into pointer events (a move, a
    /// press, a release), so every view that works with a mouse works with
    /// touch, the way Flutter and the web unify pointers. Views that act
    /// differently for a finger, such as dragging to scroll, are told it is
    /// one. Every finger, the first included, is also delivered as a raw
    /// touch event, for views that follow several (pinching).
    ///
    /// A cancelled touch (the system took the gesture) leaves before it
    /// releases, so nothing under it activates.
    fn touch(&mut self, contact: winit::event::Touch) -> [Option<taim_core::Event>; 3] {
        use crate::touch::Step;

        let raw = conversion::window_event(
            WindowEvent::Touch(contact),
            self.viewport.scale_factor(),
            self.modifiers,
        );

        let position = conversion::cursor_position(contact.location, self.viewport.scale_factor());
        let moved = taim_core::Event::Mouse(mouse::Event::CursorMoved { position });
        let pressed = taim_core::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        let released = taim_core::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        let left = taim_core::Event::Mouse(mouse::Event::CursorLeft);

        match self.fingers.step(contact.id, contact.phase) {
            Step::Press => {
                self.touch = true;
                self.pointer = Some(contact.location);
                [Some(moved), Some(pressed), raw]
            }
            Step::Move => {
                self.pointer = Some(contact.location);
                [Some(moved), raw, None]
            }
            Step::Release => {
                self.pointer = Some(contact.location);
                [Some(released), Some(left), raw]
            }
            Step::Cancel => [Some(left), Some(released), raw],
            Step::Ignore => [raw, None, None],
        }
    }

    /// The pointer in logical coordinates.
    pub fn pointer(&self) -> Option<Point> {
        self.pointer
            .map(|at| conversion::cursor_position(at, self.viewport.scale_factor()))
    }

    /// Routes `event` to the views.
    pub fn handle(&mut self, event: &taim_core::Event) -> Response {
        if matches!(event, taim_core::Event::Mouse(mouse::Event::CursorLeft)) {
            self.pointer = None;
        }

        // A press may move the caret; what the input method's buffer holds
        // no longer sits just before it.
        #[cfg(target_os = "android")]
        if self.text_input
            && matches!(
                event,
                taim_core::Event::Mouse(mouse::Event::ButtonPressed(_))
            )
        {
            self.clear_typing();
        }

        let response = self
            .root
            .handle_input(event, self.pointer(), Instant::now(), self.touch);
        self.sync_text_input();

        if response.paint
            || response.captured
            || !matches!(
                event,
                taim_core::Event::Mouse(mouse::Event::CursorMoved { .. })
            )
        {
            self.semantics_stale = true;
        }

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
        response
    }

    /// Shows the on-screen keyboard while a text view has focus, and hides
    /// it when none does.
    fn sync_text_input(&mut self) {
        let wants = self.root.wants_text_input();
        if wants == self.text_input {
            return;
        }
        self.text_input = wants;

        #[cfg(target_os = "android")]
        self.clear_typing();

        // Phones need the on-screen keyboard. Desktop input methods are not
        // wired up yet: enabling one there changes how typed text arrives.
        if cfg!(any(target_os = "android", target_os = "ios")) {
            self.raw.set_ime_allowed(wants);
        }
    }

    /// Picks up state changes the views were told about.
    pub fn absorb_changes(&mut self, system: theme::Mode) {
        self.changes.lower();

        if self.root.settings_changed() {
            self.restyle(system);
            let size = self.viewport.physical_size();
            self.fit(size);
            self.stale_layout = true;
        }

        let (paint, layout) = self.root.take_changes();
        if layout {
            self.stale_layout = true;
        }
        if paint || layout {
            self.semantics_stale = true;
            self.request_frame();
        }
    }

    /// Re-resolves the theme after the application or the desktop changed
    /// it.
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

        self.renderer.set_light(theme.light());
        self.look.style = theme.base();
        self.root.set_theme(theme.clone());
        self.look.theme = theme;
        self.stale_layout = true;
        self.request_frame();
    }

    /// Draws and presents one frame.
    pub fn draw(&mut self, compositor: &mut Compositor) {
        let now = Instant::now();
        let started = std::time::Instant::now();

        if self.clock.is_suspended() || self.target.is_none() {
            return;
        }
        // A frame the platform asked for (an expose) is drawn even if the
        // clock had nothing scheduled.
        let _ = self.clock.begin_frame(now);

        if self.resized {
            let size = self.viewport.physical_size();
            if let Some(target) = self.target.as_mut() {
                compositor.configure_surface(target, size.width, size.height);
            }
            #[cfg(feature = "hinting")]
            self.renderer.hint(self.viewport.scale_factor());
            self.resized = false;
            self.stale_layout = true;
        }

        let (_, layout) = self.root.take_changes();
        if layout || self.stale_layout {
            self.root
                .layout(&self.renderer, self.viewport.logical_size());
            self.stale_layout = false;
        }

        let pointer = self.pointer();
        let bounds = Rectangle::with_size(self.viewport.logical_size());
        self.renderer.reset(bounds);
        self.root
            .paint(&mut self.renderer, self.look.style.text_color, pointer, now);
        let painted = started.elapsed();
        self.publish_semantics();
        self.update_cursor();
        let described = started.elapsed();

        self.clock.submit(now);
        let Some(target) = self.target.as_mut() else {
            return;
        };
        let presented = compositor.present(
            &mut self.renderer,
            target,
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

        match presented {
            Ok(()) => {
                #[cfg(all(feature = "measurement", not(target_arch = "wasm32")))]
                crate::measurement::first_present();
            }
            Err(compositor::SurfaceError::OutOfMemory) => {
                panic!("the GPU ran out of memory while presenting {:?}", self.id);
            }
            Err(compositor::SurfaceError::Lost) => {
                let size = self.viewport.physical_size();
                self.target =
                    Some(compositor.create_surface(self.raw.clone(), size.width, size.height));
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

        if self.clock.should_produce_frame(Instant::now()) {
            self.raw.request_redraw();
        }
    }

    /// Diffs the views' semantic tree against what assistive technology
    /// last saw and sends the difference.
    fn publish_semantics(&mut self) {
        if !self.semantics_stale {
            return;
        }

        #[cfg(target_os = "android")]
        let talkback = self
            .talkback
            .as_ref()
            .is_some_and(crate::android::TalkBack::is_active);
        #[cfg(not(target_os = "android"))]
        let talkback = false;
        let atspi = cfg!(feature = "accessibility");

        // Nothing reads the tree: do not build it.
        if !talkback && !atspi {
            return;
        }
        self.semantics_stale = false;

        let window = self.root.describe(self.viewport.logical_size());

        #[cfg(target_os = "android")]
        if let Some(talkback) = self
            .talkback
            .as_mut()
            .filter(|talkback| talkback.is_active())
        {
            talkback.publish(&window, self.viewport.scale_factor());
        }

        if !atspi {
            return;
        }

        let next = taim_accessibility::Tree::from_forest(&[window]);
        let changes = self.published.diff(&next);
        self.published = next;

        self.accessibility.publish(&changes);
        let focused = self
            .published
            .find(|entry| entry.state.contains(taim_core::semantics::State::FOCUSED))
            .map(|entry| &entry.id);
        self.accessibility.focus_changed(focused);
    }

    /// Connects the window to the activity: TalkBack and the input
    /// method's text buffer.
    #[cfg(target_os = "android")]
    pub fn attach_android(&mut self, android: &crate::AndroidApp) {
        self.talkback = crate::android::TalkBack::attach(android, self.id, self.changes.proxy());
        self.android = Some(android.clone());
    }

    /// Empties the input method's buffer, so it starts a new word with
    /// nothing before it that TAIM would have to reconcile.
    #[cfg(target_os = "android")]
    fn clear_typing(&mut self) {
        use winit::platform::android::activity::input::{TextInputState, TextSpan};

        self.typing.reset();
        if let Some(android) = &self.android {
            android.set_text_input_state(TextInputState {
                text: String::new(),
                selection: TextSpan { start: 0, end: 0 },
                compose_region: None,
            });
        }
    }

    /// Replays what the input method changed in its buffer as typing:
    /// Backspace for each character it took back, then the new text. Only
    /// GameActivity has such a buffer; with NativeActivity the on-screen
    /// keyboard sends key events and this finds nothing to do.
    #[cfg(target_os = "android")]
    pub fn follow_typing(&mut self) -> Vec<Response> {
        if !self.text_input {
            return Vec::new();
        }
        let Some(android) = &self.android else {
            return Vec::new();
        };

        let state = android.text_input_state();
        let Some(edit) = self.typing.update(&state.text) else {
            return Vec::new();
        };

        let mut responses = Vec::new();
        for _ in 0..edit.delete {
            responses.push(self.handle(&backspace()));
        }
        if !edit.insert.is_empty() {
            responses.push(self.handle(&taim_core::Event::InputMethod(
                taim_core::input_method::Event::Commit(edit.insert),
            )));
        }

        // Between words the buffer can be emptied without disturbing the
        // input method; while it composes, it must be left alone.
        if state.compose_region.is_none() && !state.text.is_empty() {
            self.clear_typing();
        }

        responses
    }

    /// A screen reader started reading: publish the whole tree next frame.
    #[cfg(target_os = "android")]
    pub fn describe_again(&mut self) {
        self.semantics_stale = true;
        self.request_frame();
    }

    /// Carries out a screen reader's action as the input a person would
    /// give: a tap at the node's center to activate, focus, expand or
    /// collapse it; a wheel turn over it to scroll. Returns what the views
    /// asked of the shell.
    #[cfg(target_os = "android")]
    pub fn accessibility_action(&mut self, request: &accesskit::ActionRequest) -> Vec<Response> {
        use accesskit::Action;

        let Some(target) = self
            .talkback
            .as_ref()
            .and_then(|talkback| talkback.target(request.target_node))
        else {
            return Vec::new();
        };

        let at = target.center();
        let page = (target.bounds.height * 0.8).max(40.0);
        let press = [
            taim_core::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            taim_core::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ];
        let wheel = |y: f32| {
            [taim_core::Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: 0.0, y },
            })]
        };

        let events: &[taim_core::Event] = match request.action {
            Action::Click | Action::Focus | Action::Expand | Action::Collapse => &press,
            Action::ScrollDown => &wheel(-page),
            Action::ScrollUp => &wheel(page),
            _ => return Vec::new(),
        };

        let scale = f64::from(self.viewport.scale_factor());
        self.pointer = Some(PhysicalPosition::new(
            f64::from(at.x) * scale,
            f64::from(at.y) * scale,
        ));
        self.touch = false;

        let mut responses =
            vec![
                self.handle(&taim_core::Event::Mouse(mouse::Event::CursorMoved {
                    position: at,
                })),
            ];
        for event in events {
            responses.push(self.handle(event));
        }
        responses.push(self.handle(&taim_core::Event::Mouse(mouse::Event::CursorLeft)));
        responses
    }

    /// Matches the viewport to a new surface size or scale.
    fn fit(&mut self, physical: Size<u32>) {
        let scale = zoomed(self.raw.scale_factor(), self.root.scale());

        if self.viewport.update(physical, scale) {
            self.resized = true;
            self.stale_layout = true;
            self.request_frame();
        }
    }

    fn refresh_visibility(&mut self) {
        if self.hidden.occluded || self.hidden.empty || self.hidden.suspended {
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
}

/// A Backspace press, as an input method's deletion is replayed.
#[cfg(target_os = "android")]
fn backspace() -> taim_core::Event {
    use taim_core::keyboard::{self, key};

    taim_core::Event::Keyboard(keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(key::Named::Backspace),
        modified_key: keyboard::Key::Named(key::Named::Backspace),
        physical_key: key::Physical::Unidentified(key::NativeCode::Unidentified),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::empty(),
        text: None,
        repeat: false,
    })
}

/// Whether `TAIM_FRAME_TRACE` asks for one timing line per frame.
fn trace_frames() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("TAIM_FRAME_TRACE").is_some())
}

/// The platform scale times the application's zoom; never zero or
/// non-finite.
fn zoomed(platform: f64, zoom: f32) -> f32 {
    let usable = |scale: f32| scale.is_finite() && scale > 0.0;
    let zoom = if usable(zoom) { zoom } else { 1.0 };
    let scale = platform as f32 * zoom;

    if usable(scale) { scale } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::zoomed;

    #[test]
    fn broken_scales_fall_back_to_one() {
        assert_eq!(zoomed(2.0, 1.5), 3.0);
        assert_eq!(zoomed(f64::NAN, 1.0), 1.0);
        assert_eq!(zoomed(1.0, 0.0), 1.0);
        assert_eq!(zoomed(f64::INFINITY, 1.0), 1.0);
    }
}
