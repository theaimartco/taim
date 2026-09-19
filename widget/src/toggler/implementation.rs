use crate::core::alignment;
use crate::core::border;
use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::text;
use crate::core::touch;
use crate::core::widget;
use crate::core::widget::tree::{self, Tree};
use crate::core::{Border, Element, Event, Layout, Length, Rectangle, Shell, Size, Widget};

use super::{Catalog, Toggler};

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Toggler<'_, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::<Renderer::Paragraph>::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits.width(self.width);
        let scale = valid_scale(renderer.scale_factor());

        layout::next_to_each_other(
            &limits,
            if self.label.is_some() {
                self.spacing
            } else {
                0.0
            },
            |_| {
                let size = if renderer::CRISP {
                    (self.size * scale).round() / scale
                } else {
                    self.size
                };
                layout::Node::new(Size::new(2.0 * size, size))
            },
            |limits| {
                let Some(label) = self.label.as_deref() else {
                    return layout::Node::new(Size::ZERO);
                };
                let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

                widget::text::layout(
                    &mut state.text,
                    renderer,
                    limits,
                    label,
                    widget::text::Format {
                        width: self.width,
                        height: Length::Shrink,
                        line_height: self.line_height,
                        size: self.text_size,
                        font: self.font,
                        align_x: self.alignment,
                        align_y: alignment::Vertical::Top,
                        shaping: self.text_shaping,
                        wrapping: self.wrapping,
                        ellipsis: text::Ellipsis::None,
                    },
                )
            },
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let hovered = cursor.is_over(layout.bounds());
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

        if state.hovered != hovered {
            state.hovered = hovered;
            shell.request_redraw();
        }

        if hovered
            && matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                    | Event::Touch(touch::Event::FingerPressed { .. })
            )
            && let Some(on_toggle) = &self.on_toggle
        {
            shell.publish(on_toggle(!self.is_toggled));
            shell.capture_event();
        }
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        let enabled = self.on_toggle.is_some();
        let mut flags = if enabled {
            SemanticState::FOCUSABLE
        } else {
            SemanticState::DISABLED
        };
        if self.is_toggled {
            flags |= SemanticState::CHECKED;
        }

        let node = Node::new(tree.id().clone(), layout.bounds(), Role::Checkbox)
            .with_state(flags)
            .with_actions(if enabled {
                Actions::ACTIVATE | Actions::FOCUS
            } else {
                Actions::empty()
            });

        Some(match &self.label {
            Some(label) => node.with_name(label.as_ref()),
            None => node,
        })
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if !cursor.is_over(layout.bounds()) {
            mouse::Interaction::None
        } else if self.on_toggle.is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::NotAllowed
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        defaults: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if !layout.bounds().intersects(viewport) {
            return;
        }

        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();
        let style = theme.style(&self.class, self.status(state.hovered));
        let mut children = layout.children();
        let toggler_layout = children.next().expect("Toggler track layout");

        if self.label.is_some() {
            let label_layout = children.next().expect("Toggler label layout");
            crate::text::draw(
                renderer,
                defaults,
                label_layout.bounds(),
                state.text.raw(),
                crate::text::Style {
                    color: style.text_color,
                },
                viewport,
            );
        }

        let scale = valid_scale(renderer.scale_factor());
        let bounds = toggler_layout.bounds();
        let radius = style
            .border_radius
            .unwrap_or_else(|| border::Radius::new(bounds.height / 2.0));

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    radius,
                    width: style.background_border_width.max(0.0),
                    color: style.background_border_color,
                },
                ..renderer::Quad::default()
            },
            style.background,
        );

        let physical = if renderer::CRISP {
            (bounds * scale).round()
        } else {
            bounds
        };
        let padding = (style.padding_ratio.clamp(0.0, 0.5) * physical.height).round();
        let thumb = Rectangle {
            x: physical.x
                + if self.is_toggled {
                    physical.width - physical.height + padding
                } else {
                    padding
                },
            y: physical.y + padding,
            width: (physical.height - 2.0 * padding).max(0.0),
            height: (physical.height - 2.0 * padding).max(0.0),
        } * scale.recip();

        renderer.fill_quad(
            renderer::Quad {
                bounds: thumb,
                border: Border {
                    radius,
                    width: style.foreground_border_width.max(0.0),
                    color: style.foreground_border_color,
                },
                ..renderer::Quad::default()
            },
            style.foreground,
        );
    }
}

#[derive(Default)]
struct State<Paragraph: text::Paragraph> {
    text: widget::text::State<Paragraph>,
    hovered: bool,
}

#[inline]
fn valid_scale(scale: Option<f32>) -> f32 {
    scale
        .filter(|scale| scale.is_finite() && *scale > 0.0)
        .unwrap_or(1.0)
}

impl<'a, Message, Theme, Renderer> From<Toggler<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: Catalog + 'a,
    Renderer: text::Renderer + 'a,
{
    fn from(toggler: Toggler<'a, Message, Theme, Renderer>) -> Self {
        Element::new(toggler)
    }
}
