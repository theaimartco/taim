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

/// The device that owns a pending click or drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Press {
    Mouse,
    Touch(taim_core::touch::Finger),
}

impl Press {
    pub(crate) fn start(event: &Event) -> Option<Self> {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => Some(Self::Mouse),
            Event::Touch(taim_core::touch::Event::FingerPressed { id, .. }) => {
                Some(Self::Touch(*id))
            }
            _ => None,
        }
    }

    pub(crate) fn released_by(self, event: &Event) -> bool {
        matches!(
            (self, event),
            (
                Self::Mouse,
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            )
        ) || matches!((self, event), (Self::Touch(owner),
                Event::Touch(taim_core::touch::Event::FingerLifted { id, .. })) if owner == *id)
    }

    pub(crate) fn cancelled_by(self, event: &Event) -> bool {
        matches!(event, Event::Window(taim_core::window::Event::Unfocused))
            || matches!((self, event), (Self::Touch(owner),
                Event::Touch(taim_core::touch::Event::FingerLost { id, .. })) if owner == *id)
    }

    pub(crate) fn movement(self, event: &Event) -> Option<crate::Point> {
        let position = match (self, event) {
            (Self::Mouse, Event::Mouse(mouse::Event::CursorMoved { position })) => *position,
            (
                Self::Touch(owner),
                Event::Touch(taim_core::touch::Event::FingerMoved { id, position, .. }),
            ) if owner == *id => *position,
            _ => return None,
        };
        (position.x.is_finite() && position.y.is_finite()).then_some(position)
    }
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
        let content = content.into();
        Self {
            font: super::Icon::is_glyph(&content).then_some(super::Icon::FONT),
            content,
            size: size.map(crate::Pixels),
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

/// How a spring moves: its natural frequency and damping.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Tuning {
    /// Natural frequency, radians per second.
    pub(crate) omega: f32,
    /// Damping ratio; below `1` it overshoots.
    pub(crate) zeta: f32,
}

/// A quick press with a little overshoot.
pub(crate) const PRESS: Tuning = Tuning {
    omega: 26.0,
    zeta: 0.62,
};
/// A loose wobble back into shape.
pub(crate) const RELEASE: Tuning = Tuning {
    omega: 22.0,
    zeta: 0.32,
};
/// A calm fade for the hover light.
pub(crate) const HOVER: Tuning = Tuning {
    omega: 14.0,
    zeta: 1.0,
};

/// A damped spring, solved in closed form from its last retarget, so its
/// value at any instant needs no stepping and no mutation.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Spring {
    tuning: Tuning,
    /// When it was last retargeted; `None` at rest.
    start: Option<crate::Instant>,
    from: f32,
    velocity: f32,
    target: f32,
}

impl Spring {
    pub(crate) const fn new(value: f32, tuning: Tuning) -> Self {
        Self {
            tuning,
            start: None,
            from: value,
            velocity: 0.0,
            target: value,
        }
    }

    /// `(value, velocity)` at `now`.
    pub(crate) fn sample(&self, now: crate::Instant) -> (f32, f32) {
        let Some(start) = self.start else {
            return (self.target, 0.0);
        };
        let t = now.saturating_duration_since(start).as_secs_f32();
        let Tuning { omega, zeta } = self.tuning;
        let offset = self.from - self.target;

        if zeta < 1.0 {
            let damped = omega * (1.0 - zeta * zeta).sqrt();
            let a = offset;
            let b = (self.velocity + zeta * omega * a) / damped;
            let decay = (-zeta * omega * t).exp();
            let (sin, cos) = (damped * t).sin_cos();
            let value = self.target + decay * (a * cos + b * sin);
            let velocity =
                decay * ((-zeta * omega) * (a * cos + b * sin) + damped * (b * cos - a * sin));
            (value, velocity)
        } else {
            // Critically damped.
            let a = offset;
            let b = self.velocity + omega * a;
            let decay = (-omega * t).exp();
            let value = self.target + (a + b * t) * decay;
            let velocity = (b - omega * (a + b * t)) * decay;
            (value, velocity)
        }
    }

    pub(crate) fn value(&self, now: crate::Instant) -> f32 {
        self.sample(now).0
    }

    pub(crate) fn go(&mut self, target: f32, now: crate::Instant) {
        if crate::state::motion_reduced() {
            *self = Self::new(target, self.tuning);
            return;
        }
        let (value, velocity) = self.sample(now);
        self.start = Some(now);
        self.from = value;
        self.velocity = velocity;
        self.target = target;
    }

    pub(crate) fn is_moving(&self, now: crate::Instant) -> bool {
        let (value, velocity) = self.sample(now);
        self.start.is_some() && ((value - self.target).abs() > 1.0e-3 || velocity.abs() > 1.0e-2)
    }
}

/// A firm snap with almost no overshoot: indicators, thumbs.
pub(crate) const SNAP: Tuning = Tuning {
    omega: 30.0,
    zeta: 0.82,
};
/// A lively pop that overshoots: ticks, dots, badges appearing.
pub(crate) const POP: Tuning = Tuning {
    omega: 24.0,
    zeta: 0.45,
};
/// The trailing edge of a stretchy indicator: slower than [`SNAP`], so the
/// indicator stretches towards its target and catches up.
pub(crate) const TRAIL: Tuning = Tuning {
    omega: 17.0,
    zeta: 0.9,
};
/// A soft glide for layout changes and entrances.
pub(crate) const GLIDE: Tuning = Tuning {
    omega: 18.0,
    zeta: 0.86,
};

impl Spring {
    /// Jumps to `value` with no motion.
    pub(crate) fn set(&mut self, value: f32) {
        *self = Self::new(value, self.tuning);
    }

    /// Where the spring is heading.
    pub(crate) fn target(&self) -> f32 {
        self.target
    }

    /// Flicks the spring with `velocity` from where it is, so it bumps away
    /// from its target and settles back: a bounce.
    pub(crate) fn kick(&mut self, velocity: f32, now: crate::Instant) {
        if crate::state::motion_reduced() {
            return;
        }
        let (value, current) = self.sample(now);
        self.start = Some(now);
        self.from = value;
        self.velocity = current + velocity;
    }

    /// Its speed at `now`, in units per second.
    pub(crate) fn velocity(&self, now: crate::Instant) -> f32 {
        self.sample(now).1
    }
}

/// The springy press every tappable view shares: it sinks a little under
/// the finger and springs back, overshooting slightly, when let go.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Squish {
    spring: Spring,
    /// How far it shrinks when fully pressed, as a fraction of its size.
    depth: f32,
}

impl Squish {
    /// A squish for a control: about 4% smaller under the finger.
    pub(crate) const fn new() -> Self {
        Self::with_depth(0.04)
    }

    /// A squish `depth` deep: `0.02` for a wide row, `0.08` for a small
    /// icon.
    pub(crate) const fn with_depth(depth: f32) -> Self {
        Self {
            spring: Spring::new(0.0, PRESS),
            depth,
        }
    }

    pub(crate) fn press(&mut self, now: crate::Instant) {
        self.spring.go(1.0, now);
    }

    pub(crate) fn release(&mut self, now: crate::Instant) {
        self.spring.go(0.0, now);
    }

    pub(crate) fn is_moving(&self, now: crate::Instant) -> bool {
        self.spring.is_moving(now)
    }

    /// The scale to draw at `now`.
    pub(crate) fn visual(&self, now: crate::Instant) -> crate::Visual {
        crate::Visual {
            scale: 1.0 - self.depth * self.spring.value(now),
            ..crate::Visual::IDENTITY
        }
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
    use taim_core::touch;

    if crate::tree::is_cancellation(event) {
        return Some(Gesture::Up(None));
    }

    Some(match event {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => Gesture::Down(pointer?),
        Event::Mouse(mouse::Event::CursorMoved { position }) => Gesture::Move(*position),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Gesture::Up(pointer),
        Event::Touch(touch::Event::FingerPressed { position, .. }) => Gesture::Down(*position),
        Event::Touch(touch::Event::FingerMoved { position, .. }) => Gesture::Move(*position),
        Event::Touch(touch::Event::FingerLifted { position, .. }) => Gesture::Up(Some(*position)),
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
