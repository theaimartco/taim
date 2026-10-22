use super::*;

/// A renderer that can shape, measure and draw text.
pub trait Renderer: crate::Renderer {
    /// The renderer's font handle.
    type Font: Copy + PartialEq;

    /// Shaped paragraph type.
    type Paragraph: Paragraph<Font = Self::Font> + 'static;

    /// Editable text buffer type.
    type Editor: Editor<Font = Self::Font> + 'static;

    // The built-in icon font and the glyphs widgets draw from it.

    /// The font holding the glyphs below.
    const ICON_FONT: Self::Font;

    /// ✔, for checkboxes.
    const CHECKMARK_ICON: char;

    /// ▼, for pick lists and combo boxes.
    const ARROW_DOWN_ICON: char;

    /// Scroll-up arrow.
    const SCROLL_UP_ICON: char;

    /// Scroll-down arrow.
    const SCROLL_DOWN_ICON: char;

    /// Scroll-left arrow.
    const SCROLL_LEFT_ICON: char;

    /// Scroll-right arrow.
    const SCROLL_RIGHT_ICON: char;

    /// The TAIM logo mark.
    const TAIM_LOGO: char;

    /// Font used where text does not choose one.
    fn default_font(&self) -> Self::Font;

    /// Size used where text does not choose one.
    fn default_size(&self) -> Pixels;

    /// Draws `paragraph` with its top-left corner at `position`, in `color`
    /// where spans do not set their own, clipped to `clip_bounds`.
    fn fill_paragraph(
        &mut self,
        paragraph: &Self::Paragraph,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    );

    /// Draws `editor` like [`fill_paragraph`](Self::fill_paragraph).
    fn fill_editor(
        &mut self,
        editor: &Self::Editor,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    );

    /// Shapes and draws `text` in one go, for text not worth caching.
    fn fill_text(
        &mut self,
        text: Text<String, Self::Font>,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    );
}
