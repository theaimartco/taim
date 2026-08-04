//! Colour palettes for highlighted tokens.
use aura_core::Color;

/// The kind of token a [`Theme`] assigns a colour to.
///
/// The set is deliberately small. A line-oriented tokenizer can classify
/// these reliably without a parse tree; anything finer would be a colour
/// the highlighter cannot actually justify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// A line or block comment.
    Comment,
    /// A string or character literal.
    String,
    /// A numeric literal.
    Number,
    /// A language keyword.
    Keyword,
    /// A built-in or well-known type name.
    Type,
    /// An identifier in call position.
    Function,
    /// An attribute, annotation, preprocessor directive, or shell variable.
    Attribute,
    /// An operator or delimiter.
    Punctuation,
    /// Anything else.
    Text,
}

impl Scope {
    /// Every [`Scope`] a [`Theme`] must colour.
    pub const ALL: &'static [Scope] = &[
        Scope::Comment,
        Scope::String,
        Scope::Number,
        Scope::Keyword,
        Scope::Type,
        Scope::Function,
        Scope::Attribute,
        Scope::Punctuation,
        Scope::Text,
    ];
}

/// A colour palette for highlighted source code.
///
/// The palettes mirror the three colour schemes of `aura-design` rather
/// than importing a third-party theme collection, so a code block in a
/// document matches the surface it is rendered on. Every colour is held
/// to a contrast floor against its own background by the tests in this
/// module -- see [`Theme::color`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Theme {
    /// The palette for Aura's dark colour scheme.
    #[default]
    Dark,
    /// The palette for Aura's light colour scheme.
    Light,
    /// The palette for Aura's high-contrast colour scheme.
    HighContrast,
}

impl Theme {
    /// Every [`Theme`] this crate provides.
    pub const ALL: &'static [Theme] = &[Theme::Dark, Theme::Light, Theme::HighContrast];

    /// Returns the [`Theme`] that matches a surface of the given darkness.
    pub fn for_surface(is_dark: bool, is_high_contrast: bool) -> Self {
        match (is_dark, is_high_contrast) {
            (_, true) => Theme::HighContrast,
            (true, false) => Theme::Dark,
            (false, false) => Theme::Light,
        }
    }

    /// Returns `true` if this [`Theme`] is meant for a dark background.
    pub fn is_dark(self) -> bool {
        !matches!(self, Theme::Light)
    }

    /// Returns the background [`Color`] a code block should use.
    pub fn background(self) -> Color {
        match self {
            Theme::Dark => Color::from_rgb8(0x1b, 0x1f, 0x27),
            Theme::Light => Color::from_rgb8(0xf6, 0xf8, 0xfa),
            Theme::HighContrast => Color::from_rgb8(0x00, 0x00, 0x00),
        }
    }

    /// Returns the [`Color`] this [`Theme`] assigns to the given [`Scope`].
    ///
    /// Every returned colour clears WCAG 2.1 AA (4.5:1) against
    /// [`Theme::background`], except [`Scope::Comment`], which is
    /// intentionally de-emphasised and held to the 3:1 large-text floor.
    /// [`Theme::HighContrast`] clears AAA (7:1) for every scope.
    pub fn color(self, scope: Scope) -> Color {
        let palette = self.palette();

        match scope {
            Scope::Comment => palette.comment,
            Scope::String => palette.string,
            Scope::Number => palette.number,
            Scope::Keyword => palette.keyword,
            Scope::Type => palette.type_,
            Scope::Function => palette.function,
            Scope::Attribute => palette.attribute,
            Scope::Punctuation => palette.punctuation,
            Scope::Text => palette.text,
        }
    }

    const fn palette(self) -> Palette {
        match self {
            Theme::Dark => Palette {
                text: Color::from_rgb8(0xd6, 0xdb, 0xe4),
                comment: Color::from_rgb8(0x8a, 0x94, 0xa6),
                string: Color::from_rgb8(0x9e, 0xd0, 0x8a),
                number: Color::from_rgb8(0xf0, 0xa1, 0x74),
                keyword: Color::from_rgb8(0xd8, 0xa2, 0xe8),
                type_: Color::from_rgb8(0xf2, 0xd0, 0x8a),
                function: Color::from_rgb8(0x5a, 0xa8, 0xff),
                attribute: Color::from_rgb8(0x7f, 0xd8, 0xc8),
                punctuation: Color::from_rgb8(0xb6, 0xbe, 0xcd),
            },
            Theme::Light => Palette {
                text: Color::from_rgb8(0x1f, 0x23, 0x28),
                comment: Color::from_rgb8(0x63, 0x6c, 0x76),
                string: Color::from_rgb8(0x0a, 0x53, 0x2a),
                number: Color::from_rgb8(0x8a, 0x3d, 0x00),
                keyword: Color::from_rgb8(0xa4, 0x14, 0x40),
                type_: Color::from_rgb8(0x0b, 0x4f, 0x9e),
                function: Color::from_rgb8(0x5b, 0x32, 0xa8),
                attribute: Color::from_rgb8(0x0d, 0x5b, 0x63),
                punctuation: Color::from_rgb8(0x3a, 0x41, 0x49),
            },
            Theme::HighContrast => Palette {
                text: Color::from_rgb8(0xff, 0xff, 0xff),
                comment: Color::from_rgb8(0xb8, 0xc4, 0xd4),
                string: Color::from_rgb8(0x8c, 0xf5, 0x9e),
                number: Color::from_rgb8(0xff, 0xc0, 0x7a),
                keyword: Color::from_rgb8(0xf2, 0xa8, 0xff),
                type_: Color::from_rgb8(0xff, 0xe6, 0x6b),
                function: Color::from_rgb8(0x8a, 0xd8, 0xff),
                attribute: Color::from_rgb8(0x7c, 0xf2, 0xe0),
                punctuation: Color::from_rgb8(0xe4, 0xea, 0xf2),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Palette {
    text: Color,
    comment: Color,
    string: Color,
    number: Color,
    keyword: Color,
    type_: Color,
    function: Color,
    attribute: Color,
    punctuation: Color,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relative_luminance(color: Color) -> f32 {
        fn channel(value: f32) -> f32 {
            if value <= 0.040_45 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }

        0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
    }

    fn contrast_ratio(a: Color, b: Color) -> f32 {
        let (a, b) = (relative_luminance(a), relative_luminance(b));
        let (lighter, darker) = if a > b { (a, b) } else { (b, a) };

        (lighter + 0.05) / (darker + 0.05)
    }

    #[test]
    fn every_scope_is_readable_on_its_own_background() {
        for theme in Theme::ALL {
            let background = theme.background();

            for scope in Scope::ALL {
                let ratio = contrast_ratio(theme.color(*scope), background);

                let floor = match (theme, scope) {
                    (Theme::HighContrast, _) => 7.0,
                    (_, Scope::Comment) => 3.0,
                    _ => 4.5,
                };

                assert!(
                    ratio >= floor,
                    "{theme:?}/{scope:?} contrast {ratio:.2} is below {floor:.1}"
                );
            }
        }
    }

    /// Converts an sRGB [`Color`] to CIE L\*a\*b\* under a D65 white point.
    fn to_lab(color: Color) -> [f32; 3] {
        fn linear(value: f32) -> f32 {
            if value <= 0.040_45 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }

        fn f(t: f32) -> f32 {
            if t > 216.0 / 24389.0 {
                t.cbrt()
            } else {
                (841.0 / 108.0) * t + 4.0 / 29.0
            }
        }

        let (r, g, b) = (linear(color.r), linear(color.g), linear(color.b));

        let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.950_47;
        let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.088_83;

        let (fx, fy, fz) = (f(x), f(y), f(z));

        [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
    }

    fn delta_e(a: Color, b: Color) -> f32 {
        let (a, b) = (to_lab(a), to_lab(b));

        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }

    #[test]
    fn scopes_are_visually_distinct_within_a_theme() {
        // Distinctness is a *perceptual* property, not a lightness one:
        // two scopes can differ only in hue and still be trivially
        // distinguishable, which is exactly how syntax palettes work. So
        // this is CIELAB delta-E, not the WCAG contrast ratio used above.
        // A delta-E of 14 is comfortably past the "clearly a different
        // colour" threshold at text sizes.
        for theme in Theme::ALL {
            for (index, scope) in Scope::ALL.iter().enumerate() {
                for other in &Scope::ALL[index + 1..] {
                    if matches!(
                        (scope, other),
                        (Scope::Text, Scope::Punctuation) | (Scope::Punctuation, Scope::Text)
                    ) {
                        // Punctuation deliberately sits next to body text.
                        continue;
                    }

                    let difference = delta_e(theme.color(*scope), theme.color(*other));

                    assert!(
                        difference >= 14.0,
                        "{theme:?}: {scope:?} and {other:?} are only {difference:.1} apart"
                    );
                }
            }
        }
    }

    #[test]
    fn dark_themes_have_dark_backgrounds() {
        for theme in Theme::ALL {
            assert_eq!(
                theme.is_dark(),
                relative_luminance(theme.background()) < 0.5,
                "{theme:?} disagrees with its own background"
            );
        }
    }

    #[test]
    fn for_surface_prefers_high_contrast() {
        assert_eq!(Theme::for_surface(true, false), Theme::Dark);
        assert_eq!(Theme::for_surface(false, false), Theme::Light);
        assert_eq!(Theme::for_surface(true, true), Theme::HighContrast);
        assert_eq!(Theme::for_surface(false, true), Theme::HighContrast);
    }
}
