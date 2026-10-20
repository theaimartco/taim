use super::*;

/// Computes the layout of a [`Container`].
pub fn layout(
    limits: &layout::Limits,
    width: Length,
    height: Length,
    padding: Padding,
    horizontal_alignment: alignment::Horizontal,
    vertical_alignment: alignment::Vertical,
    layout_content: impl FnOnce(&layout::Limits) -> layout::Node,
) -> layout::Node {
    layout::positioned(
        limits,
        width,
        height,
        padding,
        |limits| layout_content(&limits.loose()),
        |content, size| {
            content.align(
                Alignment::from(horizontal_alignment),
                Alignment::from(vertical_alignment),
                size,
            )
        },
    )
}

/// Draws the background of a [`Container`] given its [`Style`] and its `bounds`.
pub fn draw_background<Renderer>(renderer: &mut Renderer, style: &Style, bounds: Rectangle)
where
    Renderer: core::Renderer,
{
    // TAIM conservative paint-culling: transparent fills and zero-alpha
    // borders do not require a GPU draw, but non-default materials must
    // still reach the renderer because their shading can be visible.
    if !bounds.width.is_finite()
        || !bounds.height.is_finite()
        || bounds.width <= 0.0
        || bounds.height <= 0.0
    {
        return;
    }

    let visible_fill = style
        .background
        .is_some_and(|background| !background.is_fully_transparent());
    let visible_border = style.border.width > 0.0 && style.border.color.a > 0.0;
    let visible_shadow = style.shadow.color.a > 0.0;

    if visible_fill
        || visible_border
        || visible_shadow
        || style.material != Material::OPAQUE
    {
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: style.border,
                shadow: style.shadow,
                snap: style.snap,
                material: style.material,
                ..renderer::Quad::default()
            },
            style
                .background
                .unwrap_or(Background::Color(Color::TRANSPARENT)),
        );
    }
}
