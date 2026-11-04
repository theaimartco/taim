use super::look::{Press, Tween};
use super::text::Text;
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::Constraints;
use crate::state::{IntoProp, Prop};
use crate::{Border, Child, Color, Event, Handled, Padding, Point, Size, View, mouse, semantics};

use taim_core::widget;

/// A clickable button with a text label.
pub fn button(label: impl IntoProp<String>) -> Button {
    let name = label.into_prop();

    Button {
        label: Child::new(Text::from_prop(name.clone())),
        name,
        kind: Kind::Primary,
        on_press: None,
        enabled: Prop::fixed(true),
        padding: Padding::from([9.0, 16.0]),
        hover: Tween::new(0.0),
        hovered: false,
        pressed: None,
        id: widget::Id::unique(),
    }
}

/// How prominent a [`Button`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A solid accent fill: the one thing to do here.
    Primary,
    /// An outlined surface: an alternative action.
    Secondary,
    /// Only text until hovered: a quiet action, such as in a toolbar.
    Ghost,
    /// A solid red fill: an action that destroys something.
    Danger,
}

/// See [`button`].
pub struct Button {
    label: Child,
    name: Prop<String>,
    kind: Kind,
    on_press: Option<Box<dyn Fn()>>,
    enabled: Prop<bool>,
    padding: Padding,
    hover: Tween,
    hovered: bool,
    pressed: Option<Press>,
    id: widget::Id,
}

const RADIUS: f32 = 8.0;

impl Button {
    /// Runs `action` when the button is clicked, or activated from the
    /// keyboard with Space or Enter.
    pub fn on_press(mut self, action: impl Fn() + 'static) -> Self {
        self.on_press = Some(Box::new(action));
        self
    }

    /// Whether the button can be clicked; a closure follows state.
    pub fn enabled(mut self, enabled: impl IntoProp<bool>) -> Self {
        self.enabled = enabled.into_prop();
        self
    }

    /// Space between the edge and the label.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// An outlined button, for an alternative action.
    pub fn secondary(mut self) -> Self {
        self.kind = Kind::Secondary;
        self
    }

    /// A button that is only text until hovered.
    pub fn ghost(mut self) -> Self {
        self.kind = Kind::Ghost;
        self
    }

    /// A button for an action that destroys something.
    pub fn danger(mut self) -> Self {
        self.kind = Kind::Danger;
        self
    }

    /// Shows `label` instead of the text label; the name given to
    /// [`button`] is still what assistive technology announces.
    pub fn label_view(mut self, label: impl Into<crate::AnyView>) -> Self {
        self.label = Child::new(label.into());
        self
    }

    fn is_active(&self) -> bool {
        self.on_press.is_some() && self.enabled.get()
    }

    fn fire(&self) {
        if let Some(action) = &self.on_press {
            action();
        }
    }
}

impl View for Button {
    fn cancel_interaction(&mut self) {
        self.pressed = None;
        self.hovered = false;
        self.hover = Tween::new(0.0);
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        if !self.is_active() {
            self.pressed = None;
        }
        let inner = constraints.inset(self.padding).loosen();
        let label = self.label.measure(cx, inner);
        self.label
            .place(Point::new(self.padding.left, self.padding.top));

        Size::new(
            label.width + self.padding.left + self.padding.right,
            label.height + self.padding.top + self.padding.bottom,
        )
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let palette = *cx.theme().palette();
        let bounds = cx.bounds();
        let lift = self.hover.value(cx.now());
        let active = self.is_active();

        let (fill, edge) = match (self.kind, active) {
            // A quiet action stays only its label when it cannot be used.
            (Kind::Ghost, false) => (Color::TRANSPARENT, Color::TRANSPARENT),
            (_, false) => (palette.background.weak.color, palette.background.weak.color),
            (Kind::Primary, true) => {
                let rest = palette.primary.base.color;
                let lifted = if self.pressed.is_some() {
                    palette.primary.strong.color.mix(Color::BLACK, 0.12)
                } else {
                    palette.primary.strong.color
                };
                (
                    rest.mix(lifted, lift.max(f32::from(self.pressed.is_some()))),
                    rest,
                )
            }
            (Kind::Danger, true) => {
                let rest = palette.danger.base.color;
                let lifted = if self.pressed.is_some() {
                    palette.danger.strong.color.mix(Color::BLACK, 0.12)
                } else {
                    palette.danger.strong.color
                };
                (
                    rest.mix(lifted, lift.max(f32::from(self.pressed.is_some()))),
                    rest,
                )
            }
            (Kind::Secondary, true) => {
                let rest = palette.background.weak.color;
                let lifted = if self.pressed.is_some() {
                    palette.background.strong.color
                } else {
                    palette.background.neutral.color
                };
                (
                    rest.mix(lifted, lift.max(f32::from(self.pressed.is_some()))),
                    palette
                        .background
                        .strong
                        .color
                        .mix(palette.primary.base.color, lift * 0.6),
                )
            }
            (Kind::Ghost, true) => {
                let tint = if self.pressed.is_some() {
                    palette.background.neutral.color
                } else {
                    palette.background.weak.color
                };
                let shown = lift.max(f32::from(self.pressed.is_some()));
                (Color { a: shown, ..tint }, Color::TRANSPARENT)
            }
        };

        let outline = if self.kind == Kind::Secondary {
            1.0
        } else {
            0.0
        };
        cx.fill(
            bounds,
            fill,
            Border::default().width(outline).color(edge).rounded(RADIUS),
        );

        if cx.has_focus() {
            super::look::focus_ring(cx, bounds, RADIUS);
        }
    }

    fn foreground(&self, theme: &crate::Theme) -> Option<Color> {
        let palette = theme.palette();

        if !self.is_active() {
            let text = palette.background.base.text;
            return Some(text.mix(palette.background.base.color, 0.55));
        }

        match self.kind {
            Kind::Primary => Some(palette.primary.base.text),
            Kind::Danger => Some(palette.danger.base.text),
            Kind::Secondary | Kind::Ghost => None,
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        use taim_core::{touch, window};
        if matches!(event, Event::Window(window::Event::Unfocused))
            || matches!(event, Event::Touch(touch::Event::FingerLost { id, .. })
                if self.pressed == Some(Press::Touch(*id)))
        {
            self.pressed = None;
            self.hovered = false;
            self.hover.go(0.0, cx.now());
            cx.request_animation();
            return Handled::No;
        }
        if super::look::is_activation(event) && cx.has_focus() && self.is_active() {
            self.fire();
            return Handled::Yes;
        }

        let hovered = match event {
            Event::Mouse(_) => cx.is_hovered(),
            Event::Touch(
                touch::Event::FingerPressed { position, .. }
                | touch::Event::FingerMoved { position, .. }
                | touch::Event::FingerLifted { position, .. },
            ) => cx.bounds().contains(*position),
            _ => return Handled::No,
        };
        if hovered != self.hovered {
            self.hovered = hovered;
            self.hover.go(f32::from(hovered), cx.now());
            cx.request_animation();
        }
        if !self.is_active() {
            self.pressed = None;
            return Handled::No;
        }
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if hovered && self.pressed.is_none() =>
            {
                self.pressed = Some(Press::Mouse);
                cx.request_focus();
                cx.request_paint();
                Handled::Yes
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if self.pressed == Some(Press::Mouse) =>
            {
                self.pressed = None;
                cx.request_paint();
                if hovered {
                    self.fire();
                }
                Handled::Yes
            }
            Event::Touch(touch::Event::FingerPressed { id, .. })
                if hovered && self.pressed.is_none() =>
            {
                self.pressed = Some(Press::Touch(*id));
                cx.request_focus();
                cx.request_paint();
                Handled::Yes
            }
            Event::Touch(touch::Event::FingerLifted { id, .. })
                if self.pressed == Some(Press::Touch(*id)) =>
            {
                self.pressed = None;
                self.hovered = false;
                self.hover.go(0.0, cx.now());
                cx.request_animation();
                if hovered {
                    self.fire();
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn is_animating(&self, now: crate::Instant) -> bool {
        self.hover.is_moving(now)
    }

    fn accessibility_action(
        &mut self,
        cx: &mut EventCx<'_>,
        request: &semantics::Request,
    ) -> Handled {
        match request {
            semantics::Request::Activate if self.is_active() => {
                self.pressed = None;
                cx.request_focus();
                self.fire();
                cx.request_paint();
                Handled::Yes
            }
            semantics::Request::Focus if self.focusable() => {
                cx.request_focus();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        let enabled = self.is_active();
        Some(
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::Button)
                .with_name(self.name.get())
                .with_state(super::look::interactive_state(enabled, cx.has_focus()))
                .with_actions(if enabled {
                    semantics::Actions::ACTIVATE | semantics::Actions::FOCUS
                } else {
                    semantics::Actions::empty()
                }),
        )
    }

    fn cursor(&self) -> Option<mouse::Interaction> {
        self.is_active().then_some(mouse::Interaction::Pointer)
    }

    fn focusable(&self) -> bool {
        self.is_active()
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.label);
    }
}

/// Lets a [`Text`] be used where a label is expected.
impl From<Text> for Child {
    fn from(text: Text) -> Self {
        Child::new(text)
    }
}
