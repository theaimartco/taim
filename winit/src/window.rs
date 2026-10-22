//! A live window: the platform window plus everything TAIM attaches to it.
mod manager;
mod preedit;
mod state;

pub use crate::core::window::RedrawRequest;
pub use manager::Manager;

use preedit::Preedit;
use state::State;

use crate::Proxy;
use crate::conversion;
use crate::core::time::Instant;
use crate::core::{self, input_method, mouse, renderer, shell, theme};
use crate::core::{InputMethod, Point, Rectangle};
use crate::graphics::Compositor;
use crate::program::{App, AppRunner};
use crate::runtime::frame_scheduler::Scheduler;

use std::sync::Arc;
use winit::dpi::{LogicalPosition, LogicalSize};

/// The accessibility session every window publishes its semantic tree to.
type Accessibility = taim_accessibility::Session<taim_accessibility::atspi::adapter::Adapter>;

pub struct Window<A, C>
where
    A: App,
    C: Compositor<Renderer = A::Renderer>,
    A::Theme: theme::Base,
{
    pub raw: Arc<winit::window::Window>,
    pub waker: shell::Waker,
    pub state: State<A>,
    pub exit_on_close_request: bool,
    pub surface: C::Surface,
    /// The [`State::surface_version`] `surface` was last configured for.
    pub surface_version: u64,
    pub renderer: A::Renderer,
    pub hit_test_index: Option<crate::runtime::hit_test::Index>,
    /// This window's frame clock. Occlusion, size and presentation feedback
    /// all flow through it; the event loop's wait deadline is derived from
    /// [`Scheduler::deadline`] instead of from raw redraw requests.
    pub scheduler: Scheduler,
    /// Retained semantic tree, diffed on every frame.
    pub accessibility: Accessibility,
    /// Carries [`Window::accessibility`]'s queue to the platform bus for as
    /// long as the window lives; dropping it stops the bridge thread.
    #[cfg(feature = "accessibility")]
    _accessibility_bridge: taim_accessibility::atspi::bridge::Bridge,
    cursor: mouse::Interaction,
    ime: Ime<A::Renderer>,
    /// Reasons the window cannot show frames right now.
    occluded: bool,
    zero_sized: bool,
}

/// Input-method state of one window.
struct Ime<Renderer: core::text::Renderer> {
    /// Caret area and purpose last sent to the platform; `None` while the
    /// input method is off.
    active: Option<(Rectangle, input_method::Purpose)>,
    /// The composition overlay, while there is something being composed.
    overlay: Option<Preedit<Renderer>>,
}

impl<A, C> Window<A, C>
where
    A: App,
    C: Compositor<Renderer = A::Renderer>,
    A::Theme: theme::Base,
{
    /// Attaches TAIM to a freshly created platform window: state, surface,
    /// renderer, frame clock, reactive waker and accessibility.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        id: core::window::Id,
        raw: Arc<winit::window::Window>,
        runner: &AppRunner<A>,
        compositor: &mut C,
        proxy: Proxy<A::Message>,
        renderer_settings: renderer::Settings,
        exit_on_close_request: bool,
        system_theme: theme::Mode,
    ) -> Self {
        let state = State::new(runner, id, &raw, system_theme);
        let size = state.physical_size();
        let zero_sized = size.width == 0 || size.height == 0;

        let mut scheduler = Scheduler::new();
        if zero_sized {
            scheduler.suspend();
        }

        // Reactive signals inside widgets wake the window through the proxy.
        let waker = shell::Waker::new_tagged(move |reactive| {
            proxy.send_action(crate::runtime::Action::Event {
                window: id,
                event: reactive.map_or(core::Event::Waken, core::Event::Reactive),
            });
        });

        let accessibility = Accessibility::new(taim_accessibility::atspi::adapter::Adapter::new());

        #[cfg(feature = "accessibility")]
        let _accessibility_bridge = taim_accessibility::atspi::bridge::Bridge::spawn(
            accessibility.adapter().store().clone(),
            accessibility.adapter().events().clone(),
            raw.title(),
        );

        Self {
            surface: compositor.create_surface(raw.clone(), size.width, size.height),
            surface_version: state.surface_version(),
            renderer: compositor.create_renderer(renderer_settings),
            raw,
            waker,
            state,
            exit_on_close_request,
            hit_test_index: None,
            scheduler,
            accessibility,
            #[cfg(feature = "accessibility")]
            _accessibility_bridge,
            cursor: mouse::Interaction::None,
            ime: Ime {
                active: None,
                overlay: None,
            },
            occluded: false,
            zero_sized,
        }
    }

    /// Outer position in logical coordinates, if the platform exposes it.
    pub fn position(&self) -> Option<Point> {
        let outer = self.raw.outer_position().ok()?;
        let logical = outer.to_logical::<f32>(self.raw.scale_factor());

        Some(Point::new(logical.x, logical.y))
    }

    /// Asks the frame clock for a frame. Only requests due now wake the
    /// platform; later ones just arm the clock's deadline.
    pub fn request_redraw(&mut self, request: RedrawRequest) {
        let Some(deadline) = request.deadline(Instant::now()) else {
            return;
        };

        self.scheduler.request_redraw_at(deadline);

        if request.wake_now() {
            self.raw.request_redraw();
        }
    }

    pub fn set_occluded(&mut self, occluded: bool) {
        self.occluded = occluded;
        self.refresh_suspension();
    }

    pub fn set_zero_sized(&mut self, zero_sized: bool) {
        self.zero_sized = zero_sized;
        self.refresh_suspension();
    }

    /// A window nobody can see produces no frames.
    fn refresh_suspension(&mut self) {
        if self.occluded || self.zero_sized {
            self.scheduler.suspend();
        } else {
            self.scheduler.resume();
        }
    }

    /// Applies what the interface wants from the input method this frame.
    pub fn request_input_method(&mut self, request: InputMethod) {
        let InputMethod::Enabled {
            cursor: caret,
            purpose,
            preedit,
        } = request
        else {
            self.disable_ime();
            return;
        };

        self.enable_ime(caret, purpose);

        self.ime.overlay = match preedit {
            Some(composing) if !composing.content.is_empty() => {
                let mut overlay = self.ime.overlay.take().unwrap_or_else(Preedit::new);
                overlay.update(
                    caret,
                    &composing,
                    self.state.background_color(),
                    &self.renderer,
                );
                Some(overlay)
            }
            _ => None,
        };
    }

    /// Shows the system cursor for `interaction`, touching the platform only
    /// when it changes.
    pub fn update_mouse(&mut self, interaction: mouse::Interaction) {
        if interaction == self.cursor {
            return;
        }

        match conversion::mouse_interaction(interaction) {
            Some(icon) => {
                self.raw.set_cursor(icon);

                if self.cursor == mouse::Interaction::Hidden {
                    self.raw.set_cursor_visible(true);
                }
            }
            None => self.raw.set_cursor_visible(false),
        }

        self.cursor = interaction;
    }

    /// Draws the composition overlay on top of the frame, if composing.
    pub fn draw_preedit(&mut self) {
        let Some(overlay) = &self.ime.overlay else {
            return;
        };

        let viewport = Rectangle::new(Point::ORIGIN, self.state.viewport().logical_size());

        overlay.draw(
            &mut self.renderer,
            self.state.text_color(),
            self.state.background_color(),
            &viewport,
        );
    }

    fn enable_ime(&mut self, caret: Rectangle, purpose: input_method::Purpose) {
        if self.ime.active.is_none() {
            self.raw.set_ime_allowed(true);
        }

        if self.ime.active == Some((caret, purpose)) {
            return;
        }

        self.raw.set_ime_cursor_area(
            LogicalPosition::new(caret.x, caret.y),
            LogicalSize::new(caret.width, caret.height),
        );
        self.raw.set_ime_purpose(conversion::ime_purpose(purpose));
        self.ime.active = Some((caret, purpose));
    }

    fn disable_ime(&mut self) {
        if self.ime.active.take().is_some() {
            self.raw.set_ime_allowed(false);
        }

        self.ime.overlay = None;
    }
}
