//! Layout, input, drawing, and overlay behavior for pick lists.

use super::*;
use crate::core::text::Paragraph as _;

impl<'a, T, L, V, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for PickList<'a, T, L, V, Message, Theme, Renderer>
where
    T: Clone + PartialEq + 'a,
    L: Borrow<[T]>,
    V: Borrow<T>,
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: text::Renderer + 'a,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::<Renderer::Paragraph>::new())
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: Length::Shrink,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

        let font = self.text.font.unwrap_or_else(|| renderer.default_font());
        let text_size = self.text.size.unwrap_or_else(|| renderer.default_size());
        let options = self.options.borrow();
        let (closed_handle, open_handle) = match &self.handle {
            Handle::Arrow { size } => {
                let icon = (
                    Renderer::ICON_FONT,
                    Renderer::ARROW_DOWN_ICON,
                    *size,
                    text::LineHeight::default(),
                    text::Shaping::Basic,
                );
                (Some(icon), Some(icon))
            }
            Handle::Static(icon) => {
                let icon = Some((
                    icon.font,
                    icon.code_point,
                    icon.size,
                    icon.line_height,
                    icon.shaping,
                ));
                (icon, icon)
            }
            Handle::Dynamic { closed, open } => (
                Some((
                    closed.font,
                    closed.code_point,
                    closed.size,
                    closed.line_height,
                    closed.shaping,
                )),
                Some((
                    open.font,
                    open.code_point,
                    open.size,
                    open.line_height,
                    open.shaping,
                )),
            ),
            Handle::None => (None, None),
        };
        let handle_width = [closed_handle, open_handle]
            .into_iter()
            .flatten()
            .map(|(_, _, size, _, _)| size.unwrap_or(text_size).0)
            .fold(0.0, f32::max);

        let option_text = Text {
            content: "",
            bounds: Size::new(
                limits.max().width,
                self.text.line_height.to_absolute(text_size).into(),
            ),
            size: text_size,
            line_height: self.text.line_height,
            font,
            align_x: text::Alignment::Default,
            align_y: alignment::Vertical::Center,
            shaping: self.text.shaping,
            wrapping: text::Wrapping::None,
            ellipsis: self.text.ellipsis,
            hint_factor: renderer.scale_factor(),
        };

        if let Some(placeholder) = &self.placeholder {
            let _ = state.placeholder.update(Text {
                content: placeholder,
                ..option_text
            });
        }

        let max_width = match self.width {
            Length::Shrink => {
                state.options.resize_with(options.len(), Default::default);

                for (option, paragraph) in options.iter().zip(state.options.iter_mut()) {
                    let label = (self.to_string)(option);

                    let _ = paragraph.update(Text {
                        content: &label,
                        ..option_text
                    });
                }

                let labels_width = state.options.iter().fold(0.0, |width, paragraph| {
                    f32::max(width, paragraph.min_width())
                });

                labels_width.max(
                    self.placeholder
                        .as_ref()
                        .map(|_| state.placeholder.min_width())
                        .unwrap_or(0.0),
                )
            }
            _ => 0.0,
        };

        let size = {
            let intrinsic = Size::new(
                max_width + handle_width + self.padding.x(),
                f32::from(self.text.line_height.to_absolute(text_size)),
            );

            limits
                .width(self.width)
                .shrink(self.padding)
                .resolve(self.width, Length::Shrink, intrinsic)
                .expand(self.padding)
        };

        let selected_label = self
            .selected
            .as_ref()
            .map(Borrow::borrow)
            .map(&self.to_string);
        state.label_is_placeholder = selected_label.is_none();

        let _ = state.label.update(Text {
            content: selected_label
                .as_deref()
                .or(self.placeholder.as_deref())
                .unwrap_or_default(),
            bounds: Size::new(
                (size.width - self.padding.x() - handle_width).max(0.0),
                f32::from(self.text.line_height.to_absolute(text_size)),
            ),
            ..option_text
        });

        let update_handle = |paragraph: &mut text::paragraph::Plain<Renderer::Paragraph>,
                             icon: Option<(
            Renderer::Font,
            char,
            Option<Pixels>,
            text::LineHeight,
            text::Shaping,
        )>| {
            let mut encoded = [0; 4];
            let (content, font, size, line_height, shaping) =
                if let Some((font, code_point, size, line_height, shaping)) = icon {
                    (
                        code_point.encode_utf8(&mut encoded) as &str,
                        font,
                        size.unwrap_or(text_size),
                        line_height,
                        shaping,
                    )
                } else {
                    (
                        "",
                        Renderer::ICON_FONT,
                        text_size,
                        text::LineHeight::default(),
                        text::Shaping::Basic,
                    )
                };

            let _ = paragraph.update(Text {
                content,
                size,
                line_height,
                font,
                bounds: Size::new(handle_width, f32::from(line_height.to_absolute(size))),
                align_x: text::Alignment::Default,
                align_y: alignment::Vertical::Center,
                shaping,
                wrapping: text::Wrapping::None,
                ellipsis: text::Ellipsis::None,
                hint_factor: renderer.scale_factor(),
            });
        };
        update_handle(&mut state.closed_handle, closed_handle);
        update_handle(&mut state.open_handle, open_handle);

        layout::Node::new(size)
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
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        let is_hovered = cursor.is_over(layout.bounds());

        if state.is_hovered != is_hovered {
            state.is_hovered = is_hovered;
            shell.request_redraw();
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if state.is_open {
                    // Event wasn't processed by overlay, so cursor was clicked either outside its
                    // bounds or on the drop-down, either way we close the overlay.
                    state.is_open = false;

                    if let Some(on_close) = self
                        .events
                        .lifecycle
                        .as_deref()
                        .and_then(|lifecycle| lifecycle.close.as_ref())
                    {
                        shell.publish(on_close.clone());
                    }

                    shell.request_redraw();
                    shell.capture_event();
                } else if self.events.select.is_some() && is_hovered {
                    let selected = self.selected.as_ref().map(Borrow::borrow);

                    state.is_open = true;
                    state.hovered_option = self
                        .options
                        .borrow()
                        .iter()
                        .position(|option| Some(option) == selected);

                    if let Some(on_open) = self
                        .events
                        .lifecycle
                        .as_deref()
                        .and_then(|lifecycle| lifecycle.open.as_ref())
                    {
                        shell.publish(on_open.clone());
                    }

                    shell.request_redraw();
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { y, .. },
            }) => {
                let Some(on_select) = &self.events.select else {
                    return;
                };

                if state.keyboard_modifiers.command()
                    && cursor.is_over(layout.bounds())
                    && !state.is_open
                {
                    fn find_next<'a, T: PartialEq>(
                        selected: &'a T,
                        mut options: impl Iterator<Item = &'a T>,
                    ) -> Option<&'a T> {
                        let _ = options.find(|&option| option == selected);

                        options.next()
                    }

                    let options = self.options.borrow();
                    let selected = self.selected.as_ref().map(Borrow::borrow);

                    let next_option = if *y < 0.0 {
                        if let Some(selected) = selected {
                            find_next(selected, options.iter())
                        } else {
                            options.first()
                        }
                    } else if *y > 0.0 {
                        if let Some(selected) = selected {
                            find_next(selected, options.iter().rev())
                        } else {
                            options.last()
                        }
                    } else {
                        None
                    };

                    if let Some(next_option) = next_option {
                        shell.publish(on_select(next_option.clone()));
                        shell.capture_event();
                    }
                }
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                state.keyboard_modifiers = *modifiers;
            }
            _ => {}
        };
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        let is_mouse_over = cursor.is_over(bounds);

        if is_mouse_over {
            if self.events.select.is_some() {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::Idle
            }
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();

        let bounds = layout.bounds();

        if bounds.width <= 0.0 || bounds.height <= 0.0 || !bounds.intersects(viewport) {
            return;
        }

        let status = if self.events.select.is_none() {
            Status::Disabled
        } else if state.is_open {
            Status::Opened {
                is_hovered: state.is_hovered,
            }
        } else if state.is_hovered {
            Status::Hovered
        } else {
            Status::Active
        };

        let style = Catalog::style(theme, &self.class, status);

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: style.border,
                ..renderer::Quad::default()
            },
            style.background,
        );

        let handle = if state.is_open {
            &state.open_handle
        } else {
            &state.closed_handle
        };
        if !handle.content().is_empty() {
            let paragraph = handle.raw();
            let handle_bounds = Rectangle::new(
                bounds.position(),
                Size::new((bounds.width - self.padding.right).max(0.0), bounds.height),
            );

            renderer.fill_paragraph(
                paragraph,
                handle_bounds.anchor(
                    paragraph.min_bounds(),
                    alignment::Horizontal::Right,
                    alignment::Vertical::Center,
                ),
                style.handle_color,
                *viewport,
            );
        }

        if !state.label.content().is_empty() {
            let paragraph = state.label.raw();
            let text_bounds = Rectangle::new(
                Point::new(bounds.x + self.padding.left, bounds.y),
                Size::new(paragraph.bounds().width, bounds.height),
            );

            renderer.fill_paragraph(
                paragraph,
                text_bounds.anchor(
                    paragraph.min_bounds(),
                    alignment::Horizontal::Left,
                    alignment::Vertical::Center,
                ),
                if state.label_is_placeholder {
                    style.placeholder_color
                } else {
                    style.text_color
                },
                *viewport,
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let Some(on_select) = &self.events.select else {
            return None;
        };

        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        let font = self.text.font.unwrap_or_else(|| renderer.default_font());

        if state.is_open {
            let bounds = layout.bounds();

            let mut menu = Menu::new(
                &mut state.menu,
                self.options.borrow(),
                &mut state.hovered_option,
                &self.to_string,
                |option| {
                    state.is_open = false;

                    (on_select)(option)
                },
                None,
                &self.menu_class,
            )
            .width(bounds.width)
            .padding(self.padding)
            .font(font)
            .ellipsis(self.text.ellipsis)
            .shaping(self.text.shaping);

            if let Some(text_size) = self.text.size {
                menu = menu.text_size(text_size);
            }

            Some(menu.overlay(
                layout.position() + translation,
                *viewport,
                bounds.height,
                self.menu_height,
            ))
        } else {
            None
        }
    }
}
