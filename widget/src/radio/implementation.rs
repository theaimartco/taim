use super::*;

struct State<P: text::Paragraph> {
    text: widget::text::State<P>,
    last_status: Option<Status>,
}

impl<P: text::Paragraph> Default for State<P> {
    fn default() -> Self {
        Self {
            text: widget::text::State::default(),
            last_status: None,
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Radio<'_, Message, Theme, Renderer>
where
    Message: Clone,
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
        layout::next_to_each_other(
            &limits.width(self.width),
            self.spacing,
            |_| layout::Node::new(Size::new(self.size, self.size)),
            |limits| {
                let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

                widget::text::layout(
                    &mut state.text,
                    renderer,
                    limits,
                    self.label.as_ref(),
                    widget::text::Format {
                        width: self.width,
                        height: Length::Shrink,
                        line_height: self.line_height,
                        size: self.text_size,
                        font: self.font,
                        align_x: text::Alignment::Default,
                        align_y: alignment::Vertical::Top,
                        shaping: self.shaping,
                        wrapping: self.wrapping,
                        ellipsis: text::Ellipsis::default(),
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
        let is_mouse_over = cursor.is_over(layout.bounds());

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. })
                if is_mouse_over =>
            {
                shell.publish(self.on_click.clone());
                shell.capture_event();
            }
            _ => {}
        }

        let current_status = if is_mouse_over {
            Status::Hovered {
                is_selected: self.is_selected,
            }
        } else {
            Status::Active {
                is_selected: self.is_selected,
            }
        };
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

        if matches!(event, Event::Window(window::Event::RedrawRequested(_))) {
            state.last_status = Some(current_status);
        } else if state
            .last_status
            .is_some_and(|last_status| last_status != current_status)
        {
            shell.request_redraw();
        }
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        let mut flags = SemanticState::FOCUSABLE;

        if self.is_selected {
            flags |= SemanticState::CHECKED | SemanticState::SELECTED;
        }

        Some(
            Node::new(tree.id().clone(), layout.bounds(), Role::RadioButton)
                .with_state(flags)
                .with_actions(Actions::ACTIVATE | Actions::FOCUS)
                .with_name(self.label.as_ref()),
        )
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
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

        let mut children = layout.children();
        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();
        let style = theme.style(
            &self.class,
            state.last_status.unwrap_or(Status::Active {
                is_selected: self.is_selected,
            }),
        );

        if let Some(indicator) = children.next() {
            let bounds = indicator.bounds();
            let dot_size = bounds.width / 2.0;

            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: (bounds.width / 2.0).into(),
                        width: style.border_width,
                        color: style.border_color,
                    },
                    ..renderer::Quad::default()
                },
                style.background,
            );

            if self.is_selected {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: bounds.x + dot_size / 2.0,
                            y: bounds.y + dot_size / 2.0,
                            width: bounds.width - dot_size,
                            height: bounds.height - dot_size,
                        },
                        border: border::rounded(dot_size / 2.0),
                        ..renderer::Quad::default()
                    },
                    style.dot_color,
                );
            }
        }

        if let Some(label_layout) = children.next() {
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
    }
}

impl<'a, Message, Theme, Renderer> From<Radio<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog,
    Renderer: 'a + text::Renderer,
{
    fn from(radio: Radio<'a, Message, Theme, Renderer>) -> Self {
        Self::new(radio)
    }
}
