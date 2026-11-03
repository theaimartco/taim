//! Liquid glass: a surface that lenses, frosts and lights what is behind it.
use crate::cx::{DescribeCx, MeasureCx, PaintCx};
use crate::layout::{Axis, Constraints, Sizing};
use crate::{AnyView, Border, Child, Color, Padding, Point, Shadow, Size, Vector, View, semantics};

use taim_core::material::Material;

/// `content` on a pane of liquid glass, in the manner of Apple's interface
/// material.
///
/// What is painted behind the pane shows through it frosted and a little
/// more vivid; its rounded edge is thick enough to bend the backdrop like a
/// lens, with a faint rainbow fringe; the scene light catches the rim; a
/// soft shadow lifts it off the page. Put it over something with colour and
/// shape (an image, a gradient, a scrolling list): glass over a flat colour
/// has nothing to show.
///
/// Without a GPU the pane is drawn as a solid tinted surface, which is also
/// how platforms show glass when "reduce transparency" is on.
///
/// ```ignore
/// layers(views![
///     photo,
///     container(glass(text("Now playing")).radius(28.0)).align_bottom().padding(16.0),
/// ])
/// ```
pub fn glass(content: impl Into<AnyView>) -> Glass {
    Glass {
        content: Child::new(content.into()),
        padding: Padding::new(16.0),
        radius: 24.0,
        material: Material::LIQUID,
        tint: None,
        sizing: Sizing::HUG,
        elevation: 1.0,
    }
}

/// See [`glass`].
pub struct Glass {
    content: Child,
    padding: Padding,
    radius: f32,
    material: Material,
    tint: Option<Color>,
    sizing: Sizing,
    /// How far the pane floats above the page, scaling its shadow.
    elevation: f32,
}

impl Glass {
    /// Space between the pane's edge and its content.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// The corner radius. A radius of half the height makes a capsule.
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius.max(0.0);
        self
    }

    /// Clear liquid glass: almost no frost, so the lensing edge carries the
    /// effect. For small controls over busy content.
    pub fn clear(mut self) -> Self {
        self.material = Material::LIQUID_CLEAR;
        self
    }

    /// Any material, for surfaces between the presets.
    pub fn material(mut self, material: Material) -> Self {
        self.material = material;
        self
    }

    /// The colour the glass is tinted towards. By default it follows the
    /// theme: bright glass on a light theme, smoked glass on a dark one.
    pub fn tint(mut self, tint: impl Into<Color>) -> Self {
        self.tint = Some(tint.into());
        self
    }

    /// How high the pane floats: `0` lies flat, `1` is the default, `2` a
    /// floating panel.
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = elevation.clamp(0.0, 4.0);
        self
    }

    /// Takes all the width it is offered.
    pub fn fill_width(mut self) -> Self {
        self.sizing.width = Axis::Fill(1);
        self
    }

    /// Takes all the room it is offered.
    pub fn fill(mut self) -> Self {
        self.sizing = Sizing::FILL;
        self
    }
}

impl View for Glass {
    fn sizing(&self) -> Sizing {
        self.sizing
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let outer = constraints.with_sizing(self.sizing);
        let inner = outer.inset(self.padding);
        let child = self.content.measure(cx, inner.loosen());

        self.content
            .place(Point::new(self.padding.left, self.padding.top));

        Size::new(
            child.width.max(inner.min.width) + self.padding.left + self.padding.right,
            child.height.max(inner.min.height) + self.padding.top + self.padding.bottom,
        )
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        pane(
            cx,
            bounds,
            self.radius,
            self.material,
            self.tint,
            self.elevation,
        );
    }

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    /// Glass looks through what was painted before it, so it is drawn in a
    /// layer of its own, after that.
    fn is_layer(&self) -> bool {
        true
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.content);
    }
}

/// The glass every TAIM surface shares: the tinted, lensing pane, its lit
/// rim, its shadow and the sheen on its upper half. Paint it from a view
/// that [`is_layer`](View::is_layer), before the view's content.
pub(crate) fn pane(
    cx: &mut PaintCx<'_>,
    bounds: crate::Rectangle,
    radius: f32,
    material: Material,
    tint: Option<Color>,
    elevation: f32,
) {
    let radius = radius.min(bounds.width / 2.0).min(bounds.height / 2.0);
    let dark = cx.theme().palette().is_dark;

    // The fill is the tint the glass leans towards; how much of the
    // backdrop shows through it is the material's transmission. It is
    // opaque on purpose: the backdrop already holds what is behind.
    let tint = tint.unwrap_or(if dark {
        Color::from_rgb(0.08, 0.10, 0.16)
    } else {
        Color::WHITE
    });

    // A hairline lit edge, brighter on light glass.
    let rim = if dark {
        Color::from_rgba(1.0, 1.0, 1.0, 0.14)
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.55)
    };

    let shadow = if elevation > 0.0 {
        Shadow {
            color: Color::from_rgba(
                0.0,
                0.0,
                0.0,
                if dark { 0.45 } else { 0.18 } * elevation.min(1.5),
            ),
            offset: Vector::new(0.0, 6.0 * elevation),
            blur_radius: 22.0 * elevation,
        }
    } else {
        Shadow::default()
    };

    // Smoked glass needs more of its tint to read as dark.
    let mut material = material;
    if dark {
        material.transmission *= 0.8;
    }

    cx.fill_material(
        bounds,
        tint,
        Border::default().rounded(radius).width(1.0).color(rim),
        shadow,
        material,
    );

    // Sheen: light falling on the upper part of the pane, fading out before
    // the middle, as on Apple's glass.
    let sheen = if dark { 0.10 } else { 0.28 };
    cx.fill(
        bounds,
        taim_core::gradient::Linear::new(taim_core::Radians(std::f32::consts::PI))
            .add_stop(0.0, Color::from_rgba(1.0, 1.0, 1.0, sheen))
            .add_stop(0.55, Color::from_rgba(1.0, 1.0, 1.0, 0.0)),
        Border::default().rounded(radius),
    );
}
