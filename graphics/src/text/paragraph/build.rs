//! Paragraph construction shared by plain and attributed text.

use super::{Internal, Paragraph};
use crate::core::alignment;
use crate::core::text::{Alignment, Ellipsis, Shaping, Span, Text, Wrapping};
use crate::core::Font;
use crate::text;

use rustc_hash::{FxHashMap, FxHasher};
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Debug)]
struct CachedParagraph {
    content: String,
    font: Font,
    size_bits: u32,
    line_height_bits: u32,
    bounds_w_bits: u32,
    bounds_h_bits: u32,
    align_x: Alignment,
    align_y: alignment::Vertical,
    wrapping: Wrapping,
    ellipsis: Ellipsis,
    shaping: Shaping,
    hint_factor_bits: u32,
    version: text::font_system::Version,
    paragraph: Paragraph,
    clock: u64,
}

struct ParagraphCache {
    entries: FxHashMap<u64, CachedParagraph>,
    clock: u64,
}

impl ParagraphCache {
    const CAPACITY: usize = 2048;

    fn new() -> Self {
        Self {
            entries: FxHashMap::default(),
            clock: 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn get(
        &mut self,
        definition: &Text<&str>,
        size_bits: u32,
        line_height_bits: u32,
        bounds_w_bits: u32,
        bounds_h_bits: u32,
        hint_factor_bits: u32,
        version: text::font_system::Version,
        hash: u64,
    ) -> Option<Paragraph> {
        let entry = self.entries.get_mut(&hash)?;

        if entry.version == version
            && entry.font == definition.font
            && entry.size_bits == size_bits
            && entry.line_height_bits == line_height_bits
            && entry.bounds_w_bits == bounds_w_bits
            && entry.bounds_h_bits == bounds_h_bits
            && entry.align_x == definition.align_x
            && entry.align_y == definition.align_y
            && entry.wrapping == definition.wrapping
            && entry.ellipsis == definition.ellipsis
            && entry.shaping == definition.shaping
            && entry.hint_factor_bits == hint_factor_bits
            && entry.content == definition.content
        {
            self.clock = self.clock.wrapping_add(1);
            entry.clock = self.clock;
            Some(entry.paragraph.clone())
        } else {
            None
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn insert(
        &mut self,
        definition: &Text<&str>,
        size_bits: u32,
        line_height_bits: u32,
        bounds_w_bits: u32,
        bounds_h_bits: u32,
        hint_factor_bits: u32,
        version: text::font_system::Version,
        hash: u64,
        paragraph: Paragraph,
    ) {
        if self.entries.len() >= Self::CAPACITY
            && let Some((&oldest_key, _)) = self.entries.iter().min_by_key(|(_, e)| e.clock)
        {
            let _ = self.entries.remove(&oldest_key);
        }

        self.clock = self.clock.wrapping_add(1);

        let _ = self.entries.insert(
            hash,
            CachedParagraph {
                content: definition.content.to_string(),
                font: definition.font,
                size_bits,
                line_height_bits,
                bounds_w_bits,
                bounds_h_bits,
                align_x: definition.align_x,
                align_y: definition.align_y,
                wrapping: definition.wrapping,
                ellipsis: definition.ellipsis,
                shaping: definition.shaping,
                hint_factor_bits,
                version,
                paragraph,
                clock: self.clock,
            },
        );
    }
}

thread_local! {
    static CACHE: RefCell<ParagraphCache> = RefCell::new(ParagraphCache::new());
}

fn quantize_definition(definition: &Text<&str>) -> (f32, u32, u32, u32, u32, u32) {
    let hint = text::hint_factor(definition.size, definition.hint_factor);
    let scale = hint.unwrap_or(1.0);
    let size_bits = (definition.size.0 * scale).to_bits();
    let line_height_bits = (definition.line_height.to_absolute(definition.size).0 * scale).to_bits();
    let bounds_w_bits = (definition.bounds.width * scale).to_bits();
    let bounds_h_bits = (definition.bounds.height * scale).to_bits();
    let hint_factor_bits = definition.hint_factor.unwrap_or(1.0).to_bits();
    (scale, size_bits, line_height_bits, bounds_w_bits, bounds_h_bits, hint_factor_bits)
}

fn compute_hash(
    definition: &Text<&str>,
    size_bits: u32,
    line_height_bits: u32,
    bounds_w_bits: u32,
    bounds_h_bits: u32,
    hint_factor_bits: u32,
    version: text::font_system::Version,
) -> u64 {
    let mut hasher = FxHasher::default();
    definition.content.hash(&mut hasher);
    definition.font.hash(&mut hasher);
    size_bits.hash(&mut hasher);
    line_height_bits.hash(&mut hasher);
    bounds_w_bits.hash(&mut hasher);
    bounds_h_bits.hash(&mut hasher);
    definition.align_x.hash(&mut hasher);
    definition.align_y.hash(&mut hasher);
    definition.wrapping.hash(&mut hasher);
    definition.ellipsis.hash(&mut hasher);
    definition.shaping.hash(&mut hasher);
    hint_factor_bits.hash(&mut hasher);
    version.hash(&mut hasher);
    hasher.finish()
}

struct Seed {
    buffer: cosmic_text::Buffer,
    hinted: bool,
    scale: f32,
}

impl Seed {
    fn new<Content>(definition: &Text<Content>) -> Self {
        let hint = text::hint_factor(definition.size, definition.hint_factor);
        let scale = hint.unwrap_or(1.0);
        let mut buffer = cosmic_text::Buffer::new_empty(
            cosmic_text::Metrics::new(
                definition.size.0 * scale,
                definition.line_height.to_absolute(definition.size).0 * scale,
            ),
        );

        if hint.is_some() {
            buffer.set_hinting(cosmic_text::Hinting::Enabled);
        }

        buffer.set_size(
            Some(definition.bounds.width * scale),
            Some(definition.bounds.height * scale),
        );
        buffer.set_wrap(text::to_wrap(definition.wrapping));
        buffer.set_ellipsize(text::to_ellipsize(
            definition.ellipsis,
            definition.bounds.height * scale,
        ));

        Self {
            buffer,
            hinted: hint.is_some(),
            scale,
        }
    }
}

pub(super) fn plain(definition: Text<&str>) -> Paragraph {
    let version = text::font_version();
    let (_scale, size_bits, line_height_bits, bounds_w_bits, bounds_h_bits, hint_factor_bits) =
        quantize_definition(&definition);
    let hash = compute_hash(
        &definition,
        size_bits,
        line_height_bits,
        bounds_w_bits,
        bounds_h_bits,
        hint_factor_bits,
        version,
    );

    if let Some(cached) = CACHE.with_borrow_mut(|cache| {
        cache.get(
            &definition,
            size_bits,
            line_height_bits,
            bounds_w_bits,
            bounds_h_bits,
            hint_factor_bits,
            version,
            hash,
        )
    }) {
        return cached;
    }

    log::trace!(
        "TAIM shaping plain paragraph: {} bytes",
        definition.content.len()
    );
    let mut seed = Seed::new(&definition);

    seed.buffer.set_text(
        definition.content,
        &text::to_attributes(definition.font),
        text::to_shaping(definition.shaping, definition.content),
        None,
    );

    let mut fonts = text::font_system()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let paragraph = finish(seed, definition.with_content(()), &mut fonts);

    CACHE.with_borrow_mut(|cache| {
        cache.insert(
            &definition,
            size_bits,
            line_height_bits,
            bounds_w_bits,
            bounds_h_bits,
            hint_factor_bits,
            version,
            hash,
            paragraph.clone(),
        );
    });

    paragraph
}

pub(super) fn rich<Link>(definition: Text<&[Span<'_, Link>]>) -> Paragraph {
    log::trace!(
        "TAIM shaping attributed paragraph: {} spans",
        definition.content.len()
    );
    let mut seed = Seed::new(&definition);
    let scale = seed.scale;

    seed.buffer.set_rich_text(
        definition.content.iter().enumerate().map(|(index, span)| {
            let mut attributes = text::to_attributes(span.font.unwrap_or(definition.font));

            if span.size.is_some() || span.line_height.is_some() {
                let size = span.size.unwrap_or(definition.size);
                let line_height = span
                    .line_height
                    .unwrap_or(definition.line_height)
                    .to_absolute(size);
                attributes = attributes.metrics(cosmic_text::Metrics::new(
                    size.0 * scale,
                    line_height.0 * scale,
                ));
            }

            if let Some(color) = span.color {
                attributes = attributes.color(text::to_color(color));
            }

            (span.text.as_ref(), attributes.metadata(index))
        }),
        &text::to_attributes(definition.font),
        cosmic_text::Shaping::Advanced,
        None,
    );

    let mut fonts = text::font_system()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    finish(seed, definition.with_content(()), &mut fonts)
}

fn finish(mut seed: Seed, definition: Text<()>, fonts: &mut text::FontSystem) -> Paragraph {
    seed.buffer.shape_until_scroll(fonts.raw(), false);
    let minimum = text::align(&mut seed.buffer, fonts.raw(), definition.align_x) / seed.scale;

    Paragraph(Arc::new(Internal {
        buffer: seed.buffer,
        font: definition.font,
        shaping: definition.shaping,
        wrapping: definition.wrapping,
        ellipsis: definition.ellipsis,
        align_x: definition.align_x,
        align_y: definition.align_y,
        bounds: definition.bounds,
        min_bounds: minimum,
        version: fonts.version(),
        hint: seed.hinted,
        hint_factor: seed.scale,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::text::{Alignment, Ellipsis, LineHeight, Shaping, Text, Wrapping};
    use crate::core::{Font, Pixels, Size, alignment};

    #[test]
    fn plain_paragraph_cache_reuses_buffer_on_identical_definition() {
        let definition1 = Text {
            content: "Hello TAIM Performance",
            bounds: Size::new(400.0, 100.0),
            size: Pixels(16.0),
            line_height: LineHeight::default(),
            font: Font::DEFAULT,
            align_x: Alignment::Left,
            align_y: alignment::Vertical::Top,
            shaping: Shaping::Basic,
            wrapping: Wrapping::Word,
            ellipsis: Ellipsis::None,
            hint_factor: None,
        };

        let p1 = plain(definition1);
        let p2 = plain(definition1);

        assert!(
            Arc::ptr_eq(&p1.0, &p2.0),
            "Second call with identical definition must hit cache and return the same internal buffer"
        );
    }

    #[test]
    fn plain_paragraph_cache_differentiates_distinct_content_and_bounds() {
        let base = Text {
            content: "Cached String",
            bounds: Size::new(300.0, 50.0),
            size: Pixels(14.0),
            line_height: LineHeight::default(),
            font: Font::DEFAULT,
            align_x: Alignment::Center,
            align_y: alignment::Vertical::Center,
            shaping: Shaping::Basic,
            wrapping: Wrapping::None,
            ellipsis: Ellipsis::End,
            hint_factor: None,
        };

        let diff_content = Text {
            content: "Different String",
            ..base
        };

        let diff_bounds = Text {
            bounds: Size::new(600.0, 50.0),
            ..base
        };

        let p_base = plain(base);
        let p_diff_content = plain(diff_content);
        let p_diff_bounds = plain(diff_bounds);

        assert!(!Arc::ptr_eq(&p_base.0, &p_diff_content.0));
        assert!(!Arc::ptr_eq(&p_base.0, &p_diff_bounds.0));
    }
}

