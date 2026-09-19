use super::*;

impl<T, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for VerticalSlider<'_, T, Message, Theme>
where
    T: Copy + num_traits::AsPrimitive<f64> + num_traits::FromPrimitive,
    Message: Clone,
    Theme: Catalog,
    Renderer: core::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Shrink,
            height: self.height,
        }
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, self.width, self.height)
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
        let state = tree.state.downcast_mut::<State>();
        let is_dragging = state.is_dragging;
        let current_value = self.value;

        let locate = |cursor_position: Point| -> Option<T> {
            let bounds = layout.bounds();

            if cursor_position.y >= bounds.y + bounds.height {
                Some(*self.range.start())
            } else if cursor_position.y <= bounds.y {
                Some(*self.range.end())
            } else {
                let step = normalize_step(
                    if state.keyboard_modifiers.shift() {
                        self.shift_step.unwrap_or(self.step)
                    } else {
                        self.step
                    }
                    .as_(),
                );

                let start = (*self.range.start()).as_();
                let end = (*self.range.end()).as_();

                let percent =
                    1.0 - f64::from(cursor_position.y - bounds.y) / f64::from(bounds.height);

                T::from_f64(snap(start, end, percent, step))
            }
        };

        let increment = |value: T| -> Option<T> {
            let step = normalize_step(
                if state.keyboard_modifiers.shift() {
                    self.shift_step.unwrap_or(self.step)
                } else {
                    self.step
                }
                .as_(),
            );

            T::from_f64(advance(
                (*self.range.start()).as_(),
                (*self.range.end()).as_(),
                value.as_(),
                step,
                1.0,
            ))
        };

        let decrement = |value: T| -> Option<T> {
            let step = normalize_step(
                if state.keyboard_modifiers.shift() {
                    self.shift_step.unwrap_or(self.step)
                } else {
                    self.step
                }
                .as_(),
            );

            T::from_f64(advance(
                (*self.range.start()).as_(),
                (*self.range.end()).as_(),
                value.as_(),
                step,
                -1.0,
            ))
        };

        let change = |new_value: T| {
            if (self.value.as_() - new_value.as_()).abs() > f64::EPSILON {
                shell.publish((self.on_change)(new_value));

                self.value = new_value;
            }
        };

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if let Some(cursor_position) = cursor.position_over(layout.bounds()) {
                    if state.keyboard_modifiers.control() || state.keyboard_modifiers.command() {
                        let _ = self.default.map(change);
                        state.is_dragging = false;
                    } else {
                        let _ = locate(cursor_position).map(change);
                        state.is_dragging = true;
                    }

                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. })
            | Event::Touch(touch::Event::FingerLost { .. })
                if is_dragging =>
            {
                if let Some(on_release) = self.on_release.clone() {
                    shell.publish(on_release);
                }
                state.is_dragging = false;
            }
            Event::Mouse(mouse::Event::CursorMoved { .. })
            | Event::Touch(touch::Event::FingerMoved { .. })
                if is_dragging =>
            {
                let _ = cursor.land().position().and_then(locate).map(change);

                shell.capture_event();
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if state.keyboard_modifiers.control() && cursor.is_over(layout.bounds()) =>
            {
                let delta = match *delta {
                    mouse::ScrollDelta::Lines { x: _, y } => y,
                    mouse::ScrollDelta::Pixels { x: _, y } => y,
                };

                if delta < 0.0 {
                    let _ = decrement(current_value).map(change);
                } else {
                    let _ = increment(current_value).map(change);
                }

                shell.capture_event();
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if cursor.is_over(layout.bounds()) =>
            {
                match key {
                    Key::Named(key::Named::ArrowUp) => {
                        let _ = increment(current_value).map(change);
                        shell.capture_event();
                    }
                    Key::Named(key::Named::ArrowDown) => {
                        let _ = decrement(current_value).map(change);
                        shell.capture_event();
                    }
                    _ => (),
                }
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                state.keyboard_modifiers = *modifiers;
            }
            _ => {}
        }

        let current_status = if state.is_dragging {
            Status::Dragged
        } else if cursor.is_over(layout.bounds()) {
            Status::Hovered
        } else {
            Status::Active
        };

        if let Event::Window(window::Event::RedrawRequested(_now)) = event {
            state.last_status = Some(current_status);
        } else if state
            .last_status
            .is_some_and(|status| status != current_status)
        {
            shell.request_redraw();
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
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        let state = tree.state.downcast_ref::<State>();
        let style = theme.style(&self.class, state.last_status.unwrap_or(Status::Active));

        let (handle_width, handle_height, handle_border_radius) = match style.handle.shape {
            HandleShape::Circle { radius } => (radius * 2.0, radius * 2.0, radius.into()),
            HandleShape::Rectangle {
                width,
                border_radius,
            } => (f32::from(width), bounds.width, border_radius),
        };

        let value = self.value.as_() as f32;
        let (range_start, range_end) = {
            let (start, end) = self.range.clone().into_inner();

            (start.as_() as f32, end.as_() as f32)
        };

        let offset = if range_start >= range_end {
            0.0
        } else {
            (bounds.height - handle_width) * (value - range_end) / (range_start - range_end)
        };

        let rail_x = bounds.x + bounds.width / 2.0;

        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: rail_x - style.rail.width / 2.0,
                    y: bounds.y,
                    width: style.rail.width,
                    height: offset + handle_width / 2.0,
                },
                border: style.rail.border,
                ..renderer::Quad::default()
            },
            style.rail.backgrounds.1,
        );

        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: rail_x - style.rail.width / 2.0,
                    y: bounds.y + offset + handle_width / 2.0,
                    width: style.rail.width,
                    height: bounds.height - offset - handle_width / 2.0,
                },
                border: style.rail.border,
                ..renderer::Quad::default()
            },
            style.rail.backgrounds.0,
        );

        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: rail_x - handle_height / 2.0,
                    y: bounds.y + offset,
                    width: handle_height,
                    height: handle_width,
                },
                border: Border {
                    radius: handle_border_radius,
                    width: style.handle.border_width,
                    color: style.handle.border_color,
                },
                ..renderer::Quad::default()
            },
            style.handle.background,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();

        if state.is_dragging {
            // FIXME: Fall back to `Pointer` on Windows
            // See https://github.com/rust-windowing/winit/issues/1043
            if cfg!(target_os = "windows") {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::Grabbing
            }
        } else if cursor.is_over(layout.bounds()) {
            if cfg!(target_os = "windows") {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::Grab
            }
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a, T, Message, Theme, Renderer> From<VerticalSlider<'a, T, Message, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    T: Copy + num_traits::AsPrimitive<f64> + num_traits::FromPrimitive + 'a,
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: core::Renderer + 'a,
{
    fn from(
        slider: VerticalSlider<'a, T, Message, Theme>,
    ) -> Element<'a, Message, Theme, Renderer> {
        Element::new(slider)
    }
}
