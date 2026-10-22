use super::text::Text;
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::Constraints;
use crate::state::{IntoProp, Prop};
use crate::{Border, Child, Event, Handled, Padding, Point, Size, View, mouse, semantics};

use taim_core::widget;

/// A clickable button with a text label.
pub fn button(label: impl IntoProp<String>) -> Button {
    let name = label.into_prop();

    Button {
        label: Child::new(Text::from_prop(name.clone())),
        name,
        on_press: None,
        enabled: Prop::fixed(true),
        padding: Padding::from([6.0, 14.0]),
        hovered: false,
        pressed: false,
        id: widget::Id::unique(),
    }
}

/// See [`button`].
pub struct Button {
    label: Child,
    name: Prop<String>,
    on_press: Option<Box<dyn Fn()>>,
    enabled: Prop<bool>,
    padding: Padding,
    hovered: bool,
    pressed: bool,
    id: widget::Id,
}

impl Button {
    /// Runs `action` when the button is clicked.
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

    fn is_active(&self) -> bool {
        self.on_press.is_some() && self.enabled.get()
    }
}

impl View for Button {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
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
        let palette = cx.theme().palette();
        let shade = if !self.is_active() {
            palette.secondary.weak
        } else if self.pressed {
            palette.primary.strong
        } else if self.hovered {
            palette.primary.base
        } else {
            palette.primary.weak
        };

        let bounds = cx.bounds();
        cx.fill(bounds, shade.color, Border::default().rounded(6.0));
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let Event::Mouse(mouse) = event else {
            return Handled::No;
        };

        let hovered = cx.is_hovered();
        if hovered != self.hovered {
            self.hovered = hovered;
            cx.request_paint();
        }

        if !self.is_active() {
            return Handled::No;
        }

        match mouse {
            mouse::Event::ButtonPressed(mouse::Button::Left) if hovered => {
                self.pressed = true;
                cx.request_paint();
                Handled::Yes
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) if self.pressed => {
                self.pressed = false;
                cx.request_paint();

                if hovered && let Some(action) = &self.on_press {
                    action();
                }
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
                    semantics::Actions::ACTIVATE
                } else {
                    semantics::Actions::empty()
                }),
        )
    }

    fn cursor(&self) -> Option<mouse::Interaction> {
        self.is_active().then_some(mouse::Interaction::Pointer)
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
