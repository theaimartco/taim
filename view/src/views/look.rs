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

/// A pointer gesture step, from either the mouse or a finger.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Gesture {
    Down(crate::Point),
    Move(crate::Point),
    Up(Option<crate::Point>),
}

/// Decodes `event` into a gesture step. Mouse positions come from
/// `pointer`, which is what the runtime tracks for the mouse.
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

/// Whether `event` is pointer input (mouse or touch), the only kind a
/// modal overlay blocks from reaching what it covers.
pub(crate) fn is_pointer(event: &Event) -> bool {
    matches!(event, Event::Mouse(_) | Event::Touch(_))
}
