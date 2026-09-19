use super::Text;
use crate::core::alignment;
use crate::core::text::Alignment;
use crate::core::text::fallback::{FallbackChain, Script};
use crate::core::text::paragraph::Direction;
use crate::core::{Rectangle, Transformation};
use crate::graphics::text::cache::{self as text_cache, Cache as BufferCache};
use crate::graphics::text::{font_system, to_color};

use taim_text_gpu as gpu;

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    viewport: &gpu::Viewport,
    renderer: &mut gpu::TextRenderer,
    atlas: &mut gpu::TextAtlas,
    buffer_cache: &mut BufferCache,
    swash: &mut crate::graphics::text::cosmic_text::SwashCache,
    sections: &[Text],
    layer_bounds: Rectangle,
    layer_transformation: Transformation,
) -> Result<(), gpu::PrepareError> {
    renderer.begin();

    if sections.is_empty() {
        renderer.finish(device, queue);
        return Ok(());
    }

    let mut font_system = font_system()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let font_system = font_system.raw();

    for section in sections {
        let result = match section {
            Text::Paragraph {
                paragraph,
                position,
                color,
                clip_bounds,
                transformation,
            } => {
                use crate::core::text::Paragraph as _;

                let Some(paragraph) = paragraph.upgrade() else {
                    continue;
                };

                push_area(
                    device,
                    queue,
                    viewport,
                    renderer,
                    atlas,
                    font_system,
                    swash,
                    paragraph.buffer(),
                    paragraph.hint_factor(),
                    *position,
                    *color,
                    *clip_bounds,
                    *transformation,
                    layer_bounds,
                    layer_transformation,
                )
            }
            Text::Editor {
                editor,
                position,
                color,
                clip_bounds,
                transformation,
            } => {
                use crate::core::text::Editor as _;

                let Some(editor) = editor.upgrade() else {
                    continue;
                };

                push_area(
                    device,
                    queue,
                    viewport,
                    renderer,
                    atlas,
                    font_system,
                    swash,
                    editor.buffer(),
                    editor.hint_factor(),
                    *position,
                    *color,
                    *clip_bounds,
                    *transformation,
                    layer_bounds,
                    layer_transformation,
                )
            }
            Text::Cached {
                content,
                bounds,
                color,
                size,
                line_height,
                font,
                align_x,
                align_y,
                shaping,
                wrapping,
                ellipsis,
                clip_bounds,
            } => {
                let (_, entry) = buffer_cache.allocate(
                    font_system,
                    text_cache::Key {
                        content,
                        size: f32::from(*size),
                        line_height: f32::from(*line_height),
                        font: *font,
                        align_x: *align_x,
                        bounds: bounds.size(),
                        shaping: *shaping,
                        wrapping: *wrapping,
                        ellipsis: *ellipsis,
                        locale: None,
                        direction: Direction::Auto,
                        script: Script::Latin,
                        fallback: FallbackChain::for_script(Script::Latin),
                        axes: &[],
                    },
                );
                let mut position = bounds.position();
                position.x -= match align_x {
                    Alignment::Default | Alignment::Left | Alignment::Justified => 0.0,
                    Alignment::Center => entry.min_bounds.width * 0.5,
                    Alignment::Right => entry.min_bounds.width,
                };
                position.y -= match align_y {
                    alignment::Vertical::Top => 0.0,
                    alignment::Vertical::Center => entry.min_bounds.height * 0.5,
                    alignment::Vertical::Bottom => entry.min_bounds.height,
                };

                push_area(
                    device,
                    queue,
                    viewport,
                    renderer,
                    atlas,
                    font_system,
                    swash,
                    &entry.buffer,
                    None,
                    position,
                    *color,
                    *clip_bounds,
                    Transformation::IDENTITY,
                    layer_bounds,
                    layer_transformation,
                )
            }
            Text::Raw {
                raw,
                transformation,
            } => {
                let Some(buffer) = raw.buffer.upgrade() else {
                    continue;
                };

                push_area(
                    device,
                    queue,
                    viewport,
                    renderer,
                    atlas,
                    font_system,
                    swash,
                    &buffer,
                    None,
                    raw.position,
                    raw.color,
                    raw.clip_bounds,
                    *transformation,
                    layer_bounds,
                    layer_transformation,
                )
            }
        };

        if let Err(error) = result {
            // A partially prepared stream may reference glyphs displaced while
            // the atlas tried to make room. Never render that stream.
            renderer.begin();
            return Err(error);
        }
    }

    renderer.finish(device, queue);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_area(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    viewport: &gpu::Viewport,
    renderer: &mut gpu::TextRenderer,
    atlas: &mut gpu::TextAtlas,
    font_system: &mut crate::graphics::text::cosmic_text::FontSystem,
    swash: &mut crate::graphics::text::cosmic_text::SwashCache,
    buffer: &gpu::Buffer,
    hint_factor: Option<f32>,
    position: crate::core::Point,
    color: crate::core::Color,
    clip_bounds: Rectangle,
    transformation: Transformation,
    layer_bounds: Rectangle,
    layer_transformation: Transformation,
) -> Result<(), gpu::PrepareError> {
    let Some(clip_bounds) =
        layer_bounds.intersection(&(clip_bounds * transformation * layer_transformation))
    else {
        return Ok(());
    };

    let mut position = position * transformation * layer_transformation;
    let mut scale = transformation.scale_factor() * layer_transformation.scale_factor();

    if let Some(hint_factor) = hint_factor.filter(|factor| *factor > 0.0) {
        let font_size = (buffer.metrics().font_size / hint_factor).round() as u32;
        let subpixel_nudge = if font_size.is_multiple_of(2) {
            0.25
        } else if font_size.is_multiple_of(5) {
            -0.45
        } else {
            -0.25
        };

        position.x = position.x.round() + subpixel_nudge;
        scale /= hint_factor;
    }

    renderer.push(
        device,
        queue,
        font_system,
        atlas,
        viewport,
        gpu::TextArea {
            text: buffer.layout_runs(),
            left: position.x,
            top: position.y,
            scale,
            bounds: gpu::TextBounds {
                left: clip_bounds.x.round() as i32,
                top: clip_bounds.y.round() as i32,
                right: (clip_bounds.x + clip_bounds.width).round() as i32,
                bottom: (clip_bounds.y + clip_bounds.height).round() as i32,
            },
            default_color: to_color(color),
        },
        swash,
    )
}
