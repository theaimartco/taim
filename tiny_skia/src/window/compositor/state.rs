use crate::Layer;
use crate::core::{Color, backend};
use crate::graphics::compositor;

use std::collections::VecDeque;
use std::num::NonZeroU32;
use std::sync::Arc;

pub(super) const MAX_FRAME_HISTORY: usize = 8;

pub struct Compositor {
    pub(super) context: softbuffer::Context<Box<dyn compositor::Display>>,
}

pub struct Surface {
    pub(super) window:
        softbuffer::Surface<Box<dyn compositor::Display>, Box<dyn compositor::Window>>,
    pub(super) clip_mask: tiny_skia::Mask,
    pub(super) frames: VecDeque<Frame>,
    pub(super) max_age: usize,
    pub(super) working: Vec<u32>,
}

#[derive(Clone)]
pub(super) struct Frame {
    pub background: Color,
    pub layers: Arc<[Layer]>,
}

/// Connects the software compositor to `display`.
///
/// This fails when the platform refuses a software presentation context
/// (an unsupported display server, for instance). The software renderer is
/// the last fallback when no GPU adapter works, so the failure is returned
/// for the caller to report rather than aborting the process here.
pub(super) fn create_compositor(
    display: impl compositor::Display,
) -> Result<Compositor, backend::Error> {
    #[allow(unsafe_code)]
    let context = softbuffer::Context::new(Box::new(display) as _).map_err(|error| {
        backend::Error::GraphicsAdapterNotFound {
            backend: "tiny-skia",
            reason: backend::Reason::RequestFailed(error.to_string()),
        }
    })?;
    Ok(Compositor { context })
}

pub(super) fn create_surface(
    compositor: &Compositor,
    window: impl compositor::Window + Clone,
    width: u32,
    height: u32,
) -> Result<Surface, backend::Error> {
    let window = softbuffer::Surface::new(&compositor.context, Box::new(window.clone()) as _)
        .map_err(|error| backend::Error::BackendError(error.to_string()))?;
    let mut surface = Surface {
        window,
        clip_mask: tiny_skia::Mask::new(1, 1).expect("1x1 clip mask must fit"),
        frames: VecDeque::with_capacity(MAX_FRAME_HISTORY),
        max_age: 0,
        working: Vec::new(),
    };
    configure(&mut surface, width, height);
    Ok(surface)
}

pub(super) fn configure(surface: &mut Surface, width: u32, height: u32) {
    let (Some(width), Some(height)) = (NonZeroU32::new(width), NonZeroU32::new(height)) else {
        surface.frames.clear();
        surface.max_age = 0;
        surface.working.clear();
        return;
    };
    let Some(area) = (width.get() as usize).checked_mul(height.get() as usize) else {
        log::warn!("TAIM software surface rejected overflowing dimensions");
        return;
    };
    let Some(clip_mask) = tiny_skia::Mask::new(width.get(), height.get()) else {
        log::warn!("TAIM software surface could not allocate its clip mask");
        return;
    };
    let mut working = Vec::new();
    if crate::graphics::color::GAMMA_CORRECTION {
        if working.try_reserve_exact(area).is_err() {
            log::warn!("TAIM software surface could not allocate its working buffer");
            return;
        }
        working.resize(area, 0);
    }
    if surface.window.resize(width, height).is_err() {
        log::warn!("TAIM software surface resize failed");
        return;
    }

    surface.clip_mask = clip_mask;
    surface.working = working;
    surface.frames.clear();
    surface.max_age = 0;
}
