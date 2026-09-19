//! Widget-tree behavior for searchable combo boxes.

use super::*;

impl<T, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for ComboBox<'_, T, Message, Theme, Renderer>
where
    T: Display + Clone + 'static,
    Message: Clone,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    fn size(&self) -> Size<Length> {
        Widget::<TextInputEvent, Theme, Renderer>::size(&self.text_input)
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let is_focused = {
            let text_input_state = tree.children[0]
                .state
                .downcast_ref::<text_input::State<Renderer::Paragraph>>();

            text_input_state.is_focused()
        };

        self.text_input.layout(
            &mut tree.children[0],
            renderer,
            limits,
            (!is_focused).then_some(&self.selection),
        )
    }

    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<Menu<T>>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(Menu::<T> {
            menu: menu::State::new(),
            filtered_options: Filtered::empty(),
            hovered_option: Some(0),
            new_selection: None,
            input_events: Vec::new(),
        })
    }

    fn diff(&mut self, tree: &mut widget::Tree) {
        tree.diff_children(&mut [&mut self.text_input as &mut dyn Widget<_, _, _>]);
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let menu = tree.state.downcast_mut::<Menu<T>>();

        let started_focused = {
            let text_input_state = tree.children[0]
                .state
                .downcast_ref::<text_input::State<Renderer::Paragraph>>();

            text_input_state.is_focused()
        };
        // This is intended to check whether or not the message buffer was empty,
        // since `Shell` does not expose such functionality.
        let mut published_message_to_shell = false;

        // Create a new list of local messages
        let mut local_messages = std::mem::take(&mut menu.input_events);
        local_messages.clear();
        let mut local_shell = shell.local(&mut local_messages);

        // Provide it to the widget
        self.text_input.update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            &mut local_shell,
            viewport,
        );

        if local_shell.is_event_captured() {
            shell.capture_event();
        }

        shell.request_redraw_at(local_shell.redraw_request());
        shell.request_input_method(local_shell.input_method());
        shell.clipboard_mut().merge(local_shell.clipboard_mut());
        drop(local_shell);

        // Then finally react to them here
        for message in local_messages.drain(..) {
            let TextInputEvent::TextChanged(new_value) = message;

            if let Some(on_input) = &self.on_input {
                shell.publish((on_input)(new_value.clone()));
            }

            // Couple the filtered options with the `ComboBox`
            // value and only recompute them when the value changes,
            // instead of doing it in every `view` call
            self.state.with_inner_mut(|state| {
                menu.hovered_option = Some(0);
                state.value = new_value;

                state.filtered_options.update(
                    search(&self.state.options, &state.option_matchers, &state.value)
                        .cloned()
                        .collect(),
                );
            });
            shell.invalidate_layout();
            shell.request_redraw();
        }
        menu.input_events = local_messages;

        let is_focused = {
            let text_input_state = tree.children[0]
                .state
                .downcast_ref::<text_input::State<Renderer::Paragraph>>();

            text_input_state.is_focused()
        };

        if is_focused {
            self.state.with_inner(|state| {
                if !started_focused && let Some(on_option_hovered) = &mut self.on_option_hovered {
                    let hovered_option = menu.hovered_option.unwrap_or(0);

                    if let Some(option) = state.filtered_options.options.get(hovered_option) {
                        shell.publish(on_option_hovered(option.clone()));
                        published_message_to_shell = true;
                    }
                }

                if let Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(named_key),
                    modifiers,
                    ..
                }) = event
                {
                    let shift_modifier = modifiers.shift();
                    match (named_key, shift_modifier) {
                        (key::Named::Enter, _) => {
                            if let Some(index) = &menu.hovered_option
                                && let Some(option) = state.filtered_options.options.get(*index)
                            {
                                menu.new_selection = Some(option.clone());
                            }

                            shell.capture_event();
                            shell.request_redraw();
                        }
                        (key::Named::ArrowUp, _) | (key::Named::Tab, true) => {
                            if let Some(index) = &mut menu.hovered_option {
                                if *index == 0 {
                                    *index = state.filtered_options.options.len().saturating_sub(1);
                                } else {
                                    *index = index.saturating_sub(1);
                                }
                            } else {
                                menu.hovered_option = Some(0);
                            }

                            if let Some(on_option_hovered) = &mut self.on_option_hovered
                                && let Some(option) = menu
                                    .hovered_option
                                    .and_then(|index| state.filtered_options.options.get(index))
                            {
                                // Notify the selection
                                shell.publish((on_option_hovered)(option.clone()));
                                published_message_to_shell = true;
                            }

                            shell.capture_event();
                            shell.request_redraw();
                        }
                        (key::Named::ArrowDown, _) | (key::Named::Tab, false)
                            if !modifiers.shift() =>
                        {
                            if let Some(index) = &mut menu.hovered_option {
                                if *index >= state.filtered_options.options.len().saturating_sub(1)
                                {
                                    *index = 0;
                                } else {
                                    *index = index.saturating_add(1).min(
                                        state.filtered_options.options.len().saturating_sub(1),
                                    );
                                }
                            } else {
                                menu.hovered_option = Some(0);
                            }

                            if let Some(on_option_hovered) = &mut self.on_option_hovered
                                && let Some(option) = menu
                                    .hovered_option
                                    .and_then(|index| state.filtered_options.options.get(index))
                            {
                                // Notify the selection
                                shell.publish((on_option_hovered)(option.clone()));
                                published_message_to_shell = true;
                            }

                            shell.capture_event();
                            shell.request_redraw();
                        }
                        _ => {}
                    }
                }
            });
        }

        // If the overlay menu has selected something
        self.state.with_inner_mut(|state| {
            if let Some(selection) = menu.new_selection.take() {
                // Clear the value and reset the options and menu
                state.value = String::new();
                state.filtered_options.update(self.state.options.clone());
                menu.menu = menu::State::default();

                // Notify the selection
                shell.publish((self.on_selected)(selection));
                published_message_to_shell = true;

                // Unfocus the input
                let mut local_messages = std::mem::take(&mut menu.input_events);
                local_messages.clear();
                let mut local_shell = shell.local(&mut local_messages);
                self.text_input.update(
                    &mut tree.children[0],
                    &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                    layout,
                    mouse::Cursor::Unavailable,
                    renderer,
                    &mut local_shell,
                    viewport,
                );
                shell.request_input_method(local_shell.input_method());
                drop(local_shell);
                menu.input_events = local_messages;
            }
        });

        let is_focused = {
            let text_input_state = tree.children[0]
                .state
                .downcast_ref::<text_input::State<Renderer::Paragraph>>();

            text_input_state.is_focused()
        };

        if started_focused != is_focused {
            // Focus changed, invalidate widget tree to force a fresh `view`
            shell.invalidate_widgets();

            if !published_message_to_shell {
                if is_focused {
                    if let Some(on_open) = self.on_open.take() {
                        shell.publish(on_open);
                    }
                } else if let Some(on_close) = self.on_close.take() {
                    shell.publish(on_close);
                }
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.text_input
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let is_focused = {
            let text_input_state = tree.children[0]
                .state
                .downcast_ref::<text_input::State<Renderer::Paragraph>>();

            text_input_state.is_focused()
        };

        let selection = if is_focused || self.selection.is_empty() {
            None
        } else {
            Some(&self.selection)
        };

        self.text_input.draw(
            &tree.children[0],
            renderer,
            theme,
            layout,
            cursor,
            selection,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let is_focused = {
            let text_input_state = tree.children[0]
                .state
                .downcast_ref::<text_input::State<Renderer::Paragraph>>();

            text_input_state.is_focused()
        };

        if is_focused {
            let Menu {
                menu,
                filtered_options,
                hovered_option,
                ..
            } = tree.state.downcast_mut::<Menu<T>>();

            self.state.sync_filtered_options(filtered_options);

            if filtered_options.options.is_empty() {
                None
            } else {
                let bounds = layout.bounds();

                let mut menu = menu::Menu::new(
                    menu,
                    &filtered_options.options,
                    hovered_option,
                    &T::to_string,
                    |selection| {
                        self.state.with_inner_mut(|state| {
                            state.value = String::new();
                            state.filtered_options.update(self.state.options.clone());
                        });

                        tree.children[0]
                            .state
                            .downcast_mut::<text_input::State<Renderer::Paragraph>>()
                            .unfocus();

                        (self.on_selected)(selection)
                    },
                    self.on_option_hovered.as_deref(),
                    &self.menu_class,
                )
                .width(bounds.width)
                .padding(self.padding)
                .shaping(self.shaping)
                .ellipsis(self.ellipsis);

                if let Some(font) = self.font {
                    menu = menu.font(font);
                }

                if let Some(size) = self.size {
                    menu = menu.text_size(size);
                }

                Some(menu.overlay(
                    layout.position() + translation,
                    *viewport,
                    bounds.height,
                    self.menu_height,
                ))
            }
        } else {
            None
        }
    }
}
