//! The icon font must declare the family the renderers ask for.
//!
//! It did not. `graphics/fonts/Taim-Icons.ttf` declared `Iced-Icons` --
//! inherited from the fork, renamed on disk but not inside the file -- while
//! `wgpu/src/lib.rs` and `tiny_skia/src/lib.rs` both looked it up as
//! `Font::new("Taim-Icons")`. Font lookup is by declared family name, so it
//! matched nothing: every checkmark, dropdown arrow, and scrollbar arrow in
//! the framework resolved to a missing glyph.
//!
//! Nothing caught it. The font loaded, the text pipeline ran, the widgets
//! laid out at the right size, and 1,705 tests passed -- because not one of
//! them looked at a pixel. This test is the cheap half of that lesson; the
//! expensive half is a golden image suite.

/// Parsed just far enough to answer the two questions that matter.
struct IconFont {
    family: String,
    codepoints: Vec<u32>,
}

fn read_u16(bytes: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([bytes[at], bytes[at + 1]])
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn parse() -> IconFont {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/graphics/fonts/Taim-Icons.ttf");
    let bytes = std::fs::read(path).expect("the icon font must be present");

    let table_count = read_u16(&bytes, 4) as usize;
    let mut tables = std::collections::HashMap::new();

    for index in 0..table_count {
        let record = 12 + index * 16;
        let tag = String::from_utf8_lossy(&bytes[record..record + 4]).into_owned();
        let offset = read_u32(&bytes, record + 8) as usize;
        let _ = tables.insert(tag, offset);
    }

    // ── name: the family record (nameID 1) ──────────────────────────
    let name = tables["name"];
    let count = read_u16(&bytes, name + 2) as usize;
    let storage = name + read_u16(&bytes, name + 4) as usize;
    let mut family = String::new();

    for index in 0..count {
        let record = name + 6 + index * 12;
        let platform = read_u16(&bytes, record);
        let name_id = read_u16(&bytes, record + 6);
        let length = read_u16(&bytes, record + 8) as usize;
        let offset = read_u16(&bytes, record + 10) as usize;

        if name_id != 1 {
            continue;
        }

        let raw = &bytes[storage + offset..storage + offset + length];

        family = if platform == 3 {
            // Windows records are UTF-16BE.
            raw.chunks_exact(2)
                .map(|pair| read_u16(pair, 0))
                .filter_map(|unit| char::from_u32(u32::from(unit)))
                .collect()
        } else {
            raw.iter().map(|byte| char::from(*byte)).collect()
        };

        break;
    }

    // ── cmap: format 4 segment starts and ends ──────────────────────
    let cmap = tables["cmap"];
    let subtables = read_u16(&bytes, cmap + 2) as usize;
    let mut codepoints = Vec::new();

    for index in 0..subtables {
        let record = cmap + 4 + index * 8;
        let subtable = cmap + read_u32(&bytes, record + 4) as usize;

        if read_u16(&bytes, subtable) != 4 {
            continue;
        }

        let segments = read_u16(&bytes, subtable + 6) as usize / 2;
        let starts = subtable + 16 + segments * 2;

        for segment in 0..segments {
            let end = read_u16(&bytes, subtable + 14 + segment * 2);
            let start = read_u16(&bytes, starts + segment * 2);

            if end == 0xFFFF {
                continue;
            }

            codepoints.extend((start..=end).map(u32::from));
        }
    }

    codepoints.sort_unstable();
    codepoints.dedup();

    IconFont { family, codepoints }
}

#[test]
fn the_font_declares_the_family_the_renderers_look_up() {
    let font = parse();

    assert_eq!(
        font.family, "Taim-Icons",
        "the font declares `{}`, but wgpu and tiny_skia both ask for \
         `Taim-Icons`. Lookup is by declared family, so a mismatch means \
         every icon glyph silently resolves to nothing.",
        font.family
    );
}

#[test]
fn the_glyphs_the_renderers_address_by_codepoint_still_exist() {
    let font = parse();

    // These are hard-coded in wgpu/src/lib.rs and tiny_skia/src/lib.rs as
    // `Text::CHECKMARK_ICON` and friends. Regenerating the font must never
    // drop them, whatever else it adds.
    for (codepoint, what) in [
        (0xF00C, "CHECKMARK_ICON"),
        (0xE800, "ARROW_DOWN_ICON"),
        (0xE801, "TAIM_LOGO"),
        (0xE802, "SCROLL_UP_ICON"),
        (0xE803, "SCROLL_DOWN_ICON"),
        (0xE804, "SCROLL_LEFT_ICON"),
        (0xE805, "SCROLL_RIGHT_ICON"),
    ] {
        assert!(
            font.codepoints.contains(&codepoint),
            "U+{codepoint:04X} ({what}) is addressed by the renderers but is \
             not in the font"
        );
    }
}

#[test]
fn every_icon_constant_resolves_to_a_real_glyph() {
    use taim_widget::icon::Icon;

    let font = parse();

    let missing: Vec<char> = Icon::ALL
        .iter()
        .map(|icon| icon.codepoint())
        .filter(|codepoint| !font.codepoints.contains(&(*codepoint as u32)))
        .collect();

    assert!(
        missing.is_empty(),
        "these `Icon` constants have no glyph in the font: {missing:?}. \
         Re-run scripts/assets/build-icon-font.py."
    );

    assert!(
        Icon::ALL.len() >= 30,
        "the icon set has shrunk to {} -- was that deliberate?",
        Icon::ALL.len()
    );
}
