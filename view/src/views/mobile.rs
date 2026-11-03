//! Views for phone-style layouts.
use super::look::{self, Label, Tween};
use super::{button, column, container, layers, row, text, text_field, when};
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Axis, Constraints, Sizing};
use crate::state::{IntoProp, Prop, State};
use crate::{AnyView, Border, Child, Color, Event, Handled, Instant, Point, Rectangle, Size, View};
use crate::{mouse, semantics};

use taim_core::widget;

/// A row of equally wide choices bound to `selected`, with an underline
/// that slides to the chosen one.
pub fn tabs<S: Into<String>>(
    labels: impl IntoIterator<Item = S>,
    selected: State<usize>,
) -> Segments {
    Segments::new(
        Look::Underline,
        labels.into_iter().map(|label| (None, label.into())),
        selected,
    )
}

/// A bottom navigation bar: each item is a glyph over a label, bound to
/// `selected`.
pub fn nav_bar<G: Into<String>, L: Into<String>>(
    items: impl IntoIterator<Item = (G, L)>,
    selected: State<usize>,
) -> Segments {
    Segments::new(
        Look::Pill,
        items
            .into_iter()
            .map(|(glyph, label)| (Some(glyph.into()), label.into())),
        selected,
    )
}

/// A segmented control: a row of joined buttons, one of them chosen,
/// bound to `selected`.
pub fn segmented<S: Into<String>>(
    labels: impl IntoIterator<Item = S>,
    selected: State<usize>,
) -> Segments {
    Segments::new(
        Look::Boxed,
        labels.into_iter().map(|label| (None, label.into())),
        selected,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Look {
    Underline,
    Pill,
    Boxed,
}

struct Segment {
    glyph: Option<Label>,
    label: Label,
}

/// See [`tabs`] and [`nav_bar`].
pub struct Segments {
    look: Look,
    segments: Vec<Segment>,
    selected: State<usize>,
    shown: Prop<usize>,
    indicator: Tween,
    pressed: Option<usize>,
    id: widget::Id,
}

impl Segments {
    fn new(
        look: Look,
        items: impl Iterator<Item = (Option<String>, String)>,
        selected: State<usize>,
    ) -> Self {
        let (glyph_size, label_size) = match look {
            Look::Underline => (None, Some(14.0)),
            Look::Pill => (Some(20.0), Some(11.0)),
            Look::Boxed => (None, Some(13.0)),
        };

        Self {
            look,
            segments: items
                .map(|(glyph, label)| Segment {
                    glyph: glyph.map(|glyph| Label::new(glyph, glyph_size)),
                    label: Label::new(label, label_size),
                })
                .collect(),
            selected,
            shown: selected.into_prop(),
            indicator: Tween::new(selected.get() as f32),
            pressed: None,
            id: widget::Id::unique(),
        }
    }

    /// Where the segments sit: the whole bounds, except for the floating
    /// navigation capsule, which keeps clear of the screen's edges.
    fn track(&self, bounds: Rectangle) -> Rectangle {
        match self.look {
            Look::Pill => Rectangle::new(
                Point::new(bounds.x + CAPSULE_MARGIN.0, bounds.y + CAPSULE_MARGIN.1),
                Size::new(
                    (bounds.width - 2.0 * CAPSULE_MARGIN.0).max(0.0),
                    (bounds.height - CAPSULE_MARGIN.1 - CAPSULE_MARGIN.2).max(0.0),
                ),
            ),
            _ => bounds,
        }
    }

    fn slot(&self, bounds: Rectangle, index: f32) -> Rectangle {
        let bounds = self.track(bounds);
        let width = bounds.width / self.segments.len().max(1) as f32;
        Rectangle::new(
            Point::new(bounds.x + width * index, bounds.y),
            Size::new(width, bounds.height),
        )
    }

    fn index_at(&self, bounds: Rectangle, at: Point) -> Option<usize> {
        let bounds = self.track(bounds);
        if !bounds.contains(at) || self.segments.is_empty() {
            return None;
        }
        let width = bounds.width / self.segments.len() as f32;
        Some((((at.x - bounds.x) / width) as usize).min(self.segments.len() - 1))
    }
}

/// The floating navigation capsule's margin: sides, top, bottom.
const CAPSULE_MARGIN: (f32, f32, f32) = (16.0, 6.0, 10.0);

impl View for Segments {
    fn sizing(&self) -> Sizing {
        Sizing {
            width: Axis::Fill(1),
            height: Axis::Hug,
        }
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let mut tallest: f32 = 0.0;
        for segment in &mut self.segments {
            let glyph = segment
                .glyph
                .as_mut()
                .map_or(0.0, |glyph| glyph.measure(cx).height + 2.0);
            tallest = tallest.max(glyph + segment.label.measure(cx).height);
        }

        let padding = match self.look {
            Look::Underline => 28.0,
            Look::Pill => 20.0 + CAPSULE_MARGIN.1 + CAPSULE_MARGIN.2,
            Look::Boxed => 14.0,
        };
        let sized = constraints.with_sizing(self.sizing());
        let width = if sized.max.width.is_finite() {
            sized.max.width
        } else {
            100.0 * self.segments.len() as f32
        };
        Size::new(width, tallest + padding)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let selected = self.shown.get();
        let indicator = self.slot(bounds, self.indicator.value(cx.now()));

        match self.look {
            Look::Underline => {
                let line = Rectangle::new(
                    Point::new(indicator.x, bounds.y + bounds.height - 3.0),
                    Size::new(indicator.width, 3.0),
                );
                cx.fill(
                    line,
                    palette.primary.base.color,
                    Border::default().rounded(1.5),
                );
            }
            Look::Boxed => {
                // A sunken track with a raised thumb, as iOS draws it: the
                // chosen segment is lifted, not flooded with colour.
                let radius = 9.0;
                cx.fill(
                    bounds,
                    palette.background.weak.color,
                    Border::default().rounded(radius),
                );
                let thumb = if palette.is_dark {
                    palette.background.strong.color
                } else {
                    palette.background.base.color
                };
                cx.fill_shadowed(
                    indicator.shrink(2.0),
                    thumb,
                    Border::default().rounded(radius - 2.0),
                    crate::Shadow {
                        color: crate::Color::from_rgba(0.0, 0.0, 0.0, 0.16),
                        offset: crate::Vector::new(0.0, 1.5),
                        blur_radius: 5.0,
                    },
                );
            }
            Look::Pill => {
                // A floating capsule of liquid glass, clear of the edges.
                let capsule = self.track(bounds);
                let radius = capsule.height / 2.0;
                let dark = palette.is_dark;
                let mut material = taim_core::material::Material::LIQUID;
                if dark {
                    material.transmission *= 0.8;
                }
                cx.fill_material(
                    capsule,
                    if dark {
                        crate::Color::from_rgb(0.08, 0.10, 0.16)
                    } else {
                        crate::Color::WHITE
                    },
                    Border::default().rounded(radius).width(1.0).color(if dark {
                        crate::Color::from_rgba(1.0, 1.0, 1.0, 0.14)
                    } else {
                        crate::Color::from_rgba(1.0, 1.0, 1.0, 0.6)
                    }),
                    crate::Shadow {
                        color: crate::Color::from_rgba(
                            0.0,
                            0.0,
                            0.0,
                            if dark { 0.5 } else { 0.16 },
                        ),
                        offset: crate::Vector::new(0.0, 6.0),
                        blur_radius: 20.0,
                    },
                    material,
                );

                // The chosen item sits in a soft, tinted lozenge.
                let pill = indicator.shrink(4.0);
                // Neutral, as Apple draws it: the accent is left to the
                // chosen item's glyph and label, where it reads.
                cx.fill(
                    pill,
                    if dark {
                        crate::Color::from_rgba(1.0, 1.0, 1.0, 0.10)
                    } else {
                        crate::Color::from_rgba(0.0, 0.0, 0.0, 0.05)
                    },
                    Border::default().rounded(pill.height / 2.0),
                );
            }
        }

        for (index, segment) in self.segments.iter().enumerate() {
            let slot = self.slot(bounds, index as f32);
            let chosen = index == selected;
            let color = match (self.look, chosen) {
                (Look::Boxed, true) => cx.text_color(),
                // The accent: the deeper shade on bright glass, the lighter
                // one on smoked glass, so it reads on either.
                (Look::Pill, true) if palette.is_dark => palette.primary.base.color,
                (Look::Pill, true) => palette.primary.strong.color,
                (Look::Underline, true) => palette.primary.base.color,
                // Readable on the track it sits on.
                (Look::Underline, false) => cx.muted(),
                (_, false) => cx.muted_on(palette.background.weak.color),
            };

            match &segment.glyph {
                Some(glyph) => {
                    let top =
                        Rectangle::new(slot.position(), Size::new(slot.width, slot.height * 0.6));
                    let bottom = Rectangle::new(
                        Point::new(slot.x, slot.y + slot.height * 0.55),
                        Size::new(slot.width, slot.height * 0.4),
                    );
                    glyph.paint_centered(cx, top, color);
                    segment.label.paint_centered(cx, bottom, color);
                }
                None => segment.label.paint_centered(cx, slot, color),
            }
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let bounds = cx.bounds();

        match look::gesture(event, cx.pointer()) {
            Some(look::Gesture::Down(at)) => {
                self.pressed = self.index_at(bounds, at);
                Handled::from(self.pressed.is_some())
            }
            Some(look::Gesture::Up(at)) => {
                let Some(pressed) = self.pressed.take() else {
                    return Handled::No;
                };
                if at.and_then(|at| self.index_at(bounds, at)) == Some(pressed) {
                    let _ = self.selected.set(pressed);
                    self.indicator.go(pressed as f32, cx.now());
                    cx.request_animation();
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    /// The navigation capsule is glass, which looks through what was
    /// painted before it.
    fn is_layer(&self) -> bool {
        self.look == Look::Pill
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        let selected = self.shown.get();
        let bounds = cx.bounds();
        let mut group = semantics::Node::new(self.id.clone(), bounds, semantics::Role::Group);

        for (index, segment) in self.segments.iter().enumerate() {
            let mut state = semantics::State::FOCUSABLE;
            state.set(semantics::State::SELECTED, index == selected);
            group = group.with_child(
                semantics::Node::new(
                    widget::Id::unique(),
                    self.slot(bounds, index as f32),
                    semantics::Role::Tab,
                )
                .with_name(segment.label.text())
                .with_state(state),
            );
        }

        Some(group)
    }

    fn cursor(&self) -> Option<mouse::Interaction> {
        Some(mouse::Interaction::Pointer)
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.indicator.is_moving(now)
    }
}

/// A rounded, toggleable filter chip bound to `selected`.
pub fn chip(label: impl Into<String>, selected: State<bool>) -> Chip {
    Chip {
        label: Label::new(label, Some(13.0)),
        selected,
        shown: selected.into_prop(),
        pressed: false,
        id: widget::Id::unique(),
    }
}

/// See [`chip`].
pub struct Chip {
    label: Label,
    selected: State<bool>,
    shown: Prop<bool>,
    pressed: bool,
    id: widget::Id,
}

impl View for Chip {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _constraints: Constraints) -> Size {
        let label = self.label.measure(cx);
        Size::new(label.width + 24.0, label.height + 14.0)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let on = self.shown.get();
        let (fill, text, edge) = if on {
            (
                palette.primary.base.color,
                palette.primary.base.text,
                palette.primary.base.color,
            )
        } else {
            (
                palette.background.weak.color,
                cx.text_color(),
                palette.background.strong.color,
            )
        };

        cx.fill(
            bounds,
            fill,
            Border::default()
                .width(1.0)
                .color(edge)
                .rounded(bounds.height / 2.0),
        );
        self.label.paint_centered(cx, bounds, text);
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match look::gesture(event, cx.pointer()) {
            Some(look::Gesture::Down(at)) if cx.bounds().contains(at) => {
                self.pressed = true;
                Handled::Yes
            }
            Some(look::Gesture::Up(at)) if self.pressed => {
                self.pressed = false;
                if at.is_some_and(|at| cx.bounds().contains(at)) {
                    self.selected.update(|on| *on = !*on);
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        let mut state = semantics::State::FOCUSABLE;
        state.set(semantics::State::CHECKED, self.shown.get());
        Some(
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::Checkbox)
                .with_name(self.label.text())
                .with_state(state),
        )
    }

    fn cursor(&self) -> Option<mouse::Interaction> {
        Some(mouse::Interaction::Pointer)
    }
}

/// A shimmering placeholder of `width` × `height`, shown while content
/// loads.
pub fn skeleton(width: f32, height: f32) -> Skeleton {
    Skeleton {
        size: Size::new(width, height),
        born: Instant::now(),
    }
}

/// See [`skeleton`].
pub struct Skeleton {
    size: Size,
    born: Instant,
}

impl View for Skeleton {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        constraints.clamp(self.size)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let phase = (cx.now().saturating_duration_since(self.born).as_secs_f32() / 1.2).fract();
        let pulse = 0.5 - 0.5 * (phase * std::f32::consts::TAU).cos();
        let color = palette
            .background
            .weak
            .color
            .mix(palette.background.strong.color, pulse);

        cx.fill(bounds, color, Border::default().rounded(4.0));
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        Some(
            semantics::Node::new(widget::Id::unique(), cx.bounds(), semantics::Role::Group)
                .with_state(semantics::State::BUSY),
        )
    }

    fn is_animating(&self, _now: Instant) -> bool {
        true
    }
}

/// A circle with someone's initials, colored from their name.
pub fn avatar(name: impl Into<String>) -> Avatar {
    let name = name.into();
    let initials: String = name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect();

    Avatar {
        initials: Label::new(initials, Some(14.0)),
        diameter: 40.0,
        hue: name.bytes().fold(0_u32, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(u32::from(byte))
        }),
        name,
    }
}

/// See [`avatar`].
pub struct Avatar {
    name: String,
    initials: Label,
    diameter: f32,
    hue: u32,
}

impl Avatar {
    /// Diameter in logical pixels.
    pub fn size(mut self, diameter: f32) -> Self {
        self.diameter = diameter.max(1.0);
        self
    }

    fn color(&self) -> Color {
        const TONES: [Color; 6] = [
            Color::from_rgb8(0x17, 0x5E, 0x54),
            Color::from_rgb8(0x6A, 0x2C, 0x91),
            Color::from_rgb8(0x9C, 0x3D, 0x10),
            Color::from_rgb8(0x1F, 0x4E, 0x9C),
            Color::from_rgb8(0x8A, 0x1C, 0x4A),
            Color::from_rgb8(0x3B, 0x6B, 0x16),
        ];
        TONES[self.hue as usize % TONES.len()]
    }
}

impl View for Avatar {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _constraints: Constraints) -> Size {
        let _ = self.initials.measure(cx);
        Size::new(self.diameter, self.diameter)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        cx.fill(
            bounds,
            self.color(),
            Border::default().rounded(self.diameter / 2.0),
        );
        self.initials.paint_centered(cx, bounds, Color::WHITE);
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        Some(
            semantics::Node::new(widget::Id::unique(), cx.bounds(), semantics::Role::Image)
                .with_name(self.name.clone()),
        )
    }
}

/// `content` with a small count bubble at its top-right corner; no bubble
/// while the count is zero.
pub fn badge(content: impl Into<AnyView>, count: impl IntoProp<u32>) -> Badge {
    Badge {
        content: Child::new(content.into()),
        count: count.into_prop(),
        number: Label::new("", Some(10.0)),
    }
}

/// See [`badge`].
pub struct Badge {
    content: Child,
    count: Prop<u32>,
    number: Label,
}

impl View for Badge {
    fn sizing(&self) -> Sizing {
        self.content.sizing()
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let count = self.count.get();
        self.number.set(&if count > 99 {
            "99+".into()
        } else {
            count.to_string()
        });
        let _ = self.number.measure(cx);

        let size = self.content.measure(cx, constraints);
        self.content.place(Point::ORIGIN);
        size
    }

    fn paint(&self, _cx: &mut PaintCx<'_>) {}

    fn paint_over(&self, cx: &mut PaintCx<'_>) {
        if self.count.get() == 0 {
            return;
        }
        let bounds = cx.bounds();
        let bubble = Rectangle::new(
            Point::new(bounds.x + bounds.width - 6.0, bounds.y - 6.0),
            Size::new(14.0, 14.0),
        );
        let palette = *cx.theme().palette();
        cx.fill(
            bubble,
            palette.danger.base.color,
            Border::default().rounded(7.0),
        );
        self.number
            .paint_centered(cx, bubble, palette.danger.base.text);
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        let count = self.count.get();
        (count > 0).then(|| {
            semantics::Node::new(
                widget::Id::unique(),
                cx.bounds(),
                semantics::Role::StaticText,
            )
            .with_name(format!("{count} unread"))
        })
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.content);
    }
}

/// A rounded search field with a magnifier and a clear button.
pub fn search(placeholder: impl IntoProp<String>, query: State<String>) -> AnyView {
    container(
        row(crate::views![
            text("⌕").size(18),
            text_field(placeholder, query),
            when(
                move || !query.get().is_empty(),
                move || {
                    button("✕").on_press(move || {
                        let _ = query.set(String::new());
                    })
                }
            ),
        ])
        .gap(8.0)
        .align(super::Align::Center)
        .fill_width(),
    )
    .padding([4.0, 12.0])
    .background(Color::from_rgb8(0xE6, 0xE6, 0xEA))
    .border(Border::default().rounded(22.0))
    .fill_width()
    .into()
}

/// A screen: `top` bar, `body` filling the middle, `bottom` bar, and any
/// `overlays` (drawer, sheet, snackbar) above everything.
pub fn scaffold(
    top: impl Into<AnyView>,
    body: impl Into<AnyView>,
    bottom: impl Into<AnyView>,
    overlays: impl IntoIterator<Item = AnyView>,
) -> AnyView {
    let screen = column(crate::views![top, container(body).fill(), bottom,])
        .fill_width()
        .fill_height();

    layers(std::iter::once(screen.into()).chain(overlays)).into()
}
