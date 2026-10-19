use super::*;
use crate::pane_grid::controls::Presentation;

impl<Message, Theme, Renderer> TitleBar<'_, Message, Theme, Renderer>
where
    Theme: container::Catalog,
    Renderer: core::Renderer,
{
    pub(in crate::pane_grid) fn state(&self) -> Tree {
        let children = match self.controls.as_ref() {
            Some(controls) => match controls.compact.as_ref() {
                Some(compact) => vec![
                    Tree::new(&self.content),
                    Tree::new(&controls.full),
                    Tree::new(compact),
                ],
                None => vec![
                    Tree::new(&self.content),
                    Tree::new(&controls.full),
                    Tree::empty(),
                ],
            },
            None => {
                vec![Tree::new(&self.content), Tree::empty(), Tree::empty()]
            }
        };

        Tree::with_children(children)
    }

    pub(in crate::pane_grid) fn diff(&mut self, tree: &mut Tree) {
        if tree.children.len() != 3 {
            *tree = self.state();
        }

        if let Some(controls) = &mut self.controls {
            if let Some(compact) = &mut controls.compact {
                tree.children[2].diff(compact);
            } else {
                release_retained_tree(&mut tree.children[2]);
            }

            tree.children[1].diff(&mut controls.full);
        } else {
            release_retained_tree(&mut tree.children[1]);
            release_retained_tree(&mut tree.children[2]);
        }

        tree.children[0].diff(&mut self.content);

        if tree
            .children
            .iter()
            .any(|child| child.dirty().contains(widget::tree::DirtyFlags::LAYOUT))
        {
            tree.mark_dirty(widget::tree::DirtyFlags::LAYOUT);
        }
    }

    /// Draws the [`TitleBar`] with the provided [`Renderer`] and [`Layout`].
    ///
    /// [`Renderer`]: core::Renderer
    pub fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        inherited_style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        show_controls: bool,
    ) {
        let bounds = layout.bounds();
        let style = theme.style(&self.class);

        let inherited_style = renderer::Style {
            text_color: style.text_color.unwrap_or(inherited_style.text_color),
        };

        container::draw_background(renderer, &style, bounds);

        let mut children = layout.children();
        let padded = children.next().unwrap();

        let mut children = padded.children();
        let title_layout = children.next().unwrap();
        let mut show_title = true;

        if let Some(controls) = &self.controls
            && (show_controls || self.always_show_controls)
        {
            let controls_layout = children.next().unwrap();
            if controls.presentation(
                title_layout.bounds().width,
                controls_layout.bounds().width,
                padded.bounds().width,
            ) != Presentation::FullWithTitle
            {
                if let Some(compact) = controls.compact.as_ref() {
                    let compact_layout = children.next().unwrap();

                    compact.as_widget().draw(
                        &tree.children[2],
                        renderer,
                        theme,
                        &inherited_style,
                        compact_layout,
                        cursor,
                        viewport,
                    );
                } else {
                    show_title = false;

                    controls.full.as_widget().draw(
                        &tree.children[1],
                        renderer,
                        theme,
                        &inherited_style,
                        controls_layout,
                        cursor,
                        viewport,
                    );
                }
            } else {
                controls.full.as_widget().draw(
                    &tree.children[1],
                    renderer,
                    theme,
                    &inherited_style,
                    controls_layout,
                    cursor,
                    viewport,
                );
            }
        }

        if show_title {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                &inherited_style,
                title_layout,
                cursor,
                viewport,
            );
        }
    }

    /// Returns whether the mouse cursor is over the pick area of the
    /// [`TitleBar`] or not.
    ///
    /// The whole [`TitleBar`] is a pick area, except its controls.
    pub fn is_over_pick_area(&self, layout: Layout<'_>, cursor_position: Point) -> bool {
        if layout.bounds().contains(cursor_position) {
            let mut children = layout.children();
            let padded = children.next().unwrap();
            let mut children = padded.children();
            let title_layout = children.next().unwrap();

            if let Some(controls) = self.controls.as_ref() {
                let controls_layout = children.next().unwrap();

                if controls.presentation(
                title_layout.bounds().width,
                controls_layout.bounds().width,
                padded.bounds().width,
            ) != Presentation::FullWithTitle
                {
                    if controls.compact.is_some() {
                        let compact_layout = children.next().unwrap();

                        !compact_layout.bounds().contains(cursor_position)
                            && !title_layout.bounds().contains(cursor_position)
                    } else {
                        !controls_layout.bounds().contains(cursor_position)
                    }
                } else {
                    !controls_layout.bounds().contains(cursor_position)
                        && !title_layout.bounds().contains(cursor_position)
                }
            } else {
                !title_layout.bounds().contains(cursor_position)
            }
        } else {
            false
        }
    }

    pub(crate) fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits.shrink(self.padding);
        let max_size = limits.max();
        let child_limits = layout::Limits::new(Size::ZERO, max_size);

        let title_layout = tree.children[0].layout_with_cache(child_limits, |state| {
            self.content
                .as_widget_mut()
                .layout(state, renderer, &child_limits)
        });

        let title_size = title_layout.size();

        let node = if let Some(controls) = &mut self.controls {
            let controls_layout = tree.children[1].layout_with_cache(child_limits, |state| {
                controls
                    .full
                    .as_widget_mut()
                    .layout(state, renderer, &child_limits)
            });

            if controls.presentation(
                title_layout.bounds().width,
                controls_layout.bounds().width,
                max_size.width,
            ) != Presentation::FullWithTitle {
                if let Some(compact) = controls.compact.as_mut() {
                    let compact_layout =
                        tree.children[2].layout_with_cache(child_limits, |state| {
                            compact
                                .as_widget_mut()
                                .layout(state, renderer, &child_limits)
                        });

                    let compact_size = compact_layout.size();
                    let space_before_controls = max_size.width - compact_size.width;

                    let height = title_size.height.max(compact_size.height);

                    layout::Node::with_children(
                        Size::new(max_size.width, height),
                        vec![
                            title_layout,
                            controls_layout,
                            compact_layout.move_to(Point::new(space_before_controls, 0.0)),
                        ],
                    )
                } else {
                    let controls_size = controls_layout.size();
                    let space_before_controls = max_size.width - controls_size.width;

                    let height = title_size.height.max(controls_size.height);

                    layout::Node::with_children(
                        Size::new(max_size.width, height),
                        vec![
                            title_layout,
                            controls_layout.move_to(Point::new(space_before_controls, 0.0)),
                        ],
                    )
                }
            } else {
                let controls_size = controls_layout.size();
                let space_before_controls = max_size.width - controls_size.width;

                let height = title_size.height.max(controls_size.height);

                layout::Node::with_children(
                    Size::new(max_size.width, height),
                    vec![
                        title_layout,
                        controls_layout.move_to(Point::new(space_before_controls, 0.0)),
                    ],
                )
            }
        } else {
            layout::Node::with_children(
                Size::new(max_size.width, title_size.height),
                vec![title_layout],
            )
        };

        layout::Node::container(node, self.padding)
    }

    /// Mirrors [`TitleBar::operate`], including its collapse rule: when
    /// the title and controls do not both fit, only the one actually
    /// shown is visited. Reporting a hidden title to a screen reader
    /// would announce something that is not on screen.
    pub(crate) fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        let mut children = layout.children();
        let padded = children.next().unwrap();

        let mut children = padded.children();
        let title_layout = children.next().unwrap();
        let mut show_title = true;

        if let Some(controls) = &mut self.controls {
            let controls_layout = children.next().unwrap();

            if controls.presentation(
                title_layout.bounds().width,
                controls_layout.bounds().width,
                padded.bounds().width,
            ) != Presentation::FullWithTitle
            {
                if let Some(compact) = controls.compact.as_mut() {
                    let compact_layout = children.next().unwrap();

                    visit(&mut visit::Widgets::new(
                        compact.as_widget_mut(),
                        &mut tree.children[2],
                        compact_layout,
                    ));
                } else {
                    show_title = false;

                    visit(&mut visit::Widgets::new(
                        controls.full.as_widget_mut(),
                        &mut tree.children[1],
                        controls_layout,
                    ));
                }
            } else {
                visit(&mut visit::Widgets::new(
                    controls.full.as_widget_mut(),
                    &mut tree.children[1],
                    controls_layout,
                ));
            }
        };

        if show_title {
            visit(&mut visit::Widgets::new(
                self.content.as_widget_mut(),
                &mut tree.children[0],
                title_layout,
            ));
        }
    }

    pub(crate) fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let mut children = layout.children();
        let padded = children.next().unwrap();

        let mut children = padded.children();
        let title_layout = children.next().unwrap();
        let mut show_title = true;

        if let Some(controls) = &mut self.controls {
            let controls_layout = children.next().unwrap();

            if controls.presentation(
                title_layout.bounds().width,
                controls_layout.bounds().width,
                padded.bounds().width,
            ) != Presentation::FullWithTitle
            {
                if let Some(compact) = controls.compact.as_mut() {
                    let compact_layout = children.next().unwrap();

                    compact.as_widget_mut().operate(
                        &mut tree.children[2],
                        compact_layout,
                        renderer,
                        operation,
                    );
                } else {
                    show_title = false;

                    controls.full.as_widget_mut().operate(
                        &mut tree.children[1],
                        controls_layout,
                        renderer,
                        operation,
                    );
                }
            } else {
                controls.full.as_widget_mut().operate(
                    &mut tree.children[1],
                    controls_layout,
                    renderer,
                    operation,
                );
            }
        };

        if show_title {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                title_layout,
                renderer,
                operation,
            );
        }
    }

    pub(crate) fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let mut children = layout.children();
        let padded = children.next().unwrap();

        let mut children = padded.children();
        let title_layout = children.next().unwrap();
        let mut show_title = true;

        if let Some(controls) = &mut self.controls {
            let controls_layout = children.next().unwrap();

            if controls.presentation(
                title_layout.bounds().width,
                controls_layout.bounds().width,
                padded.bounds().width,
            ) != Presentation::FullWithTitle
            {
                if let Some(compact) = controls.compact.as_mut() {
                    let compact_layout = children.next().unwrap();

                    compact.as_widget_mut().update(
                        &mut tree.children[2],
                        event,
                        compact_layout,
                        cursor,
                        renderer,
                        shell,
                        viewport,
                    );
                } else {
                    show_title = false;

                    controls.full.as_widget_mut().update(
                        &mut tree.children[1],
                        event,
                        controls_layout,
                        cursor,
                        renderer,
                        shell,
                        viewport,
                    );
                }
            } else {
                controls.full.as_widget_mut().update(
                    &mut tree.children[1],
                    event,
                    controls_layout,
                    cursor,
                    renderer,
                    shell,
                    viewport,
                );
            }
        }

        if show_title {
            self.content.as_widget_mut().update(
                &mut tree.children[0],
                event,
                title_layout,
                cursor,
                renderer,
                shell,
                viewport,
            );
        }

        if shell.is_layout_invalid().is_some() {
            for child in &mut tree.children {
                child.mark_dirty(widget::tree::DirtyFlags::LAYOUT);
            }
        }
    }

    pub(crate) fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let mut children = layout.children();
        let padded = children.next().unwrap();

        let mut children = padded.children();
        let title_layout = children.next().unwrap();

        let title_interaction = self.content.as_widget().mouse_interaction(
            &tree.children[0],
            title_layout,
            cursor,
            viewport,
            renderer,
        );

        if let Some(controls) = &self.controls {
            let controls_layout = children.next().unwrap();
            let controls_interaction = controls.full.as_widget().mouse_interaction(
                &tree.children[1],
                controls_layout,
                cursor,
                viewport,
                renderer,
            );

            if controls.presentation(
                title_layout.bounds().width,
                controls_layout.bounds().width,
                padded.bounds().width,
            ) != Presentation::FullWithTitle
            {
                if let Some(compact) = controls.compact.as_ref() {
                    let compact_layout = children.next().unwrap();
                    let compact_interaction = compact.as_widget().mouse_interaction(
                        &tree.children[2],
                        compact_layout,
                        cursor,
                        viewport,
                        renderer,
                    );

                    compact_interaction.max(title_interaction)
                } else {
                    controls_interaction
                }
            } else {
                controls_interaction.max(title_interaction)
            }
        } else {
            title_interaction
        }
    }

    pub(crate) fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let mut children = layout.children();
        let padded = children.next()?;

        let mut children = padded.children();
        let title_layout = children.next()?;

        let Self {
            content, controls, ..
        } = self;

        let mut states = tree.children.iter_mut();
        let title_state = states.next().unwrap();
        let controls_state = states.next().unwrap();

        content
            .as_widget_mut()
            .overlay(title_state, title_layout, renderer, viewport, translation)
            .or_else(move || {
                controls.as_mut().and_then(|controls| {
                    let controls_layout = children.next()?;

                    if controls.presentation(
                title_layout.bounds().width,
                controls_layout.bounds().width,
                padded.bounds().width,
            ) != Presentation::FullWithTitle
                    {
                        if let Some(compact) = controls.compact.as_mut() {
                            let compact_state = states.next().unwrap();
                            let compact_layout = children.next()?;

                            compact.as_widget_mut().overlay(
                                compact_state,
                                compact_layout,
                                renderer,
                                viewport,
                                translation,
                            )
                        } else {
                            controls.full.as_widget_mut().overlay(
                                controls_state,
                                controls_layout,
                                renderer,
                                viewport,
                                translation,
                            )
                        }
                    } else {
                        controls.full.as_widget_mut().overlay(
                            controls_state,
                            controls_layout,
                            renderer,
                            viewport,
                            translation,
                        )
                    }
                })
            })
    }
}

fn release_retained_tree(tree: &mut Tree) {
    if matches!(tree.state, widget::tree::State::Some(_)) || !tree.children.is_empty() {
        *tree = Tree::empty();
    }
}
