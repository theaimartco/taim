mod manager;
mod preedit;
mod state;

pub use manager::Manager;

use preedit::Preedit;
use state::State;

pub use crate::core::window::{Event, Id, RedrawRequest, Settings};

use crate::conversion;
use crate::core::input_method;
use crate::core::mouse;
use crate::core::shell;
use crate::core::theme;
use crate::core::time::Instant;
use crate::core::{InputMethod, Point, Rectangle};
use crate::graphics::Compositor;
use crate::program::App;
use crate::runtime::frame_scheduler::Scheduler;

use std::sync::Arc;
use winit::dpi::{LogicalPosition, LogicalSize};

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
    pub mouse_interaction: mouse::Interaction,
    pub surface: C::Surface,
    pub surface_version: u64,
    pub renderer: A::Renderer,
    pub hit_test_index: Option<crate::runtime::hit_test::Index>,
    /// This window's frame-production state machine (Phase 6):
    /// [`WindowEvent::Occluded`] and presentation feedback flow through
    /// it, and `ControlFlow` computation defers to
    /// [`Scheduler::should_produce_frame`]/[`Scheduler::deadline`]
    /// rather than acting on raw redraw requests directly.
    ///
    /// [`WindowEvent::Occluded`]: winit::event::WindowEvent::Occluded
    pub scheduler: Scheduler,
    is_occluded: bool,
    is_zero_sized: bool,
    /// Retained accessibility session for this window. The in-memory AT-SPI
    /// adapter is updated on the UI thread; platform bus delivery can drain
    /// its bounded event queue without blocking rendering.
    pub accessibility: taim_accessibility::Session<taim_accessibility::atspi::adapter::Adapter>,
    /// The thread carrying [`Window::accessibility`]'s queue to the platform
    /// accessibility bus.
    ///
    /// Held so it lives exactly as long as the window: dropping it asks the
    /// thread to stop. Without it the semantic tree is diffed every frame
    /// into a queue nothing reads (`Capstone.md` F-15).
    #[cfg(feature = "accessibility")]
    _accessibility_bridge: taim_accessibility::atspi::bridge::Bridge,
    preedit: Option<Preedit<A::Renderer>>,
    ime_state: Option<(Rectangle, input_method::Purpose)>,
}

impl<A, C> Window<A, C>
where
    A: App,
    C: Compositor<Renderer = A::Renderer>,
    A::Theme: theme::Base,
{
    pub fn position(&self) -> Option<Point> {
        self.raw
            .outer_position()
            .ok()
            .map(|position| position.to_logical(self.raw.scale_factor()))
            .map(|position| Point {
                x: position.x,
                y: position.y,
            })
    }

    pub fn request_redraw(&mut self, redraw_request: RedrawRequest) {
        // All widget requests feed the same TAIM-owned per-window clock.
        // Future work is queued without an unnecessary immediate OS wake.
        if let Some(deadline) = redraw_request.deadline(Instant::now()) {
            self.scheduler.request_redraw_at(deadline);
            if redraw_request.wake_now() {
                self.raw.request_redraw();
            }
        }
    }

    pub fn set_occluded(&mut self, is_occluded: bool) {
        self.is_occluded = is_occluded;
        self.synchronize_suspension();
    }

    pub fn set_zero_sized(&mut self, is_zero_sized: bool) {
        self.is_zero_sized = is_zero_sized;
        self.synchronize_suspension();
    }

    fn synchronize_suspension(&mut self) {
        if self.is_occluded || self.is_zero_sized {
            self.scheduler.suspend();
        } else {
            self.scheduler.resume();
        }
    }

    pub fn request_input_method(&mut self, input_method: InputMethod) {
        match input_method {
            InputMethod::Disabled => {
                self.disable_ime();
            }
            InputMethod::Enabled {
                cursor,
                purpose,
                preedit,
            } => {
                self.enable_ime(cursor, purpose);

                if let Some(preedit) = preedit {
                    if preedit.content.is_empty() {
                        self.preedit = None;
                    } else {
                        let mut overlay = self.preedit.take().unwrap_or_else(Preedit::new);

                        overlay.update(
                            cursor,
                            &preedit,
                            self.state.background_color(),
                            &self.renderer,
                        );

                        self.preedit = Some(overlay);
                    }
                } else {
                    self.preedit = None;
                }
            }
        }
    }

    pub fn update_mouse(&mut self, interaction: mouse::Interaction) {
        if interaction != self.mouse_interaction {
            if let Some(icon) = conversion::mouse_interaction(interaction) {
                self.raw.set_cursor(icon);

                if self.mouse_interaction == mouse::Interaction::Hidden {
                    self.raw.set_cursor_visible(true);
                }
            } else {
                self.raw.set_cursor_visible(false);
            }

            self.mouse_interaction = interaction;
        }
    }

    pub fn draw_preedit(&mut self) {
        if let Some(preedit) = &self.preedit {
            preedit.draw(
                &mut self.renderer,
                self.state.text_color(),
                self.state.background_color(),
                &Rectangle::new(Point::ORIGIN, self.state.viewport().logical_size()),
            );
        }
    }

    fn enable_ime(&mut self, cursor: Rectangle, purpose: input_method::Purpose) {
        if self.ime_state.is_none() {
            self.raw.set_ime_allowed(true);
        }

        if self.ime_state != Some((cursor, purpose)) {
            self.raw.set_ime_cursor_area(
                LogicalPosition::new(cursor.x, cursor.y),
                LogicalSize::new(cursor.width, cursor.height),
            );
            self.raw.set_ime_purpose(conversion::ime_purpose(purpose));

            self.ime_state = Some((cursor, purpose));
        }
    }

    fn disable_ime(&mut self) {
        if self.ime_state.is_some() {
            self.raw.set_ime_allowed(false);
            self.ime_state = None;
        }

        self.preedit = None;
    }
}
