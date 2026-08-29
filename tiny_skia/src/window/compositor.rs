use crate::core::backend;
use crate::core::renderer;
use crate::core::{Color, Rectangle, Size};
use crate::graphics::compositor::{self, Information};
use crate::graphics::damage;
use crate::graphics::{Shell, Viewport};
use crate::{Layer, Renderer};

use std::collections::VecDeque;
use std::num::NonZeroU32;

pub struct Compositor {
    context: softbuffer::Context<Box<dyn compositor::Display>>,
}

pub struct Surface {
    window: softbuffer::Surface<Box<dyn compositor::Display>, Box<dyn compositor::Window>>,
    clip_mask: tiny_skia::Mask,
    frames: VecDeque<Frame>,
    max_age: u8,

    /// The compositing buffer, held in the working space.
    ///
    /// Separate from the presentation buffer because the two are in different
    /// colour spaces and both persist. Damage-based redraw reuses what the
    /// last frame left behind, and blending reads it back as the destination
    /// -- so the buffer a frame accumulates into has to stay in the space
    /// blending happens in. Encoding the presentation buffer in place would
    /// feed the next frame's blends an sRGB destination and compound the
    /// error every frame the damage did not cover.
    ///
    /// Empty when the working space is already the presentation space, which
    /// is the `web-colors` compatibility case.
    working: Vec<u32>,
}

#[derive(Clone)]
struct Frame {
    background: Color,
    layers: Vec<Layer>,
}

impl crate::graphics::Compositor for Compositor {
    type Renderer = Renderer;
    type Surface = Surface;

    async fn new(
        settings: backend::Settings,
        display: impl compositor::Display,
        _compatible_window: impl compositor::Window,
        _shell: Shell,
    ) -> Result<Self, backend::Error> {
        if !settings.backend.is_software() && !settings.backend.matches("tiny-skia") {
            return Err(backend::Error::GraphicsAdapterNotFound {
                backend: "tiny-skia",
                reason: backend::Reason::DidNotMatch {
                    preferred_backend: settings.backend,
                },
            });
        }

        Ok(new(display))
    }

    fn create_renderer(&self, settings: renderer::Settings) -> Self::Renderer {
        Renderer::new(settings)
    }

    fn create_surface(
        &mut self,
        window: impl compositor::Window + Clone,
        width: u32,
        height: u32,
    ) -> Self::Surface {
        let window = softbuffer::Surface::new(&self.context, Box::new(window.clone()) as _)
            .expect("Create softbuffer surface for window");

        let mut surface = Surface {
            window,
            clip_mask: tiny_skia::Mask::new(1, 1).expect("Create clip mask"),
            frames: VecDeque::new(),
            max_age: 0,
            working: Vec::new(),
        };

        if width > 0 && height > 0 {
            self.configure_surface(&mut surface, width, height);
        }

        surface
    }

    fn configure_surface(&mut self, surface: &mut Self::Surface, width: u32, height: u32) {
        surface
            .window
            .resize(
                NonZeroU32::new(width).expect("Non-zero width"),
                NonZeroU32::new(height).expect("Non-zero height"),
            )
            .expect("Resize surface");

        surface.clip_mask = tiny_skia::Mask::new(width, height).expect("Create clip mask");
        surface.frames.clear();

        if crate::graphics::color::GAMMA_CORRECTION {
            surface.working.resize(width as usize * height as usize, 0);
            surface.working.fill(0);
        }
    }

    fn information(&self) -> Information {
        Information {
            adapter: String::from("CPU"),
            backend: String::from("tiny-skia"),
        }
    }

    fn present(
        &mut self,
        renderer: &mut Self::Renderer,
        surface: &mut Self::Surface,
        viewport: &Viewport,
        background_color: Color,
        on_pre_present: impl FnOnce(),
    ) -> Result<(), compositor::SurfaceError> {
        present(
            renderer,
            surface,
            viewport,
            background_color,
            on_pre_present,
        )
    }

    fn screenshot(
        &mut self,
        renderer: &mut Self::Renderer,
        viewport: &Viewport,
        background_color: Color,
    ) -> Vec<u8> {
        screenshot(renderer, viewport, background_color)
    }
}

pub fn new(display: impl compositor::Display) -> Compositor {
    #[allow(unsafe_code)]
    let context =
        softbuffer::Context::new(Box::new(display) as _).expect("Create softbuffer context");

    Compositor { context }
}

pub fn present(
    renderer: &mut Renderer,
    surface: &mut Surface,
    viewport: &Viewport,
    background: Color,
    on_pre_present: impl FnOnce(),
) -> Result<(), compositor::SurfaceError> {
    let physical_size = viewport.physical_size();

    let mut buffer = surface
        .window
        .buffer_mut()
        .map_err(|_| compositor::SurfaceError::Lost)?;

    let last_frame = {
        let age = buffer.age();

        surface.max_age = surface.max_age.max(age);
        surface.frames.truncate(surface.max_age as usize);

        if age > 0 {
            surface.frames.get(age as usize - 1)
        } else {
            None
        }
    };

    let damage = last_frame
        .and_then(|last_frame| {
            (last_frame.background == background).then(|| {
                damage::diff(
                    &last_frame.layers,
                    renderer.layers(),
                    |layer| vec![layer.bounds],
                    Layer::damage,
                )
            })
        })
        .unwrap_or_else(|| vec![Rectangle::with_size(viewport.logical_size())]);

    if damage.is_empty() {
        if let Some(last_frame) = last_frame {
            surface.frames.push_front(last_frame.clone());
        }
    } else {
        surface.frames.push_front(Frame {
            background,
            layers: renderer.layers().to_vec(),
        });

        let damage = damage::group(damage, Rectangle::with_size(viewport.logical_size()));

        if crate::graphics::color::GAMMA_CORRECTION {
            // Composite in the working space, then encode into what the
            // window actually presents.
            surface.working.resize(
                physical_size.width as usize * physical_size.height as usize,
                0,
            );

            let mut pixels = tiny_skia::PixmapMut::from_bytes(
                bytemuck::cast_slice_mut(&mut surface.working),
                physical_size.width,
                physical_size.height,
            )
            .expect("Create pixel map");

            renderer.draw(
                &mut pixels,
                &mut surface.clip_mask,
                viewport,
                &damage,
                background,
            );

            encode_damage(
                &surface.working,
                &mut buffer,
                physical_size,
                &damage,
                viewport.scale_factor(),
            );
        } else {
            let mut pixels = tiny_skia::PixmapMut::from_bytes(
                bytemuck::cast_slice_mut(&mut buffer),
                physical_size.width,
                physical_size.height,
            )
            .expect("Create pixel map");

            renderer.draw(
                &mut pixels,
                &mut surface.clip_mask,
                viewport,
                &damage,
                background,
            );
        }
    }

    on_pre_present();
    buffer.present().map_err(|_| compositor::SurfaceError::Lost)
}

/// Encodes one pixel from the working space into the presentation space.
///
/// Alpha is coverage, not light, and is copied through untouched.
#[inline]
fn encode_pixel(source: u32) -> u32 {
    let alpha = source & 0xFF_00_00_00;
    let red = crate::engine::encode_channel(((source >> 16) & 0xFF) as u8);
    let green = crate::engine::encode_channel(((source >> 8) & 0xFF) as u8);
    let blue = crate::engine::encode_channel((source & 0xFF) as u8);

    alpha | (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
}

/// Encodes the damaged region of a working-space buffer into the presentation
/// buffer.
///
/// Only the damaged region, because the whole point of damage tracking is that
/// an idle frame touches almost nothing. Encoding the full buffer would undo
/// that: at 1920x1080 it is six million table lookups per frame whether or not
/// anything moved, which costs more than the drawing did.
///
/// The presentation buffer persists between frames, so pixels outside the
/// damage still hold the correctly encoded result of the frame that drew them.
fn encode_damage(
    working: &[u32],
    presented: &mut [u32],
    size: Size<u32>,
    damage: &[Rectangle],
    scale_factor: f32,
) {
    for region in damage {
        // Damage is logical; the buffers are physical. Round outwards so a
        // fractional scale factor never leaves a seam of stale pixels.
        let left = ((region.x * scale_factor).floor().max(0.0) as u32).min(size.width);
        let top = ((region.y * scale_factor).floor().max(0.0) as u32).min(size.height);
        let right =
            (((region.x + region.width) * scale_factor).ceil().max(0.0) as u32).min(size.width);
        let bottom =
            (((region.y + region.height) * scale_factor).ceil().max(0.0) as u32).min(size.height);

        for y in top..bottom {
            let row = (y * size.width) as usize;
            let span = row + left as usize..row + right as usize;

            for index in span {
                presented[index] = encode_pixel(working[index]);
            }
        }
    }
}

pub fn screenshot(
    renderer: &mut Renderer,
    viewport: &Viewport,
    background_color: Color,
) -> Vec<u8> {
    let size = viewport.physical_size();

    let mut offscreen_buffer: Vec<u32> = vec![0; size.width as usize * size.height as usize];

    let mut clip_mask = tiny_skia::Mask::new(size.width, size.height).expect("Create clip mask");

    renderer.draw(
        &mut tiny_skia::PixmapMut::from_bytes(
            bytemuck::cast_slice_mut(&mut offscreen_buffer),
            size.width,
            size.height,
        )
        .expect("Create offscreen pixel map"),
        &mut clip_mask,
        viewport,
        &[Rectangle::with_size(Size::new(
            size.width as f32,
            size.height as f32,
        ))],
        background_color,
    );

    offscreen_buffer.iter().fold(
        Vec::with_capacity(offscreen_buffer.len() * 4),
        |mut acc, pixel| {
            const A_MASK: u32 = 0xFF_00_00_00;
            const R_MASK: u32 = 0x00_FF_00_00;
            const G_MASK: u32 = 0x00_00_FF_00;
            const B_MASK: u32 = 0x00_00_00_FF;

            let a = ((A_MASK & pixel) >> 24) as u8;
            let r = ((R_MASK & pixel) >> 16) as u8;
            let g = ((G_MASK & pixel) >> 8) as u8;
            let b = (B_MASK & pixel) as u8;

            // The pixmap holds the working space, which is linear light on
            // the production path. Encode once, here, as the frame leaves.
            let encode = |channel: u8| {
                if crate::graphics::color::GAMMA_CORRECTION {
                    crate::engine::encode_channel(channel)
                } else {
                    channel
                }
            };

            acc.extend([encode(r), encode(g), encode(b), a]);
            acc
        },
    )
}
