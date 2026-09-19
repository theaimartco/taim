use super::control::ColumnMeta;
use super::*;

struct Metrics {
    columns: Vec<f32>,
    rows: Vec<f32>,
    column_factors: Vec<u16>,
    row_factors: Vec<u16>,
}

impl Metrics {
    fn new() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            column_factors: Vec::new(),
            row_factors: Vec::new(),
        }
    }

    fn reset(&mut self, columns: usize, rows: usize) {
        self.columns.resize(columns, 0.0);
        self.columns.fill(0.0);
        self.rows.resize(rows, 0.0);
        self.rows.fill(0.0);
        self.column_factors.resize(columns, 0);
        self.column_factors.fill(0);
        self.row_factors.resize(rows, 0);
        self.row_factors.fill(0);
    }

    fn clear(&mut self) {
        self.columns.clear();
        self.rows.clear();
        self.column_factors.clear();
        self.row_factors.clear();
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Table<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: core::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<Metrics>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(Metrics::new())
    }

    fn diff(&mut self, tree: &mut widget::Tree) {
        tree.diff_children(&mut self.cells);
        self.height = Length::Fit;

        for cell in &self.cells {
            self.height = self.height.stack(cell.as_widget().size().height);
        }
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let metrics = tree.state.downcast_mut::<Metrics>();
        let column_count = self.columns.len();
        let limits = limits.width(self.width).height(self.height);

        if column_count == 0 {
            metrics.clear();
            return layout::Node::new(limits.resolve(self.width, self.height, Size::ZERO));
        }

        let row_count = self.cells.len() / column_count;
        let available = limits.max();
        let table_fluid = if self.width.fill_factor() == 0 {
            Length::Shrink
        } else {
            Length::Fill
        };
        let mut cells = vec![layout::Node::default(); self.cells.len()];
        metrics.reset(column_count, row_count);

        let spacing_x = self.padding_x * 2.0 + self.separator_x;
        let spacing_y = self.padding_y * 2.0 + self.separator_y;
        let mut x = self.padding_x;
        let mut y = self.padding_y;

        for (index, (cell, state)) in self.cells.iter_mut().zip(&mut tree.children).enumerate() {
            let row = index / column_count;
            let column = index % column_count;
            let width = self.columns[column].width;
            let size = cell.as_widget().size();

            if column == 0 {
                x = self.padding_x;
                if row > 0 {
                    y += metrics.rows[row - 1] + spacing_y;
                }
            }

            let width_factor = width.fill_factor();
            let height_factor = size.height.fill_factor();

            if width_factor != 0 || height_factor != 0 || size.width.is_fill() {
                metrics.column_factors[column] = metrics.column_factors[column].max(width_factor);
                metrics.row_factors[row] = metrics.row_factors[row].max(height_factor);
                continue;
            }

            let cell_limits = layout::Limits::new(
                Size::ZERO,
                Size::new(
                    (available.width - x).max(0.0),
                    (available.height - y).max(0.0),
                ),
            )
            .width(width);
            let node = cell.as_widget_mut().layout(state, renderer, &cell_limits);
            let size = cell_limits.resolve(width, Length::Shrink, node.size());

            metrics.columns[column] = metrics.columns[column].max(size.width);
            metrics.rows[row] = metrics.rows[row].max(size.height);
            cells[index] = node;
            x += size.width + spacing_x;
        }

        let fixed_row_height = metrics
            .rows
            .iter()
            .zip(&metrics.row_factors)
            .filter(|(_, factor)| **factor == 0)
            .map(|(height, _)| *height)
            .sum::<f32>();
        let total_row_factors = metrics.row_factors.iter().sum::<u16>();
        let left = Size::new(
            available.width
                - metrics
                    .columns
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| metrics.column_factors[*index] == 0)
                    .map(|(_, width)| width)
                    .sum::<f32>(),
            available.height - fixed_row_height,
        );
        let total_column_factors = metrics.column_factors.iter().sum::<u16>();
        let width_unit = if total_column_factors == 0 {
            0.0
        } else {
            (left.width - spacing_x * column_count.saturating_sub(1) as f32 - self.padding_x * 2.0)
                .max(0.0)
                / total_column_factors as f32
        };
        let height_unit = if total_row_factors == 0 {
            0.0
        } else {
            (left.height - spacing_y * row_count.saturating_sub(1) as f32 - self.padding_y * 2.0)
                .max(0.0)
                / total_row_factors as f32
        };

        x = self.padding_x;
        y = self.padding_y;

        for (index, (cell, state)) in self.cells.iter_mut().zip(&mut tree.children).enumerate() {
            let row = index / column_count;
            let column = index % column_count;
            let size = cell.as_widget().size();
            let width = self.columns[column].width;
            let width_factor = width.fill_factor();
            let height_factor = size.height.fill_factor();

            if column == 0 {
                x = self.padding_x;
                if row > 0 {
                    y += metrics.rows[row - 1] + spacing_y;
                }
            }

            if width_factor == 0 && !size.width.is_fill() && height_factor == 0 {
                continue;
            }

            let max_width = if width_factor == 0 {
                if size.width.is_fill() {
                    metrics.columns[column]
                } else {
                    (available.width - x).max(0.0)
                }
            } else {
                width_unit * width_factor as f32
            };
            let max_height = if height_factor == 0 {
                if size.height.is_fill() {
                    metrics.rows[row]
                } else {
                    (available.height - y).max(0.0)
                }
            } else {
                height_unit * height_factor as f32
            };
            let cell_limits =
                layout::Limits::new(Size::ZERO, Size::new(max_width, max_height)).width(width);
            let node = cell.as_widget_mut().layout(state, renderer, &cell_limits);
            let size = cell_limits.resolve(
                if matches!(width, Length::Fixed(_)) {
                    width
                } else {
                    table_fluid
                },
                Length::Shrink,
                node.size(),
            );

            metrics.columns[column] = metrics.columns[column].max(size.width);
            metrics.rows[row] = metrics.rows[row].max(size.height);
            cells[index] = node;
            x += size.width + spacing_x;
        }

        x = self.padding_x;
        y = self.padding_y;

        for (index, cell) in cells.iter_mut().enumerate() {
            let row = index / column_count;
            let column = index % column_count;

            if column == 0 {
                x = self.padding_x;
                if row > 0 {
                    y += metrics.rows[row - 1] + spacing_y;
                }
            }

            let ColumnMeta {
                align_x, align_y, ..
            } = &self.columns[column];
            cell.move_to_mut((x, y));
            cell.align_mut(
                Alignment::from(*align_x),
                Alignment::from(*align_y),
                Size::new(metrics.columns[column], metrics.rows[row]),
            );
            x += metrics.columns[column] + spacing_x;
        }

        let intrinsic = limits.resolve(
            self.width,
            self.height,
            Size::new(
                x - spacing_x + self.padding_x,
                y + metrics
                    .rows
                    .last()
                    .copied()
                    .map(|height| height + self.padding_y)
                    .unwrap_or_default(),
            ),
        );

        layout::Node::with_children(intrinsic, cells)
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &core::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut core::Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        for ((cell, state), layout) in self
            .cells
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            cell.as_widget_mut()
                .update(state, event, layout, cursor, renderer, shell, viewport);
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        if !bounds.intersects(viewport) {
            return;
        }

        for ((cell, state), layout) in self.cells.iter().zip(&tree.children).zip(layout.children())
        {
            if layout.bounds().intersects(viewport) {
                cell.as_widget()
                    .draw(state, renderer, theme, style, layout, cursor, viewport);
            }
        }

        let metrics = tree.state.downcast_ref::<Metrics>();
        let style = theme.style(&self.class);

        if self.separator_x > 0.0 {
            let mut x = self.padding_x;
            for width in &metrics.columns[..metrics.columns.len().saturating_sub(1)] {
                x += width + self.padding_x;
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: bounds.x + x,
                            y: bounds.y,
                            width: self.separator_x,
                            height: bounds.height,
                        },
                        snap: true,
                        ..renderer::Quad::default()
                    },
                    style.separator_x,
                );
                x += self.separator_x + self.padding_x;
            }
        }

        if self.separator_y > 0.0 {
            let mut y = self.padding_y;
            for height in &metrics.rows[..metrics.rows.len().saturating_sub(1)] {
                y += height + self.padding_y;
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: bounds.x,
                            y: bounds.y + y,
                            width: bounds.width,
                            height: self.separator_y,
                        },
                        snap: true,
                        ..renderer::Quad::default()
                    },
                    style.separator_y,
                );
                y += self.separator_y + self.padding_y;
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
        self.cells
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .filter(|(_, layout)| layout.bounds().intersects(viewport))
            .map(|((cell, state), layout)| {
                cell.as_widget()
                    .mouse_interaction(state, layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
    }

    fn visit_children(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        for ((cell, state), layout) in self
            .cells
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            visit(&mut visit::Widgets::new(
                cell.as_widget_mut(),
                state,
                layout,
            ));
        }
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        for ((cell, state), layout) in self
            .cells
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            cell.as_widget_mut()
                .operate(state, layout, renderer, operation);
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: core::Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        overlay::from_children(
            &mut self.cells,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<Table<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: Catalog + 'a,
    Renderer: core::Renderer + 'a,
{
    fn from(table: Table<'a, Message, Theme, Renderer>) -> Self {
        Self::new(table)
    }
}
