use super::*;

/// A renderer capable of measuring and drawing [`Text`].
pub trait Renderer: crate::Renderer {
    /// The font type used.
    type Font: Copy + PartialEq;

    /// The [`Paragraph`] of this [`Renderer`].
    type Paragraph: Paragraph<Font = Self::Font> + 'static;

    /// The [`Editor`] of this [`Renderer`].
    type Editor: Editor<Font = Self::Font> + 'static;

    /// The icon font of the backend.
    const ICON_FONT: Self::Font;

    /// The `char` representing a ✔ icon in the [`ICON_FONT`].
    ///
    /// [`ICON_FONT`]: Self::ICON_FONT
    const CHECKMARK_ICON: char;

    /// The `char` representing a ▼ icon in the built-in [`ICON_FONT`].
    ///
    /// [`ICON_FONT`]: Self::ICON_FONT
    const ARROW_DOWN_ICON: char;

    /// The `char` representing a ^ icon in the built-in [`ICON_FONT`].
    ///
    /// [`ICON_FONT`]: Self::ICON_FONT
    const SCROLL_UP_ICON: char;

    /// The `char` representing a v icon in the built-in [`ICON_FONT`].
    ///
    /// [`ICON_FONT`]: Self::ICON_FONT
    const SCROLL_DOWN_ICON: char;

    /// The `char` representing a < icon in the built-in [`ICON_FONT`].
    ///
    /// [`ICON_FONT`]: Self::ICON_FONT
    const SCROLL_LEFT_ICON: char;

    /// The `char` representing a > icon in the built-in [`ICON_FONT`].
    ///
    /// [`ICON_FONT`]: Self::ICON_FONT
    const SCROLL_RIGHT_ICON: char;

    /// The 'char' representing the taim logo in the built-in ['ICON_FONT'].
    ///
    /// ['ICON_FONT']: Self::ICON_FONT
    const TAIM_LOGO: char;

    /// Returns the default [`Self::Font`].
    fn default_font(&self) -> Self::Font;

    /// Returns the default size of [`Text`].
    fn default_size(&self) -> Pixels;

    /// Draws the given [`Paragraph`] at the given position and with the given
    /// [`Color`].
    fn fill_paragraph(
        &mut self,
        text: &Self::Paragraph,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    );

    /// Draws the given [`Editor`] at the given position and with the given
    /// [`Color`].
    fn fill_editor(
        &mut self,
        editor: &Self::Editor,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    );

    /// Draws the given [`Text`] at the given position and with the given
    /// [`Color`].
    fn fill_text(
        &mut self,
        text: Text<String, Self::Font>,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    );
}
