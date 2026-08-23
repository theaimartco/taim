//! A person or entity's likeness, drawn as a circle.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::avatar;
//!
//! # enum Message {}
//! fn view() -> Element<'static, Message> {
//!     avatar("Ada Lovelace").size(40).into()
//! }
//! ```
//!
//! # The name is the widget
//!
//! An avatar is constructed from a name, not from an image, because the
//! name is the part that always exists and always has to be announced.
//! An image is an optional embellishment on top of it. Building it the
//! other way round makes the accessible name something a caller can
//! forget, and an unlabelled avatar is a blank circle to a screen-reader
//! user.
//!
//! # Colour is derived, not chosen
//!
//! An avatar with no image gets its fill from a hash of its name, so the
//! same person is the same colour everywhere in an application without
//! anyone maintaining a mapping. The hash is deliberately simple and
//! stable: it must not change between releases, or every avatar in every
//! screenshot changes with it.

use crate::core::alignment;
use crate::core::border::Border;
use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::text;
use crate::core::widget::tree::{self, Tree};
use crate::core::{Background, Color, Element, Layout, Length, Rectangle, Size, Theme, Widget};

/// The default diameter.
const DEFAULT_SIZE: f32 = 40.0;

/// How many initials are taken from a name.
const MAX_INITIALS: usize = 2;

/// A person or entity's likeness, drawn as a circle.
#[allow(missing_debug_implementations)]
pub struct Avatar<'a, Theme = crate::Theme>
where
    Theme: Catalog,
{
    name: String,
    size: f32,
    class: Theme::Class<'a>,
}

impl<'a, Theme> Avatar<'a, Theme>
where
    Theme: Catalog,
{
    /// Creates an avatar for `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            size: DEFAULT_SIZE,
            class: Theme::default(),
        }
    }

    /// Sets the diameter.
    pub fn size(mut self, size: impl Into<crate::core::Pixels>) -> Self {
        self.size = size.into().0.max(0.0);
        self
    }

    /// Sets the style of the avatar.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, u64) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// The initials drawn when there is no image.
    ///
    /// Takes the first character of up to the first two whitespace-
    /// separated words, which is the convention people already read
    /// without being taught it.
    pub fn initials(&self) -> String {
        self.name
            .split_whitespace()
            .filter_map(|word| word.chars().next())
            .take(MAX_INITIALS)
            .flat_map(char::to_uppercase)
            .collect()
    }

    /// A stable hash of the name, used to pick a fill.
    ///
    /// This is FNV-1a over the name's bytes. It is written out rather
    /// than taken from `DefaultHasher` on purpose: `DefaultHasher` makes
    /// no stability guarantee across Rust releases, and an avatar that
    /// silently changes colour when the toolchain moves is a bug nobody
    /// will think to look for.
    pub fn hash(&self) -> u64 {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;

        self.name.bytes().fold(OFFSET, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(PRIME)
        })
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Avatar<'_, Theme>
where
    Renderer: crate::core::text::Renderer,
    Theme: Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.resolve(
            Length::Fixed(self.size),
            Length::Fixed(self.size),
            Size::new(self.size, self.size),
        ))
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        Some(
            Node::new(tree.id().clone(), layout.bounds(), Role::Image)
                .with_name(self.name.as_str())
                .with_state(SemanticState::empty())
                .with_actions(Actions::empty()),
        )
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let appearance = theme.style(&self.class, self.hash());
        let bounds = layout.bounds();

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    radius: (bounds.height / 2.0).into(),
                    ..appearance.border
                },
                ..renderer::Quad::default()
            },
            appearance.background,
        );

        // The initials. This used to be `let _ = self.initials();`, on the
        // reasoning that "an avatar has to work in the software fallback and
        // in headless tests" -- so the widget rendered a coloured disc and
        // nothing else, for every caller, forever. The premise was wrong:
        // `tiny_skia` draws text exactly as the GPU renderer does, which the
        // golden suite now exercises on every run without a GPU present.
        renderer.fill_text(
            text::Text {
                content: self.initials(),
                font: renderer.default_font(),
                // Proportional to the circle, so an avatar reads the same at
                // any size a caller picks.
                size: crate::core::Pixels(bounds.height * 0.4),
                line_height: text::LineHeight::default(),
                bounds: bounds.size(),
                align_x: text::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text::Shaping::Advanced,
                wrapping: text::Wrapping::None,
                ellipsis: text::Ellipsis::default(),
                hint_factor: None,
            },
            bounds.center(),
            appearance.text_color,
            *viewport,
        );
    }
}

impl<'a, Message, Theme, Renderer> From<Avatar<'a, Theme>> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::text::Renderer,
{
    fn from(avatar: Avatar<'a, Theme>) -> Self {
        Self::new(avatar)
    }
}

/// The appearance of an [`Avatar`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The circle's fill.
    pub background: Background,
    /// The colour of the initials drawn inside it.
    pub text_color: Color,
    /// The border around the circle.
    pub border: Border,
}

/// The theme catalog of an [`Avatar`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class, given a stable hash of the avatar's
    /// name.
    fn style(&self, class: &Self::Class<'_>, hash: u64) -> Style;
}

/// A styling function for an [`Avatar`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, u64) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>, hash: u64) -> Style {
        class(self, hash)
    }
}

/// The default styling of an [`Avatar`] for aura's own theme.
///
/// Picks one of the palette's own swatches from the name hash, so every
/// avatar stays inside the theme rather than introducing colours the
/// rest of the interface never uses.
pub fn default(theme: &Theme, hash: u64) -> Style {
    let palette = theme.palette();

    let pair = match hash % 4 {
        0 => palette.primary.base,
        1 => palette.secondary.base,
        2 => palette.success.base,
        _ => palette.warning.base,
    };

    Style {
        background: Background::Color(pair.color),
        text_color: pair.text,
        border: Border::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn avatar<'a>(name: &str) -> Avatar<'a> {
        Avatar::new(name)
    }

    #[test]
    fn initials_come_from_the_first_two_words() {
        assert_eq!(avatar("Ada Lovelace").initials(), "AL");
        assert_eq!(avatar("Grace Brewster Hopper").initials(), "GB");
    }

    #[test]
    fn a_single_word_yields_one_initial() {
        assert_eq!(avatar("Ada").initials(), "A");
    }

    #[test]
    fn initials_are_uppercased() {
        assert_eq!(avatar("ada lovelace").initials(), "AL");
    }

    #[test]
    fn extra_whitespace_does_not_produce_empty_initials() {
        assert_eq!(avatar("  Ada   Lovelace  ").initials(), "AL");
    }

    #[test]
    fn an_empty_name_yields_no_initials_rather_than_panicking() {
        assert_eq!(avatar("").initials(), "");
        assert_eq!(avatar("   ").initials(), "");
    }

    #[test]
    fn non_ascii_names_keep_their_own_initials() {
        assert_eq!(avatar("Ámbar Ñuñez").initials(), "ÁÑ");
    }

    #[test]
    fn the_same_name_always_hashes_the_same_way() {
        assert_eq!(avatar("Ada Lovelace").hash(), avatar("Ada Lovelace").hash());
    }

    #[test]
    fn different_names_generally_hash_differently() {
        assert_ne!(avatar("Ada Lovelace").hash(), avatar("Grace Hopper").hash());
    }

    #[test]
    fn the_hash_stays_fnv_1a_so_avatars_do_not_change_colour_between_releases() {
        // Checked against an independent FNV-1a written out here rather
        // than against a magic literal: this fails if someone swaps the
        // implementation for `DefaultHasher`, which makes no stability
        // guarantee across Rust releases and would silently recolour
        // every avatar in every screenshot.
        fn reference_fnv_1a(text: &str) -> u64 {
            let mut hash = 0xcbf2_9ce4_8422_2325_u64;

            for byte in text.bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }

            hash
        }

        for name in ["Ada Lovelace", "Grace Hopper", "", "Ámbar"] {
            assert_eq!(
                avatar(name).hash(),
                reference_fnv_1a(name),
                "hash for {name:?} diverged from FNV-1a"
            );
        }
    }

    #[test]
    fn the_derived_swatch_is_stable_for_a_given_name() {
        // What the hash is actually for: the same person must land on
        // the same colour every time the avatar is built.
        // A named variant rather than `Theme::default()`: `Catalog` is
        // in scope and also has a `default`, so the bare call resolves
        // to the class instead of the theme.
        let theme = Theme::Light;

        let first = default(&theme, avatar("Ada Lovelace").hash());
        let second = default(&theme, avatar("Ada Lovelace").hash());

        assert_eq!(first, second);
    }
}
