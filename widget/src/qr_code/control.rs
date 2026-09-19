use super::*;

/// A styled matrix barcode view.
pub struct QRCode<'a, Theme = crate::Theme>
where
    Theme: Catalog,
{
    pub(super) data: &'a Data,
    pub(super) cell_size: f32,
    pub(super) class: Theme::Class<'a>,
}

impl<'a, Theme> QRCode<'a, Theme>
where
    Theme: Catalog,
{
    /// Creates a QR-code view from encoded data.
    pub fn new(data: &'a Data) -> Self {
        Self {
            data,
            cell_size: DEFAULT_CELL_SIZE,
            class: Theme::default(),
        }
    }

    /// Sets the side length of an individual module.
    pub fn cell_size(mut self, cell_size: impl Into<Pixels>) -> Self {
        let size = cell_size.into().0;

        if size.is_finite() && size > 0.0 {
            self.cell_size = size;
        }

        self
    }

    /// Sets the total side length, including the quiet zone.
    pub fn total_size(mut self, total_size: impl Into<Pixels>) -> Self {
        let size = total_size.into().0;

        if size.is_finite() && size > 0.0 {
            self.cell_size = size / (self.data.width + 2 * QUIET_ZONE) as f32;
        }

        self
    }

    /// Sets a style function.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets a theme style class.
    #[cfg(feature = "advanced")]
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }
}
