#!/usr/bin/env python3
"""Builds Taim's icon font, SVG sources, manifest, and typed Rust module.

Everything downstream of `icon_geometry.py` is generated, so an icon is
changed in one place. Run after editing the geometry:

    scripts/assets/build-icon-font.py

Requires `fonttools`. The font extends the existing one rather than replacing
it: the seven legacy glyphs the renderers reference by codepoint
(`Text::CHECKMARK_ICON` and friends) are preserved byte for byte.
"""

import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from fontTools.pens.ttGlyphPen import TTGlyphPen  # noqa: E402
from fontTools.ttLib import TTFont  # noqa: E402

from icon_geometry import ICONS  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
FONT = os.path.join(ROOT, "graphics", "fonts", "Taim-Icons.ttf")
SVG_DIR = os.path.join(ROOT, "assets", "icons")
MANIFEST = os.path.join(SVG_DIR, "manifest.toml")
RUST = os.path.join(ROOT, "widget", "src", "icon.rs")

GRID = 24.0
UPEM = 1000.0
SCALE = UPEM / GRID
BASELINE = 875.0  # puts the 24x24 box from y=875 down to y=-125
FIRST_CODEPOINT = 0xE900

# Icons whose meaning mirrors in a right-to-left layout.
MIRROR_RTL = {
    "chevron_left", "chevron_right", "arrow_left", "arrow_right",
    "edit", "upload", "download", "folder", "search",
}

# Codepoints the renderers hard-code; see wgpu/src/lib.rs `ICON_FONT`.
LEGACY = {
    0xE800: "down-dir", 0xE801: "iced", 0xE802: "uniE802", 0xE803: "uniE803",
    0xE804: "uniE804", 0xE805: "uniE805", 0xF00C: "ok",
}


def to_font(x, y):
    return (round(x * SCALE), round(BASELINE - y * SCALE))


def draw(contours, pen):
    for contour in contours:
        if not contour:
            continue
        last = contour[-1]
        start = (last[1], last[2]) if last[0] == "L" else (last[3], last[4])
        pen.moveTo(to_font(*start))
        for seg in contour:
            if seg[0] == "L":
                pen.lineTo(to_font(seg[1], seg[2]))
            else:
                pen.qCurveTo(to_font(seg[1], seg[2]), to_font(seg[3], seg[4]))
        pen.closePath()


def svg_path(contours):
    out = []
    for contour in contours:
        if not contour:
            continue
        last = contour[-1]
        start = (last[1], last[2]) if last[0] == "L" else (last[3], last[4])
        out.append(f"M{start[0]:.2f} {start[1]:.2f}")
        for seg in contour:
            if seg[0] == "L":
                out.append(f"L{seg[1]:.2f} {seg[2]:.2f}")
            else:
                out.append(f"Q{seg[1]:.2f} {seg[2]:.2f} {seg[3]:.2f} {seg[4]:.2f}")
        out.append("Z")
    return " ".join(out)


def rename(font):
    """The family this font declares must match what the renderers ask for.

    It declared `Iced-Icons` while `wgpu/src/lib.rs` looked it up as
    `Font::new("Taim-Icons")`, so every checkmark, dropdown arrow, and
    scrollbar arrow resolved to a missing glyph.
    """
    for record in font["name"].names:
        value = str(record)
        if "Iced" in value:
            record.string = value.replace("Iced-Icons", "Taim-Icons").replace(
                "Iced Icons", "Taim Icons")


def main():
    font = TTFont(FONT)
    rename(font)

    glyf, hmtx = font["glyf"], font["hmtx"]

    # Idempotence: drop anything a previous run of this script added, so
    # re-running rebuilds rather than appending a second copy of every glyph.
    generated = [name for name in font.getGlyphOrder() if name.startswith("taim_")]

    for name in generated:
        del glyf.glyphs[name]
        del hmtx.metrics[name]

    # Delete by glyph name, not by codepoint range: the legacy checkmark
    # lives at U+F00C, which is numerically above FIRST_CODEPOINT, so a
    # range-based sweep silently ate it on the second run.
    for table in font["cmap"].tables:
        for codepoint in [
            c for c, name in table.cmap.items() if name.startswith("taim_")
        ]:
            del table.cmap[codepoint]

    order = [name for name in font.getGlyphOrder() if not name.startswith("taim_")]

    if generated:
        print(f"rebuild:  dropped {len(generated)} glyphs from a previous run")

    assigned = {}

    for index, (name, contours) in enumerate(sorted(ICONS.items())):
        codepoint = FIRST_CODEPOINT + index
        glyph_name = f"taim_{name}"
        pen = TTGlyphPen(None)
        draw(contours, pen)
        glyf[glyph_name] = pen.glyph()
        hmtx[glyph_name] = (int(UPEM), 0)
        order.append(glyph_name)
        assigned[name] = (codepoint, glyph_name)

    font.setGlyphOrder(order)
    glyf.glyphOrder = order
    font["maxp"].numGlyphs = len(order)

    # Only the Unicode subtables: this font also carries a legacy format-0
    # Macintosh subtable, which can address 256 codepoints and nothing above.
    unicode_subtables = [
        table for table in font["cmap"].tables if table.format in (4, 12)
    ]
    assert unicode_subtables, "no Unicode cmap subtable to extend"

    for table in unicode_subtables:
        for name, (codepoint, glyph_name) in assigned.items():
            table.cmap[codepoint] = glyph_name

    font.save(FONT)
    print(f"font:     {os.path.relpath(FONT, ROOT)} "
          f"({len(assigned)} new glyphs, {len(LEGACY)} legacy preserved)")

    os.makedirs(SVG_DIR, exist_ok=True)
    for name, contours in sorted(ICONS.items()):
        path = os.path.join(SVG_DIR, f"{name}.svg")
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(
                '<svg xmlns="http://www.w3.org/2000/svg" '
                f'viewBox="0 0 {int(GRID)} {int(GRID)}" '
                f'width="{int(GRID)}" height="{int(GRID)}">\n'
                f'  <path fill="currentColor" fill-rule="nonzero" '
                f'd="{svg_path(contours)}"/>\n</svg>\n')
    print(f"svg:      {os.path.relpath(SVG_DIR, ROOT)}/*.svg ({len(ICONS)} files)")

    with open(MANIFEST, "w", encoding="utf-8") as handle:
        handle.write(
            "# Taim icon manifest -- GENERATED by scripts/assets/build-icon-font.py.\n"
            "# Edit scripts/assets/icon_geometry.py and re-run that script.\n"
            "#\n"
            "# Every icon here was authored for this repository and is covered by\n"
            "# the project's MIT licence. No third-party icon set is vendored, so\n"
            "# there is no separate attribution requirement.\n"
            "#\n"
            "# `codepoint` is the Private Use Area slot in graphics/fonts/Taim-Icons.ttf.\n"
            "# `mirror_rtl` marks icons whose meaning flips in a right-to-left layout.\n")
        for name, (codepoint, _) in sorted(assigned.items()):
            handle.write(
                f'\n[[icon]]\nname = "{name}"\ngrid = 24\n'
                f'codepoint = "U+{codepoint:04X}"\n'
                f'mirror_rtl = {"true" if name in MIRROR_RTL else "false"}\n'
                f'license = "MIT (this repository)"\n')
    print(f"manifest: {os.path.relpath(MANIFEST, ROOT)}")

    with open(RUST, "w", encoding="utf-8") as handle:
        handle.write(RUST_HEADER)
        for name, (codepoint, _) in sorted(assigned.items()):
            const = name.upper()
            mirror = "mirrors in RTL" if name in MIRROR_RTL else "does not mirror"
            handle.write(
                f"    /// The `{name}` icon ({mirror}).\n"
                f"    pub const {const}: Icon = Icon::new('\\u{{{codepoint:x}}}');\n\n")
        handle.write(RUST_FOOTER.replace(
            "__ALL__",
            ",\n        ".join(f"Self::{n.upper()}" for n in sorted(assigned))))
    # Generated code still has to satisfy `cargo fmt --check`, or every
    # regeneration leaves the tree dirty and CI red.
    formatted = subprocess.run(
        ["rustfmt", "--edition", "2024", RUST],
        capture_output=True,
        check=False,
    )

    if formatted.returncode != 0:
        print(f"warning: rustfmt failed: {formatted.stderr.decode().strip()}")

    print(f"rust:     {os.path.relpath(RUST, ROOT)} ({len(assigned)} constants)")


RUST_HEADER = '''//! The bundled icon set.
//!
//! GENERATED by `scripts/assets/build-icon-font.py` -- do not edit by hand.
//! Change `scripts/assets/icon_geometry.py` and re-run that script.
//!
//! Icons are glyphs in `graphics/fonts/Taim-Icons.ttf`, which the text stack
//! loads unconditionally. An icon therefore costs one `text` widget and no new
//! dependency: no SVG rasteriser, no image decode, no second font.
//!
//! ```ignore
//! use taim::widget::icon::{self, Icon};
//! use taim::widget::row;
//!
//! row![icon::icon(Icon::SEARCH), icon::sized(Icon::SETTINGS, 32)]
//! ```
//!
//! Marked `ignore` for the same reason `widget/src/taim.rs` is: this crate
//! cannot depend on the `taim` facade that re-exports it, so an example
//! written the way a user would write it has nothing to compile against.

use crate::core::{Font, Length, Pixels};
use crate::text::{self, Text};

/// The font every [`Icon`] is drawn from.
///
/// The family name here must match what `graphics/fonts/Taim-Icons.ttf`
/// declares. It did not for some time -- the font said `Iced-Icons` while the
/// renderers asked for `Taim-Icons`, so every glyph resolved to nothing.
pub const FONT: Font = Font::new("Taim-Icons");

/// One glyph in the icon font.
///
/// A plain `char` would work and would also accept any char -- including one
/// the font has no glyph for, which renders as a blank space no compiler can
/// warn about. This type only ever holds a codepoint the build script assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Icon(char);

impl Icon {
    const fn new(codepoint: char) -> Self {
        Self(codepoint)
    }

    /// The codepoint this icon occupies in [`FONT`].
    pub const fn codepoint(self) -> char {
        self.0
    }

'''

RUST_FOOTER = '''    /// Every icon in the set, in name order.
    ///
    /// Exists so a gallery can show the whole set without a hand-written list
    /// that silently falls behind the font.
    pub const ALL: &'static [Self] = &[
        __ALL__,
    ];
}

impl From<Icon> for char {
    fn from(icon: Icon) -> Self {
        icon.0
    }
}

/// Draws an [`Icon`] at the renderer's default text size.
pub fn icon<'a, Theme, Renderer>(icon: Icon) -> Text<'a, Theme, Renderer>
where
    Theme: text::Catalog + 'a,
    Renderer: crate::core::text::Renderer<Font = Font>,
{
    Text::new(icon.codepoint().to_string()).font(FONT).center()
}

/// Draws an [`Icon`] in a square box of the given edge length.
///
/// The box is what makes a row of icons line up: glyph advances differ, so
/// icons laid out by their own width would not share a baseline grid.
pub fn sized<'a, Theme, Renderer>(
    icon_: Icon,
    size: impl Into<Pixels>,
) -> Text<'a, Theme, Renderer>
where
    Theme: text::Catalog + 'a,
    Renderer: crate::core::text::Renderer<Font = Font>,
{
    let size = size.into();

    icon(icon_)
        .size(size)
        .width(Length::Fixed(size.0))
        .height(Length::Fixed(size.0))
}
'''

if __name__ == "__main__":
    main()
