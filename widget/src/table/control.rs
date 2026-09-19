use super::*;

/// Builds a table from column descriptions and row values.
pub fn table<'a, 'b, T, Message, Theme, Renderer>(
    columns: impl IntoIterator<Item = Column<'a, 'b, T, Message, Theme, Renderer>>,
    rows: impl IntoIterator<Item = T>,
) -> Table<'a, Message, Theme, Renderer>
where
    T: Clone,
    Theme: Catalog,
    Renderer: core::Renderer,
{
    Table::new(columns, rows)
}

/// Builds a column from a header and row-to-cell view function.
pub fn column<'a, 'b, T, E, Message, Theme, Renderer>(
    header: impl Into<Element<'a, Message, Theme, Renderer>>,
    view: impl Fn(T) -> E + 'b,
) -> Column<'a, 'b, T, Message, Theme, Renderer>
where
    T: 'a,
    E: Into<Element<'a, Message, Theme, Renderer>>,
{
    Column {
        header: header.into(),
        view: Box::new(move |data| view(data).into()),
        width: Length::Shrink,
        align_x: alignment::Horizontal::Left,
        align_y: alignment::Vertical::Top,
    }
}

/// A grid-like representation of rows distributed across columns.
pub struct Table<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Theme: Catalog,
{
    pub(super) columns: Vec<ColumnMeta>,
    pub(super) cells: Vec<Element<'a, Message, Theme, Renderer>>,
    pub(super) width: Length,
    pub(super) height: Length,
    pub(super) padding_x: f32,
    pub(super) padding_y: f32,
    pub(super) separator_x: f32,
    pub(super) separator_y: f32,
    pub(super) class: Theme::Class<'a>,
}

pub(super) struct ColumnMeta {
    pub(super) width: Length,
    pub(super) align_x: alignment::Horizontal,
    pub(super) align_y: alignment::Vertical,
}

impl<'a, Message, Theme, Renderer> Table<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: core::Renderer,
{
    /// Creates a table and materializes its cells once.
    pub fn new<'b, T>(
        columns: impl IntoIterator<Item = Column<'a, 'b, T, Message, Theme, Renderer>>,
        rows: impl IntoIterator<Item = T>,
    ) -> Self
    where
        T: Clone,
    {
        let columns = columns.into_iter();
        let rows = rows.into_iter();
        let mut width = Length::Fit;
        let capacity = columns
            .size_hint()
            .0
            .saturating_mul(1usize.saturating_add(rows.size_hint().0));
        let mut cells = Vec::with_capacity(capacity);

        let (mut columns, views): (Vec<_>, Vec<_>) = columns
            .map(|column| {
                width = width.stack(column.width);
                cells.push(column.header);

                (
                    ColumnMeta {
                        width: column.width,
                        align_x: column.align_x,
                        align_y: column.align_y,
                    },
                    column.view,
                )
            })
            .collect();

        if width == Length::Shrink
            && let Some(first) = columns.first_mut()
        {
            first.width = Length::Fill;
        }

        for row in rows {
            let mut row = Some(row);

            for (index, view) in views.iter().enumerate() {
                let data = if index + 1 == views.len() {
                    row.take().expect("the final table column consumes the row")
                } else {
                    row.as_ref().expect("the table row is available").clone()
                };

                cells.push(view(data));
            }
        }

        Self {
            columns,
            cells,
            width,
            height: Length::Fit,
            padding_x: 10.0,
            padding_y: 5.0,
            separator_x: 1.0,
            separator_y: 1.0,
            class: Theme::default(),
        }
    }

    /// Sets table width.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets equal horizontal and vertical cell padding.
    pub fn padding(self, padding: impl Into<Pixels>) -> Self {
        let padding = padding.into();
        self.padding_x(padding).padding_y(padding)
    }

    /// Sets horizontal cell padding.
    pub fn padding_x(mut self, padding: impl Into<Pixels>) -> Self {
        self.padding_x = finite_nonnegative(padding.into().0);
        self
    }

    /// Sets vertical cell padding.
    pub fn padding_y(mut self, padding: impl Into<Pixels>) -> Self {
        self.padding_y = finite_nonnegative(padding.into().0);
        self
    }

    /// Sets equal horizontal and vertical separator thickness.
    pub fn separator(self, separator: impl Into<Pixels>) -> Self {
        let separator = separator.into();
        self.separator_x(separator).separator_y(separator)
    }

    /// Sets vertical separator thickness.
    pub fn separator_x(mut self, separator: impl Into<Pixels>) -> Self {
        self.separator_x = finite_nonnegative(separator.into().0);
        self
    }

    /// Sets horizontal separator thickness.
    pub fn separator_y(mut self, separator: impl Into<Pixels>) -> Self {
        self.separator_y = finite_nonnegative(separator.into().0);
        self
    }
}

fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

/// A table column header, sizing policy, and cell view.
pub struct Column<'a, 'b, T, Message, Theme = crate::Theme, Renderer = crate::Renderer> {
    pub(super) header: Element<'a, Message, Theme, Renderer>,
    pub(super) view: Box<dyn Fn(T) -> Element<'a, Message, Theme, Renderer> + 'b>,
    pub(super) width: Length,
    pub(super) align_x: alignment::Horizontal,
    pub(super) align_y: alignment::Vertical,
}

impl<'a, 'b, T, Message, Theme, Renderer> Column<'a, 'b, T, Message, Theme, Renderer> {
    /// Sets column width.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets horizontal cell alignment.
    pub fn align_x(mut self, alignment: impl Into<alignment::Horizontal>) -> Self {
        self.align_x = alignment.into();
        self
    }

    /// Sets vertical cell alignment.
    pub fn align_y(mut self, alignment: impl Into<alignment::Vertical>) -> Self {
        self.align_y = alignment.into();
        self
    }
}
