//! One frame of one window: settle the interface, draw it, present it.
use super::engine::Engine;
use super::views::Views;
use crate::Proxy;
use crate::clipboard::Clipboard;
#[cfg(feature = "hinting")]
use crate::core::Renderer as _;
use crate::core::time::Instant;
use crate::core::{self, Size, renderer, theme, window};
use crate::debug;
use crate::frame_gate::{DirtyFlags, PlatformEvent};
use crate::futures::subscription;
use crate::graphics::{self, Compositor, compositor};
use crate::program::{App, AppRunner};
use crate::runtime::{Action, user_interface};

use std::slice;
use std::sync::Arc;

/// A redraw in progress.
pub(super) struct Redraw {
    pub id: window::Id,
    /// The `RedrawRequested` event every settling pass delivers.
    event: core::Event,
    /// The logical size the interface was last laid out for.
    logical_size: Size,
}

impl<A, C> Engine<A, C>
where
    A: App,
    A::Theme: theme::Base,
    C: Compositor<Renderer = A::Renderer> + 'static,
{
    /// Decides whether `platform_id` should produce a frame now and, if so,
    /// brings its surface and layout up to date with the window size.
    pub(super) fn begin_redraw(
        &mut self,
        views: &mut Views<'_, A>,
        platform_id: winit::window::WindowId,
    ) -> Option<Redraw> {
        let compositor = self.compositor.as_mut()?;
        let (id, window) = self.windows.get_mut_alias(platform_id)?;

        let physical_size = window.state.physical_size();
        let logical_size = window.state.logical_size();

        if physical_size.width == 0 || physical_size.height == 0 {
            window.set_zero_sized(true);
            let _ = self
                .frames
                .handle_event(id, PlatformEvent::SurfaceSizeChanged { zero_sized: true });
            return None;
        }

        // A redraw queued before an occlusion was processed must not submit
        // work the scheduler has already been told to stop.
        if !window.scheduler.begin_frame(Instant::now()) {
            return None;
        }

        let _ = self.frames.handle_event(id, PlatformEvent::RedrawRequested);

        if window.surface_version != window.state.surface_version() {
            #[cfg(feature = "hinting")]
            window.renderer.hint(window.state.scale_factor());

            let layout_span = debug::layout(id);
            if let Some(interface) =
                views.replace(id, |ui| ui.relayout(logical_size, &mut window.renderer))
            {
                window.hit_test_index = Some(interface.hit_test_index());
            }
            layout_span.finish();

            compositor.configure_surface(
                &mut window.surface,
                physical_size.width,
                physical_size.height,
            );
            window.surface_version = window.state.surface_version();
        }

        Some(Redraw {
            id,
            event: core::Event::Window(window::Event::RedrawRequested(Instant::now())),
            logical_size,
        })
    }

    /// Delivers the redraw event once. The interface has settled when the
    /// pass produced no messages and did not move anything.
    pub(super) fn deliver_redraw(
        &mut self,
        views: &mut Views<'_, A>,
        redraw: &Redraw,
    ) -> Option<(user_interface::State, bool)> {
        let window = self.windows.get_mut(redraw.id)?;
        let interface = views.get_mut(redraw.id)?;

        let interact_span = debug::interact(redraw.id);
        let produced_before = self.messages.len();

        let (outcome, _) = interface.update(
            &window.raw,
            &window.waker,
            slice::from_ref(&redraw.event),
            window.state.cursor(),
            &mut window.renderer,
            &mut self.messages,
        );
        interact_span.finish();

        let settled = self.messages.len() == produced_before && !outcome.has_layout_changed();

        Some((outcome, settled))
    }

    /// Runs what an update during a redraw produced. Window actions wait for
    /// the next turn: the compositor is mid-frame for `drawing`.
    pub(super) fn after_redraw_update<'a>(
        &mut self,
        runner: &'a AppRunner<A>,
        views: &mut Views<'a, A>,
        drawing: window::Id,
    ) {
        for action in std::mem::take(&mut self.staged) {
            if let Action::Window(_) = action {
                self.proxy.send_action(action);
            } else {
                self.perform(action, runner, views);
            }
        }

        for (id, window) in self.windows.iter_mut() {
            if id != drawing {
                self.frames.invalidate(id, DirtyFlags::PAINT_DIRTY);
                window.request_redraw(window::RedrawRequest::NextFrame);
            }
        }
    }

    /// Re-lays out the window being drawn if an update changed its size
    /// (typically its scale factor). Returns `false` if the window is gone.
    pub(super) fn follow_resize(&mut self, views: &mut Views<'_, A>, redraw: &mut Redraw) -> bool {
        let Some(window) = self.windows.get_mut(redraw.id) else {
            return false;
        };

        let size = window.state.logical_size();
        if size == redraw.logical_size {
            return true;
        }

        log::debug!("window {:?} was resized during its own redraw", redraw.id);
        redraw.logical_size = size;

        let layout_span = debug::layout(redraw.id);
        if let Some(interface) =
            views.replace(redraw.id, |ui| ui.relayout(size, &mut window.renderer))
        {
            window.hit_test_index = Some(interface.hit_test_index());
        }
        layout_span.finish();

        true
    }

    /// Draws the settled interface, publishes accessibility changes and
    /// presents the damaged region.
    pub(super) fn finish_redraw(
        &mut self,
        views: &mut Views<'_, A>,
        redraw: &Redraw,
        outcome: user_interface::State,
    ) {
        let id = redraw.id;
        let (Some(compositor), Some(window), Some(interface)) = (
            self.compositor.as_mut(),
            self.windows.get_mut(id),
            views.get_mut(id),
        ) else {
            return;
        };

        let cursor = window.state.cursor();

        let draw_span = debug::draw(id);
        interface.draw(
            &mut window.renderer,
            window.state.theme(),
            &renderer::Style {
                text_color: window.state.text_color(),
            },
            cursor,
        );

        // The interface keeps the previous semantic tree and returns only
        // what changed, so publishing every frame is cheap and assistive
        // technology never depends on the application remembering to do it.
        let changes = interface.semantics_changes(&window.renderer);
        window.accessibility.publish(&changes);

        let focused = interface
            .semantic_tree()
            .find(|entry| entry.state.contains(core::semantics::State::FOCUSED))
            .map(|entry| &entry.id);
        window.accessibility.focus_changed(focused);
        draw_span.finish();

        let scale_factor = window.state.viewport().scale_factor();
        let frame = match outcome.damage_region() {
            Some(region) => graphics::FrameContext::with_damage(
                graphics::damage::DamageRegion::from_core_region(region),
                scale_factor,
            ),
            None => graphics::FrameContext::full_present(scale_factor),
        };

        if let user_interface::State::Updated {
            redraw_request,
            input_method,
            mouse_interaction,
            clipboard,
            is_animating,
            ..
        } = outcome
        {
            window.request_redraw(redraw_request);
            window.request_input_method(input_method);
            window.update_mouse(mouse_interaction);

            if is_animating {
                window.scheduler.request_animation();
            } else {
                window.scheduler.stop_animating();
            }

            serve_clipboard(&mut self.proxy, &mut self.clipboard, clipboard, id);
        }

        self.runtime.broadcast(subscription::Event::Interaction {
            window: id,
            event: redraw.event.clone(),
            status: core::event::Status::Ignored,
        });

        window.draw_preedit();
        window.scheduler.submit(Instant::now());

        let present_span = debug::present(id);
        log::debug!("presenting {id:?}");

        let presented = compositor.present_with_damage(
            &mut window.renderer,
            &mut window.surface,
            window.state.viewport(),
            window.state.background_color(),
            &frame,
            || window.raw.pre_present_notify(),
        );
        present_span.finish();

        match presented {
            Ok(()) => {
                #[cfg(all(feature = "measurement", not(target_arch = "wasm32")))]
                crate::measurement::first_present();

                // No asynchronous presentation feedback is wired yet, so a
                // synchronous present that returned is its own feedback.
                window.scheduler.feedback_received();
                self.frames.on_frame_presented(id);

                if window.scheduler.should_produce_frame(Instant::now()) {
                    window.raw.request_redraw();
                }
            }
            Err(compositor::SurfaceError::OutOfMemory) => {
                panic!("the GPU ran out of memory while presenting {id:?}");
            }
            Err(error @ (compositor::SurfaceError::Outdated | compositor::SurfaceError::Lost)) => {
                window.scheduler.feedback_received();

                let size = window.state.physical_size();
                if error == compositor::SurfaceError::Lost {
                    window.surface =
                        compositor.create_surface(Arc::clone(&window.raw), size.width, size.height);
                } else {
                    compositor.configure_surface(&mut window.surface, size.width, size.height);
                }

                window.request_redraw(window::RedrawRequest::NextFrame);
            }
            Err(compositor::SurfaceError::Occluded) => {
                // The backend saw the occlusion even if the platform never
                // reported it; suspend the same way either signal would.
                window.set_occluded(true);
                let _ = self
                    .frames
                    .handle_event(id, PlatformEvent::OcclusionChanged(true));
            }
            Err(error) => {
                window.scheduler.feedback_received();
                log::warn!("presenting {id:?} failed with {error:?}; retrying every window");

                self.redraw_all();
            }
        }
    }
}

/// Carries out the clipboard reads and writes an interface asked for and
/// reports each result back to `window` as an event.
pub(super) fn serve_clipboard<Message: Send>(
    proxy: &mut Proxy<Message>,
    clipboard: &mut Clipboard,
    requests: core::Clipboard,
    window: window::Id,
) {
    let report = move |proxy: &Proxy<Message>, event| {
        proxy.send_action(Action::Event {
            window,
            event: core::Event::Clipboard(event),
        });
    };

    for kind in requests.reads {
        let proxy = proxy.clone();
        clipboard.read(kind, move |result| {
            report(&proxy, core::clipboard::Event::Read(result.map(Arc::new)));
        });
    }

    if let Some(content) = requests.write {
        let proxy = proxy.clone();
        clipboard.write(content, move |result| {
            report(&proxy, core::clipboard::Event::Written(result));
        });
    }
}
