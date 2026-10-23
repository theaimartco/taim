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

    target: <Compositor as compositor::Compositor>::Surface,
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

    accessibility: Accessibility,
    published: taim_accessibility::Tree,
    /// Something that can change the semantic tree happened since it was
    /// last published. A pointer merely moving over the window does not.
    semantics_stale: bool,
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
            target: compositor.create_surface(raw.clone(), physical.width, physical.height),
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
            accessibility,
            published: taim_accessibility::Tree::new(),
            semantics_stale: true,
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
    pub fn observe(&mut self, event: WindowEvent, system: theme::Mode) -> Option<taim_core::Event> {
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
            WindowEvent::CursorMoved { position, .. } => self.pointer = Some(*position),
            WindowEvent::Touch(touch) => self.pointer = Some(touch.location),
            WindowEvent::CursorLeft { .. } => self.pointer = None,
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = conversion::modifiers(modifiers.state());
            }
            _ => {}
        }

        conversion::window_event(event, self.viewport.scale_factor(), self.modifiers)
    }

    /// The pointer in logical coordinates.
    pub fn pointer(&self) -> Option<Point> {
        self.pointer
            .map(|at| conversion::cursor_position(at, self.viewport.scale_factor()))
    }

    /// Routes `event` to the views.
    pub fn handle(&mut self, event: &taim_core::Event) -> Response {
        let response = self.root.handle(event, self.pointer(), Instant::now());

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

        if self.clock.is_suspended() {
            return;
        }
        // A frame the platform asked for (an expose) is drawn even if the
        // clock had nothing scheduled.
        let _ = self.clock.begin_frame(now);

        if self.resized {
            let size = self.viewport.physical_size();
            compositor.configure_surface(&mut self.target, size.width, size.height);
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
        let presented = compositor.present(
            &mut self.renderer,
            &mut self.target,
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
                self.target = compositor.create_surface(self.raw.clone(), size.width, size.height);
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
        // Without the AT-SPI bridge nothing reads the published tree.
        if !cfg!(feature = "accessibility") || !self.semantics_stale {
            return;
        }
        self.semantics_stale = false;

        let window = self.root.describe(self.viewport.logical_size());
        let next = taim_accessibility::Tree::from_forest(&[window]);
        let changes = self.published.diff(&next);
        self.published = next;

        self.accessibility.publish(&changes);
        let focused = self
            .published
            .find(|entry| {
                entry
                    .state
                    .contains(taim_core::semantics::State::FOCUSED)
            })
            .map(|entry| &entry.id);
        self.accessibility.focus_changed(focused);
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
        if self.hidden.occluded || self.hidden.empty {
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
