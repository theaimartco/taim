//! Small pieces shared by the built-in views.
use crate::cx::PaintCx;
use crate::{Border, Color, Event, Rectangle, keyboard, mouse, semantics};

/// Width of the keyboard-focus outline.
pub(crate) const FOCUS_RING: f32 = 2.0;

/// Draws the keyboard-focus outline around `bounds`.
pub(crate) fn focus_ring(cx: &mut PaintCx<'_>, bounds: Rectangle, radius: f32) {
    let color = cx.theme().palette().primary.strong.color;
    let ring = bounds.expand(FOCUS_RING + 1.0);

    cx.fill(
        ring,
        Color::TRANSPARENT,
        Border::default()
            .width(FOCUS_RING)
            .color(color)
            .rounded(radius + FOCUS_RING + 1.0),
    );
}

/// Whether `event` is a key that activates the focused control
/// (Space or Enter).
pub(crate) fn is_activation(event: &Event) -> bool {
    use keyboard::key::Named;

    matches!(
        event,
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::Space | Named::Enter),
            ..
        })
    )
}

/// Whether `event` is a primary click starting over the view.
pub(crate) fn is_press(event: &Event) -> bool {
    matches!(
        event,
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
    )
}

/// Whether `event` ends a primary click.
pub(crate) fn is_release(event: &Event) -> bool {
    matches!(
        event,
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
    )
}

/// The common semantic state flags of an interactive view.
pub(crate) fn interactive_state(enabled: bool, focused: bool) -> semantics::State {
    let mut state = semantics::State::FOCUSABLE;
    state.set(semantics::State::DISABLED, !enabled);
    state.set(semantics::State::FOCUSED, focused);
    state
}

/// A shaped, single-style text label, for views that draw their own text.
pub(crate) struct Label {
    content: String,
    size: Option<crate::Pixels>,
    font: Option<taim_core::Font>,
    shaped: Option<<crate::Renderer as taim_core::text::Renderer>::Paragraph>,
}

impl Label {
    pub(crate) fn new(content: impl Into<String>, size: Option<f32>) -> Self {
        Self {
            content: content.into(),
            size: size.map(crate::Pixels),
            font: None,
            shaped: None,
        }
    }

    /// A label showing one icon glyph.
    pub(crate) fn glyph(icon: super::Icon, size: f32) -> Self {
        Self {
            font: Some(super::Icon::FONT),
            ..Self::new(icon.codepoint(), Some(size))
        }
    }

    /// The shaped size (zero before the first measure).
    pub(crate) fn size(&self) -> crate::Size {
        self.shaped
            .as_ref()
            .map_or(crate::Size::ZERO, taim_core::text::Paragraph::min_bounds)
    }

    pub(crate) fn text(&self) -> String {
        self.content.clone()
    }

    pub(crate) fn set(&mut self, content: &str) {
        if self.content != content {
            content.clone_into(&mut self.content);
            self.shaped = None;
        }
    }

    /// Shapes the label (once) and returns its size.
    // With no renderer compiled in, the paragraph and editor types are `()`.
    #[allow(clippy::semicolon_if_nothing_returned)]
    pub(crate) fn measure(&mut self, cx: &crate::MeasureCx<'_>) -> crate::Size {
        use taim_core::Renderer as _;
        use taim_core::text::{self, Paragraph as _, Renderer as _};

        let renderer = cx.renderer();
        let shaped = self.shaped.get_or_insert_with(|| {
            text::Paragraph::with_text(text::Text {
                content: self.content.as_str(),
                bounds: crate::Size::INFINITE,
                size: self.size.unwrap_or_else(|| renderer.default_size()),
                line_height: text::LineHeight::default(),
                font: self.font.unwrap_or_else(|| renderer.default_font()),
                align_x: text::Alignment::Left,
                align_y: taim_core::alignment::Vertical::Top,
                shaping: text::Shaping::default(),
                wrapping: text::Wrapping::None,
                ellipsis: text::Ellipsis::None,
                hint_factor: renderer.scale_factor(),
            })
        });
        shaped.min_bounds()
    }

    /// Draws the label with its top-left corner at `at`.
    pub(crate) fn paint(&self, cx: &mut PaintCx<'_>, at: crate::Point, color: Color) {
        use taim_core::text::Renderer as _;

        if let Some(shaped) = &self.shaped {
            let clip = Rectangle::new(at, taim_core::text::Paragraph::min_bounds(shaped));
            cx.renderer().fill_paragraph(shaped, at, color, clip);
        }
    }

    /// Draws the label centered in `area`.
    pub(crate) fn paint_centered(&self, cx: &mut PaintCx<'_>, area: Rectangle, color: Color) {
        if let Some(shaped) = &self.shaped {
            let size = taim_core::text::Paragraph::min_bounds(shaped);
            let at = crate::Point::new(
                area.x + (area.width - size.width) / 2.0,
                area.y + (area.height - size.height) / 2.0,
            );
            self.paint(cx, at, color);
        }
    }
}

/// A number that eases toward targets over time.
pub(crate) struct Tween {
    animation: taim_core::animation::Animation<f32>,
}

impl Tween {
    pub(crate) fn new(value: f32) -> Self {
        Self {
            animation: taim_core::animation::Animation::new(value).quick(),
        }
    }

    pub(crate) fn go(&mut self, target: f32, now: crate::Instant) {
        if crate::state::motion_reduced() {
            // Jump straight to the target.
            self.animation = taim_core::animation::Animation::new(target).quick();
        } else {
            self.animation.go_mut(target, now);
        }
    }

    pub(crate) fn value(&self, now: crate::Instant) -> f32 {
        self.animation.interpolate(now)
    }

    pub(crate) fn target(&self) -> f32 {
        self.animation.value()
    }

    pub(crate) fn is_moving(&self, now: crate::Instant) -> bool {
        self.animation.is_animating(now)
    }
}

/// WCAG AA contrast for text, with a little margin for rounding.
pub(crate) const TEXT_CONTRAST: f32 = 4.6;
/// WCAG contrast for graphics and controls that are not text.
pub(crate) const ICON_CONTRAST: f32 = 3.1;

/// The quietest version of `ink` on `surface`: `ink` faded towards the
/// surface as far as it can go while keeping `contrast` against it. If
/// `ink` itself has less, it is returned unchanged.
pub(crate) fn quiet(ink: crate::Color, surface: crate::Color, contrast: f32) -> crate::Color {
    if ink.relative_contrast(surface) <= contrast {
        return ink;
    }

    // Contrast falls monotonically as the ink fades into the surface.
    let (mut readable, mut too_faint) = (0.0_f32, 1.0_f32);
    for _ in 0..16 {
        let middle = (readable + too_faint) / 2.0;
        if ink.mix(surface, middle).relative_contrast(surface) >= contrast {
            readable = middle;
        } else {
            too_faint = middle;
        }
    }
    ink.mix(surface, readable)
}

/// A pointer gesture step, from either the mouse or a finger.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Gesture {
    Down(crate::Point),
    Move(crate::Point),
    Up(Option<crate::Point>),
}

/// Decodes `event` into a gesture step. Mouse positions come from
/// `pointer`, which is what the runtime tracks for the mouse.
///
/// Raw finger events are not gesture steps: the shell already delivers the
/// first finger as pointer events, and decoding both would act on every
/// touch twice. Views that want every finger (pinching) read
/// [`Event::Touch`] themselves.
pub(crate) fn gesture(event: &Event, pointer: Option<crate::Point>) -> Option<Gesture> {
    Some(match event {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => Gesture::Down(pointer?),
        Event::Mouse(mouse::Event::CursorMoved { position }) => Gesture::Move(*position),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Gesture::Up(pointer),
        _ => return None,
    })
}

/// Whether `event` asks to go back: the Android back button or gesture, a
/// browser/mouse back key, or Escape. The topmost open overlay closes.
pub(crate) fn is_back(event: &Event) -> bool {
    crate::root::is_back(event)
}

/// Whether `event` is pointer input (mouse or touch), the only kind a
/// modal overlay blocks from reaching what it covers.
pub(crate) fn is_pointer(event: &Event) -> bool {
    matches!(event, Event::Mouse(_) | Event::Touch(_))
}

#[cfg(test)]
mod contrast_tests {
    use super::{ICON_CONTRAST, TEXT_CONTRAST, quiet};
    use crate::Theme;

    #[test]
    fn secondary_text_is_quieter_than_text_yet_meets_wcag_aa_in_every_theme() {
        for theme in [Theme::Light, Theme::Dark] {
            let palette = *theme.palette();
            let ink = palette.background.base.text;

            for surface in [palette.background.base.color, palette.background.weak.color] {
                let text = quiet(ink, surface, TEXT_CONTRAST);
                let icon = quiet(ink, surface, ICON_CONTRAST);

                assert!(
                    text.relative_contrast(surface) >= 4.5,
                    "{theme:?}: secondary text reads at {:.2}:1",
                    text.relative_contrast(surface)
                );
                assert!(
                    text.relative_contrast(surface) < ink.relative_contrast(surface),
                    "{theme:?}: and is quieter than primary text"
                );
                assert!(
                    icon.relative_contrast(surface) >= 3.0,
                    "{theme:?}: a quiet icon reads at {:.2}:1",
                    icon.relative_contrast(surface)
                );
            }
        }
    }

    #[test]
    fn ink_that_is_already_faint_is_left_alone() {
        let palette = *Theme::Light.palette();
        let surface = palette.background.base.color;
        let faint = palette.background.strong.color;

        assert_eq!(quiet(faint, surface, TEXT_CONTRAST), faint);
    }
}
