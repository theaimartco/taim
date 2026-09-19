use super::model::StyledCache;
use super::*;

impl<Message, Theme> Widget<Message, Theme, Renderer> for QRCode<'_, Theme>
where
    Theme: Catalog,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        let side = (self.data.width + 2 * QUIET_ZONE) as f32 * self.cell_size;
        layout::Node::new(Size::new(side, side))
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _defaults: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        if !bounds.intersects(viewport) {
            return;
        }

        let style = theme.style(&self.class);
        let mut caches = self.data.caches.borrow_mut();

        if let Some(index) = caches.iter().position(|entry| entry.style == style) {
            let entry = caches.remove(index);
            caches.push(entry);
        } else {
            if caches.len() == 2 {
                let _ = caches.remove(0);
            }

            caches.push(StyledCache {
                style,
                geometry: canvas::Cache::new(),
            });
        }

        let geometry = caches
            .last()
            .expect("a styled QR cache was inserted")
            .geometry
            .draw(renderer, bounds.size(), |frame| {
                let side = self.data.width + 2 * QUIET_ZONE;
                frame.scale(self.cell_size);
                frame.fill_rectangle(
                    Point::ORIGIN,
                    Size::new(side as f32, side as f32),
                    style.background,
                );
                frame.translate(Vector::new(QUIET_ZONE as f32, QUIET_ZONE as f32));

                for row in 0..self.data.width {
                    let row_start = row * self.data.width;
                    let mut column = 0;

                    while column < self.data.width {
                        while column < self.data.width
                            && self.data.contents[row_start + column] != qrcode::Color::Dark
                        {
                            column += 1;
                        }

                        let run_start = column;

                        while column < self.data.width
                            && self.data.contents[row_start + column] == qrcode::Color::Dark
                        {
                            column += 1;
                        }

                        if run_start < column {
                            frame.fill_rectangle(
                                Point::new(run_start as f32, row as f32),
                                Size::new((column - run_start) as f32, 1.0),
                                style.cell,
                            );
                        }
                    }
                }
            });

        renderer.with_translation(bounds.position() - Point::ORIGIN, |renderer| {
            use crate::graphics::geometry::Renderer as _;
            renderer.draw_geometry(geometry);
        });
    }
}

impl<'a, Message, Theme> From<QRCode<'a, Theme>> for Element<'a, Message, Theme, Renderer>
where
    Theme: Catalog + 'a,
{
    fn from(qr_code: QRCode<'a, Theme>) -> Self {
        Self::new(qr_code)
    }
}
