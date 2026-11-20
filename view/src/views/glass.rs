//! Liquid glass: a surface that lenses, frosts and lights what is behind it,
//! and that a finger presses into like a drop of water.
use std::cell::RefCell;
use std::rc::Rc;

use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Axis, Constraints, Sizing};
use crate::{
    AnyView, Border, Child, Color, Event, Handled, Instant, Padding, Point, Rectangle, Shadow,
    Size, Vector, View, semantics,
};

use taim_core::material::{Liquid, LiquidNeighbour, LiquidTouch, Material};

use super::look::{HOVER, PRESS, RELEASE, Spring};

/// `content` on a pane of liquid glass, in the manner of Apple's interface
/// material, and alive in the same way.
///
/// What is painted behind the pane shows through it frosted and a little
/// more vivid; its rounded edge is thick enough to bend the backdrop like a
/// lens, with a faint rainbow fringe; the scene light catches the rim, and
/// follows the pointer or the tilt of the device; a soft shadow lifts it off
/// the page. A press lights the rim nearest the finger, magnifies what is
/// under it and swells the pane on a spring. Over a backdrop that would make
/// its text hard to read, the glass lets less of it through.
///
/// Put it over something with colour and shape (an image, a gradient, a
/// scrolling list): glass over a flat colour has nothing to show. Inside a
/// [`glass_group`], neighbouring panes melt into one another.
///
/// Without a GPU the pane is drawn as a solid tinted surface, which is also
/// how platforms show glass when "reduce transparency" is on. With Reduce
/// Motion on, a press lights the glass but nothing moves.
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
        sizing: Sizing::HUG,
        state: Rc::new(RefCell::new(PaneState {
            radius: 24.0,
            material: Material::LIQUID,
            tint: None,
            elevation: 1.0,
            response: Response::Light,
            press: Spring::new(0.0, PRESS),
            stretch_x: Spring::new(0.0, RELEASE),
            stretch_y: Spring::new(0.0, RELEASE),
            hover: Spring::new(0.0, HOVER),
            touch: None,
            origin: None,
            hovered: false,
        })),
        grouped: false,
    }
}

/// How a pane answers a press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Response {
    /// Nothing moves or lights.
    Still,
    /// The rim nearest the finger lights, what is under it is magnified, and
    /// the pane swells a little. The default.
    Light,
    /// As [`Light`](Self::Light), and the pane stretches after a dragging
    /// finger and wobbles back when let go: for controls.
    Jelly,
}

/// See [`glass`].
pub struct Glass {
    content: Child,
    padding: Padding,
    sizing: Sizing,
    state: Rc<RefCell<PaneState>>,
    /// Whether a [`glass_group`] paints this pane with its neighbours.
    grouped: bool,
}

impl Glass {
    /// Space between the pane's edge and its content.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// The corner radius. A radius of half the height makes a capsule.
    pub fn radius(self, radius: f32) -> Self {
        self.state.borrow_mut().radius = radius.max(0.0);
        self
    }

    /// Clear liquid glass: almost no frost, so the lensing edge carries the
    /// effect. For small controls over busy content.
    pub fn clear(self) -> Self {
        self.state.borrow_mut().material = Material::LIQUID_CLEAR;
        self
    }

    /// Any material, for surfaces between the presets.
    pub fn material(self, material: Material) -> Self {
        self.state.borrow_mut().material = material;
        self
    }

    /// The colour the glass is tinted towards. By default it follows the
    /// theme: bright glass on a light theme, smoked glass on a dark one.
    pub fn tint(self, tint: impl Into<Color>) -> Self {
        self.state.borrow_mut().tint = Some(tint.into());
        self
    }

    /// How high the pane floats: `0` lies flat, `1` is the default, `2` a
    /// floating panel.
    pub fn elevation(self, elevation: f32) -> Self {
        self.state.borrow_mut().elevation = elevation.clamp(0.0, 4.0);
        self
    }

    /// Makes the pane a control: it also stretches after a dragging finger
    /// and wobbles back into shape when let go.
    pub fn interactive(self) -> Self {
        self.state.borrow_mut().response = Response::Jelly;
        self
    }

    /// A pane that does not answer a press at all.
    pub fn still(self) -> Self {
        self.state.borrow_mut().response = Response::Still;
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
        if self.grouped {
            // The group painted this pane with its neighbours.
            return;
        }
        let bounds = cx.bounds();
        let shape = self.state.borrow().shape(bounds, cx.now());
        paint_shape(cx, &shape, Liquid::default());
    }

    fn intercept(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        // Observed on the way down, never captured: a button on the glass
        // still gets its press, and the glass answers it too.
        if self.state.borrow_mut().observe(cx, event) {
            cx.request_animation();
        }
        Handled::No
    }

    fn cancel_interaction(&mut self) {
        self.state.borrow_mut().release(None, Instant::now());
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.state.borrow().is_moving(now)
    }

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    /// Glass looks through what was painted before it, so it is drawn in a
    /// layer of its own, after that. A group draws its panes in its layer.
    fn is_layer(&self) -> bool {
        !self.grouped
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.content);
    }

    fn as_glass(&mut self) -> Option<&mut Glass> {
        Some(self)
    }
}

/// `content` with every [`glass`] pane in it drawn together, so panes that
/// come within `blend` of each other (16 logical pixels by default) flow
/// into one another like drops of liquid. Move two panes apart and the
/// bridge between them thins and snaps; bring them together and it forms.
///
/// Each pane melts into its two nearest neighbours, which is exact for up to
/// three panes that touch. Do not put scrolling content between the group
/// and its panes: their positions are taken at layout.
///
/// ```ignore
/// glass_group(row(views![
///     glass(icon("play")).radius(22.0).interactive(),
///     glass(icon("next")).radius(22.0).interactive(),
/// ]).gap(8.0))
/// ```
pub fn glass_group(content: impl Into<AnyView>) -> GlassGroup {
    GlassGroup {
        content: Child::new(content.into()),
        blend: 16.0,
        members: Vec::new(),
    }
}

/// See [`glass_group`].
pub struct GlassGroup {
    content: Child,
    blend: f32,
    /// Each pane's state, with its frame relative to the group.
    members: Vec<(Rectangle, Rc<RefCell<PaneState>>)>,
}

impl GlassGroup {
    /// How far apart two panes can be and still bridge, in logical pixels.
    pub fn blend(mut self, blend: f32) -> Self {
        self.blend = blend.max(0.0);
        self
    }
}

impl View for GlassGroup {
    fn sizing(&self) -> Sizing {
        self.content.sizing()
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let size = self.content.measure(cx, constraints);
        self.content.place(Point::ORIGIN);

        self.members.clear();
        collect(&mut self.content, Point::ORIGIN, &mut self.members);

        size
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let origin = cx.bounds().position();
        let now = cx.now();
        let shapes: Vec<Shape> = self
            .members
            .iter()
            .map(|(frame, state)| {
                state.borrow().shape(
                    Rectangle::new(origin + Vector::new(frame.x, frame.y), frame.size()),
                    now,
                )
            })
            .collect();

        for (index, shape) in shapes.iter().enumerate() {
            let liquid = Liquid {
                neighbours: nearest(&shapes, index, self.blend),
                blend: self.blend,
                ..Liquid::default()
            };
            paint_shape(cx, shape, liquid);
        }
    }

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    fn is_layer(&self) -> bool {
        true
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.members
            .iter()
            .any(|(_, state)| state.borrow().is_moving(now))
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.content);
    }
}

/// Finds the glass panes under `child`, marks them grouped and records
/// their frames relative to the group.
fn collect(
    child: &mut Child,
    origin: Point,
    members: &mut Vec<(Rectangle, Rc<RefCell<PaneState>>)>,
) {
    let frame = child.frame();
    let at = origin + Vector::new(frame.x, frame.y);

    if let Some(pane) = child.view_mut().as_glass() {
        pane.grouped = true;
        members.push((Rectangle::new(at, frame.size()), Rc::clone(&pane.state)));
    }

    let scroll = child.view().scroll_offset();
    let inner = at - scroll;
    child
        .view_mut()
        .children(&mut |grandchild| collect(grandchild, inner, members));
}

/// The (at most two) panes nearest `index` within `blend`, nearest first.
fn nearest(shapes: &[Shape], index: usize, blend: f32) -> [Option<LiquidNeighbour>; 2] {
    let own = shapes[index].bounds;
    let mut near: Vec<(f32, &Shape)> = shapes
        .iter()
        .enumerate()
        .filter(|(other, _)| *other != index)
        .map(|(_, shape)| (gap(own, shape.bounds), shape))
        .filter(|(gap, _)| *gap <= blend)
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut neighbours = [None, None];
    for (slot, (_, shape)) in neighbours.iter_mut().zip(near) {
        *slot = Some(LiquidNeighbour {
            bounds: shape.bounds,
            radius: shape.radius,
        });
    }
    neighbours
}

/// The distance between two rectangles' edges; zero when they overlap.
fn gap(a: Rectangle, b: Rectangle) -> f32 {
    let dx = (b.x - (a.x + a.width)).max(a.x - (b.x + b.width)).max(0.0);
    let dy = (b.y - (a.y + a.height))
        .max(a.y - (b.y + b.height))
        .max(0.0);
    dx.hypot(dy)
}

/// A pane as it is at one instant: deformed by its springs and lit by a
/// press.
struct Shape {
    bounds: Rectangle,
    radius: f32,
    material: Material,
    tint: Option<Color>,
    elevation: f32,
    touch: Option<LiquidTouch>,
}

/// What a pane remembers between frames.
struct PaneState {
    radius: f32,
    material: Material,
    tint: Option<Color>,
    elevation: f32,
    response: Response,
    /// `0` at rest, `1` pressed; overshoots a little on the way.
    press: Spring,
    /// How far a dragging finger has pulled the pane, in logical pixels.
    stretch_x: Spring,
    stretch_y: Spring,
    /// `1` while the pointer is over the pane (desktop only).
    hover: Spring,
    /// Where the press or the pointer is, relative to the pane.
    touch: Option<Vector>,
    /// Where the press began, relative to the pane.
    origin: Option<Vector>,
    hovered: bool,
}

impl PaneState {
    fn shape(&self, bounds: Rectangle, now: Instant) -> Shape {
        let press = self.press.value(now);
        let hover = self.hover.value(now);
        let reduced = crate::state::motion_reduced();

        let mut rect = bounds;
        let mut material = self.material;

        if self.response != Response::Still && !reduced {
            // Swell: a few pixels, more for a small control than a panel.
            let side = bounds.width.min(bounds.height);
            let swell = (side * 0.07).min(6.0) * press;
            rect = rect.expand(swell);

            if self.response == Response::Jelly {
                // Stretch after the finger, keeping roughly the same area.
                let pull = Vector::new(self.stretch_x.value(now), self.stretch_y.value(now));
                let reach_x = pull.x.abs();
                let reach_y = pull.y.abs();
                rect = Rectangle::new(
                    Point::new(
                        rect.x + pull.x.min(0.0) + reach_y * 0.25,
                        rect.y + pull.y.min(0.0) + reach_x * 0.25,
                    ),
                    Size::new(
                        (rect.width + reach_x - reach_y * 0.5).max(1.0),
                        (rect.height + reach_y - reach_x * 0.5).max(1.0),
                    ),
                );
            }

            // A pressed pane is a thicker lens.
            material.thickness_mm *= 1.0 + 0.9 * press.max(0.0);
        }

        let strength = if self.response == Response::Still {
            0.0
        } else {
            (press.clamp(0.0, 1.0)).max(hover * 0.35)
        };
        let touch = self
            .touch
            .filter(|_| strength > 0.01)
            .map(|at| LiquidTouch {
                position: bounds.position() + at,
                strength,
                radius: (bounds.width.min(bounds.height) * 0.6).clamp(18.0, 90.0),
            });

        Shape {
            bounds: rect,
            radius: self.radius,
            material,
            tint: self.tint,
            elevation: self.elevation,
            touch,
        }
    }

    /// Follows `event`; returns whether anything changed.
    fn observe(&mut self, cx: &EventCx<'_>, event: &Event) -> bool {
        use super::look::Gesture;

        if self.response == Response::Still {
            return false;
        }
        let now = cx.now();
        let bounds = cx.bounds();
        let local = |at: Point| Vector::new(at.x - bounds.x, at.y - bounds.y);

        // The pointer left the window: no move comes to say it left the pane.
        if matches!(event, Event::Mouse(taim_core::mouse::Event::CursorLeft)) && self.hovered {
            self.hovered = false;
            self.hover.go(0.0, now);
            return true;
        }

        match super::look::gesture(event, cx.pointer()) {
            Some(Gesture::Down(at)) if bounds.contains(at) && self.origin.is_none() => {
                self.origin = Some(local(at));
                self.touch = Some(local(at));
                self.press.go(1.0, now);
                true
            }
            Some(Gesture::Move(at)) if self.origin.is_some() => {
                self.touch = Some(local(at));
                if self.response == Response::Jelly
                    && let Some(origin) = self.origin
                {
                    // Rubber band: the further the drag, the less it gives.
                    let pull = local(at) - origin;
                    let give = |distance: f32| {
                        let limit = 14.0_f32;
                        limit * (distance / limit).tanh() * 0.6
                    };
                    self.stretch_x.go(give(pull.x), now);
                    self.stretch_y.go(give(pull.y), now);
                }
                true
            }
            Some(Gesture::Move(at)) if cx.is_touch() => {
                let _ = at;
                false
            }
            Some(Gesture::Move(at)) => {
                // The pointer over the pane, on a desktop: a faint light
                // follows it.
                let inside = bounds.contains(at);
                let changed = inside != self.hovered;
                if inside {
                    self.touch = Some(local(at));
                }
                if changed {
                    self.hovered = inside;
                    self.hover.go(f32::from(u8::from(inside)), now);
                }
                inside || changed
            }
            Some(Gesture::Up(_)) if self.origin.is_some() => {
                self.release(cx.pointer(), now);
                true
            }
            _ => false,
        }
    }

    /// Lets go: the pane springs back, wobbling if it was stretched.
    fn release(&mut self, _at: Option<Point>, now: Instant) {
        self.origin = None;
        self.press.go(0.0, now);
        self.stretch_x.go(0.0, now);
        self.stretch_y.go(0.0, now);
        if self.hovered {
            self.hovered = false;
            self.hover.go(0.0, now);
        }
    }

    fn is_moving(&self, now: Instant) -> bool {
        self.press.is_moving(now)
            || self.stretch_x.is_moving(now)
            || self.stretch_y.is_moving(now)
            || self.hover.is_moving(now)
    }
}

/// Paints one pane: the tinted, lensing glass, its lit rim, its shadow and
/// the sheen on its upper half.
fn paint_shape(cx: &mut PaintCx<'_>, shape: &Shape, liquid: Liquid) {
    let liquid = Liquid {
        touch: shape.touch,
        ..liquid
    };
    paint_pane(
        cx,
        shape.bounds,
        shape.radius,
        shape.material,
        shape.tint,
        shape.elevation,
        liquid,
    );
}

/// The glass every TAIM surface shares: the tinted, lensing pane, its lit
/// rim, its shadow and the sheen on its upper half. Paint it from a view
/// that [`is_layer`](View::is_layer), before the view's content.
pub(crate) fn pane(
    cx: &mut PaintCx<'_>,
    bounds: Rectangle,
    radius: f32,
    material: Material,
    tint: Option<Color>,
    elevation: f32,
) {
    paint_pane(
        cx,
        bounds,
        radius,
        material,
        tint,
        elevation,
        Liquid::default(),
    );
}

fn paint_pane(
    cx: &mut PaintCx<'_>,
    bounds: Rectangle,
    radius: f32,
    material: Material,
    tint: Option<Color>,
    elevation: f32,
    liquid: Liquid,
) {
    crate::state::note_glass();
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

    cx.fill_liquid(
        bounds,
        tint,
        Border::default().rounded(radius).width(1.0).color(rim),
        shadow,
        material,
        liquid,
    );

    // Sheen: light falling on the upper part of the pane, fading out before
    // the middle, as on Apple's glass. A merged group shows the sheen of
    // each pane, which reads as the light on each drop.
    let sheen = if dark { 0.10 } else { 0.28 };
    cx.fill(
        bounds,
        taim_core::gradient::Linear::new(taim_core::Radians(std::f32::consts::PI))
            .add_stop(0.0, Color::from_rgba(1.0, 1.0, 1.0, sheen))
            .add_stop(0.55, Color::from_rgba(1.0, 1.0, 1.0, 0.0)),
        Border::default().rounded(radius),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_spring_overshoots_then_settles_on_its_target() {
        let start = Instant::now();
        let mut spring = Spring::new(0.0, PRESS);
        spring.go(1.0, start);

        let peak = (1..60)
            .map(|step| spring.value(start + Duration::from_millis(step * 5)))
            .fold(f32::MIN, f32::max);
        assert!(
            peak > 1.0 && peak < 1.2,
            "a press overshoots a little: {peak}"
        );

        let later = start + Duration::from_secs(2);
        assert_eq!(spring.value(later), 1.0, "at rest exactly on target");
        assert!(!spring.is_moving(later));
    }

    #[test]
    fn retargeting_keeps_the_spring_continuous() {
        let start = Instant::now();
        let mut spring = Spring::new(0.0, RELEASE);
        spring.go(10.0, start);
        let midway = start + Duration::from_millis(40);
        let before = spring.value(midway);
        spring.go(0.0, midway);
        assert!((spring.value(midway) - before).abs() < 1.0e-4, "no jump");
    }

    #[test]
    fn panes_bridge_only_within_the_blend_distance() {
        let shape = |x: f32| Shape {
            bounds: Rectangle::new(Point::new(x, 0.0), Size::new(40.0, 40.0)),
            radius: 20.0,
            material: Material::LIQUID,
            tint: None,
            elevation: 1.0,
            touch: None,
        };
        let shapes = [shape(0.0), shape(50.0), shape(200.0)];

        let first = nearest(&shapes, 0, 16.0);
        assert!(first[0].is_some_and(|n| n.bounds.x == 50.0));
        assert!(first[1].is_none(), "the far pane is out of reach");
        assert!(nearest(&shapes, 2, 16.0).iter().all(Option::is_none));
        assert_eq!(gap(shapes[0].bounds, shapes[1].bounds), 10.0);
    }
}
