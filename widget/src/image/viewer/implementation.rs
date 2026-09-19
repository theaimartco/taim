use super::control::Viewer;
use super::state::{State, fitted_size, pan_limit};
use crate::core::border;
use crate::core::image;
use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::widget::tree::{self, Tree};
use crate::core::{
    ContentFit, Element, Event, Image, Layout, Length, Padding, Radians, Rectangle, Shell, Size,
    Vector, Widget,
};

impl<Message, Theme, Renderer, Handle> Widget<Message, Theme, Renderer> for Viewer<Handle>
where
    Renderer: image::Renderer<Handle = Handle>,
    Handle: Clone,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new())
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_mut::<State>();
        let measured = renderer.measure_image(&self.handle).unwrap_or_default();
        state.image_size = Size::new(measured.width as f32, measured.height as f32);

        layout::padded(
            limits,
            self.width,
            self.height,
            Padding::new(self.padding),
            |limits| {
                let raw_size = limits.resolve(self.width, self.height, state.image_size);
                let full_size = self.content_fit.fit(state.image_size, raw_size);

                layout::Node::new(Size {
                    width: match self.width {
                        Length::Shrink => raw_size.width.min(full_size.width),
                        _ => raw_size.width,
                    },
                    height: match self.height {
                        Length::Shrink => raw_size.height.min(full_size.height),
                        _ => raw_size.height,
                    },
                })
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
        let bounds = content_bounds(layout);

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let Some(cursor_position) = cursor.position_over(bounds) else {
                    return;
                };
                let y = match *delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => y,
                };
                let state = tree.state.downcast_mut::<State>();
                let previous_scale = state.scale;

                if y < 0.0 && previous_scale > self.min_scale
                    || y > 0.0 && previous_scale < self.max_scale
                {
                    state.scale = (if y > 0.0 {
                        previous_scale * (1.0 + self.scale_step)
                    } else {
                        previous_scale / (1.0 + self.scale_step)
                    })
                    .clamp(self.min_scale, self.max_scale);

                    let scaled_size = fitted_size(
                        state.image_size,
                        state.scale,
                        bounds.size(),
                        self.content_fit,
                    );
                    let factor = state.scale / previous_scale - 1.0;
                    let cursor_to_center = cursor_position - bounds.center();
                    let adjustment = cursor_to_center * factor + state.current_offset * factor;

                    state.current_offset = Vector::new(
                        if scaled_size.width > bounds.width {
                            state.current_offset.x + adjustment.x
                        } else {
                            0.0
                        },
                        if scaled_size.height > bounds.height {
                            state.current_offset.y + adjustment.y
                        } else {
                            0.0
                        },
                    );

                    shell.request_redraw();
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor.position_over(bounds) else {
                    return;
                };
                let state = tree.state.downcast_mut::<State>();
                state.cursor_grabbed_at = Some(position);
                state.starting_offset = state.current_offset;
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                tree.state.downcast_mut::<State>().cursor_grabbed_at = None;
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let state = tree.state.downcast_mut::<State>();

                if let Some(origin) = state.cursor_grabbed_at {
                    let scaled_size = fitted_size(
                        state.image_size,
                        state.scale,
                        bounds.size(),
                        self.content_fit,
                    );
                    let limit = pan_limit(bounds.size(), scaled_size);
                    let delta = *position - origin;

                    state.current_offset = Vector::new(
                        if bounds.width < scaled_size.width {
                            (state.starting_offset.x - delta.x).clamp(-limit.x, limit.x)
                        } else {
                            0.0
                        },
                        if bounds.height < scaled_size.height {
                            (state.starting_offset.y - delta.y).clamp(-limit.y, limit.y)
                        } else {
                            0.0
                        },
                    );
                    shell.request_redraw();
                    shell.capture_event();
                }
            }
            _ => {}
        }
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

        if state.is_cursor_grabbed() {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(content_bounds(layout)) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = content_bounds(layout);

        if !bounds.intersects(viewport) {
            return;
        }

        let final_size = fitted_size(
            state.image_size,
            state.scale,
            bounds.size(),
            self.content_fit,
        );
        let diff = bounds.size() - final_size;
        let image_top_left = match self.content_fit {
            ContentFit::None => Vector::new(diff.width.max(0.0) / 2.0, diff.height.max(0.0) / 2.0),
            _ => Vector::new(diff.width / 2.0, diff.height / 2.0),
        };
        let translation = image_top_left - state.offset(bounds, final_size);
        let drawing_bounds = Rectangle::new(bounds.position(), final_size);

        renderer.with_layer(bounds, |renderer| {
            renderer.with_translation(translation, |renderer| {
                renderer.draw_image(
                    Image {
                        handle: self.handle.clone(),
                        border_radius: border::Radius::default(),
                        filter_method: self.filter_method,
                        rotation: Radians(0.0),
                        opacity: 1.0,
                    },
                    drawing_bounds,
                    *viewport - translation,
                );
            });
        });
    }
}

impl<'a, Message, Theme, Renderer, Handle> From<Viewer<Handle>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: 'a + image::Renderer<Handle = Handle>,
    Message: 'a,
    Handle: Clone + 'a,
{
    fn from(viewer: Viewer<Handle>) -> Self {
        Self::new(viewer)
    }
}

fn content_bounds(layout: Layout<'_>) -> Rectangle {
    layout
        .children()
        .next()
        .map_or_else(|| layout.bounds(), |content| content.bounds())
}
