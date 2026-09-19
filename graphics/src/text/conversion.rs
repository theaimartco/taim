//! Conversion from TAIM text values to the shaping backend.

use crate::core::font::{self, Font};
use crate::core::text::{Alignment, Ellipsis, Shaping, Wrapping};
use crate::core::{Color, Pixels};

/// Builds backend attributes for a TAIM font descriptor.
pub fn to_attributes(font: Font) -> cosmic_text::Attrs<'static> {
    cosmic_text::Attrs::new()
        .family(family(font.family))
        .weight(weight(font.weight))
        .stretch(stretch(font.stretch))
        .style(style(font.style))
}

pub(super) fn alignment(value: Alignment) -> Option<cosmic_text::Align> {
    match value {
        Alignment::Default => None,
        Alignment::Left => Some(cosmic_text::Align::Left),
        Alignment::Center => Some(cosmic_text::Align::Center),
        Alignment::Right => Some(cosmic_text::Align::Right),
        Alignment::Justified => Some(cosmic_text::Align::Justified),
    }
}

/// Selects inexpensive shaping for ASCII when automatic mode is requested.
pub fn to_shaping(shaping: Shaping, content: &str) -> cosmic_text::Shaping {
    match shaping {
        Shaping::Auto if content.is_ascii() => cosmic_text::Shaping::Basic,
        Shaping::Auto | Shaping::Advanced => cosmic_text::Shaping::Advanced,
        Shaping::Basic => cosmic_text::Shaping::Basic,
    }
}

/// Converts TAIM wrapping policy.
pub fn to_wrap(wrapping: Wrapping) -> cosmic_text::Wrap {
    match wrapping {
        Wrapping::None => cosmic_text::Wrap::None,
        Wrapping::Word => cosmic_text::Wrap::Word,
        Wrapping::Glyph => cosmic_text::Wrap::Glyph,
        Wrapping::WordOrGlyph => cosmic_text::Wrap::WordOrGlyph,
    }
}

/// Converts ellipsis policy and sanitizes its physical height limit.
pub fn to_ellipsize(ellipsis: Ellipsis, maximum_height: f32) -> cosmic_text::Ellipsize {
    if ellipsis == Ellipsis::None {
        return cosmic_text::Ellipsize::None;
    }

    let height = if maximum_height.is_finite() {
        maximum_height.max(0.0)
    } else {
        f32::MAX
    };
    let limit = cosmic_text::EllipsizeHeightLimit::Height(height);

    match ellipsis {
        Ellipsis::None => cosmic_text::Ellipsize::None,
        Ellipsis::Start => cosmic_text::Ellipsize::Start(limit),
        Ellipsis::Middle => cosmic_text::Ellipsize::Middle(limit),
        Ellipsis::End => cosmic_text::Ellipsize::End(limit),
    }
}

/// Converts an sRGB color without allocation.
pub fn to_color(color: Color) -> cosmic_text::Color {
    let [red, green, blue, alpha] = color.into_rgba8();
    cosmic_text::Color::rgba(red, green, blue, alpha)
}

/// Returns the active glyph hinting scale, if hinting is enabled.
pub fn hint_factor(_size: Pixels, _scale_factor: Option<f32>) -> Option<f32> {
    None
}

fn family(value: font::Family) -> cosmic_text::Family<'static> {
    match value {
        font::Family::Name(name) => cosmic_text::Family::Name(name),
        font::Family::SansSerif => cosmic_text::Family::SansSerif,
        font::Family::Serif => cosmic_text::Family::Serif,
        font::Family::Cursive => cosmic_text::Family::Cursive,
        font::Family::Fantasy => cosmic_text::Family::Fantasy,
        font::Family::Monospace => cosmic_text::Family::Monospace,
    }
}

fn weight(value: font::Weight) -> cosmic_text::Weight {
    match value {
        font::Weight::Thin => cosmic_text::Weight::THIN,
        font::Weight::ExtraLight => cosmic_text::Weight::EXTRA_LIGHT,
        font::Weight::Light => cosmic_text::Weight::LIGHT,
        font::Weight::Normal => cosmic_text::Weight::NORMAL,
        font::Weight::Medium => cosmic_text::Weight::MEDIUM,
        font::Weight::Semibold => cosmic_text::Weight::SEMIBOLD,
        font::Weight::Bold => cosmic_text::Weight::BOLD,
        font::Weight::ExtraBold => cosmic_text::Weight::EXTRA_BOLD,
        font::Weight::Black => cosmic_text::Weight::BLACK,
    }
}

fn stretch(value: font::Stretch) -> cosmic_text::Stretch {
    match value {
        font::Stretch::UltraCondensed => cosmic_text::Stretch::UltraCondensed,
        font::Stretch::ExtraCondensed => cosmic_text::Stretch::ExtraCondensed,
        font::Stretch::Condensed => cosmic_text::Stretch::Condensed,
        font::Stretch::SemiCondensed => cosmic_text::Stretch::SemiCondensed,
        font::Stretch::Normal => cosmic_text::Stretch::Normal,
        font::Stretch::SemiExpanded => cosmic_text::Stretch::SemiExpanded,
        font::Stretch::Expanded => cosmic_text::Stretch::Expanded,
        font::Stretch::ExtraExpanded => cosmic_text::Stretch::ExtraExpanded,
        font::Stretch::UltraExpanded => cosmic_text::Stretch::UltraExpanded,
    }
}

fn style(value: font::Style) -> cosmic_text::Style {
    match value {
        font::Style::Normal => cosmic_text::Style::Normal,
        font::Style::Italic => cosmic_text::Style::Italic,
        font::Style::Oblique => cosmic_text::Style::Oblique,
    }
}
