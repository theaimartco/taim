//! Create lines from a [`Path`] and assigns them various attributes/styles.
//!
//! [`Path`]: super::Path
pub use crate::geometry::Style;

use taim_core::Color;

/// The style of a stroke.
#[derive(Debug, Clone, Copy)]
pub struct Stroke<'a> {
    /// The color or gradient of the stroke.
    ///
    /// By default, it is set to a [`Style::Solid`] with [`Color::BLACK`].
    pub style: Style,
    /// The distance between the two edges of the stroke.
    pub width: f32,
    /// The shape to be used at the end of open subpaths when they are stroked.
    pub line_cap: LineCap,
    /// The shape to be used at the corners of paths or basic shapes when they
    /// are stroked.
    pub line_join: LineJoin,
    /// The dash pattern used when stroking the line.
    pub line_dash: LineDash<'a>,
}

impl Stroke<'_> {
    /// Sets the fill [`Style`] of the stroke.
    pub fn with_style(self, style: impl Into<Style>) -> Self {
        Self {
            style: style.into(),
            ..self
        }
    }

    /// Sets the color of the [`Stroke`].
    pub fn with_color(self, color: Color) -> Self {
        Stroke {
            style: Style::Solid(color),
            ..self
        }
    }

    /// Sets the width of the [`Stroke`].
    pub fn with_width(self, width: f32) -> Self {
        Stroke {
            width: finite_nonnegative(width),
            ..self
        }
    }

    /// Sets the [`LineCap`] of the [`Stroke`].
    pub fn with_line_cap(self, line_cap: LineCap) -> Self {
        Stroke { line_cap, ..self }
    }

    /// Sets the [`LineJoin`] of the [`Stroke`].
    pub fn with_line_join(self, line_join: LineJoin) -> Self {
        Stroke { line_join, ..self }
    }

    /// Sets the dash pattern of the [`Stroke`].
    pub fn with_line_dash<'a>(self, line_dash: LineDash<'a>) -> Stroke<'a> {
        Stroke {
            style: self.style,
            width: self.width,
            line_cap: self.line_cap,
            line_join: self.line_join,
            line_dash,
        }
    }

    /// Returns a finite version safe for tessellation.
    pub fn normalized(self) -> Self {
        Self {
            width: finite_nonnegative(self.width),
            line_dash: if self.line_dash.is_valid() {
                self.line_dash
            } else {
                LineDash::default()
            },
            ..self
        }
    }

    /// Returns whether this stroke can contribute visible geometry.
    pub fn is_visible(self) -> bool {
        finite_nonnegative(self.width) > 0.0
    }
}

impl Default for Stroke<'_> {
    fn default() -> Self {
        Stroke {
            style: Style::Solid(Color::BLACK),
            width: 1.0,
            line_cap: LineCap::default(),
            line_join: LineJoin::default(),
            line_dash: LineDash::default(),
        }
    }
}

/// The shape used at the end of open subpaths when they are stroked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum LineCap {
    /// The stroke for each sub-path does not extend beyond its two endpoints.
    #[default]
    Butt,
    /// At the end of each sub-path, the shape representing the stroke will be
    /// extended by a square.
    Square,
    /// At the end of each sub-path, the shape representing the stroke will be
    /// extended by a semicircle.
    Round,
}

/// The shape used at the corners of paths or basic shapes when they are
/// stroked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum LineJoin {
    /// A sharp corner.
    #[default]
    Miter,
    /// A round corner.
    Round,
    /// A bevelled corner.
    Bevel,
}

/// The dash pattern used when stroking the line.
#[derive(Debug, Clone, Copy, Default)]
pub struct LineDash<'a> {
    /// The alternating lengths of lines and gaps which describe the pattern.
    pub segments: &'a [f32],

    /// The offset of [`LineDash::segments`] to start the pattern.
    pub offset: usize,
}

impl<'a> LineDash<'a> {
    /// Creates a dash pattern at offset zero.
    pub const fn new(segments: &'a [f32]) -> Self {
        Self {
            segments,
            offset: 0,
        }
    }

    /// Sets the starting segment offset.
    pub const fn with_offset(self, offset: usize) -> Self {
        Self { offset, ..self }
    }

    /// Returns whether the stroke is continuous.
    pub const fn is_solid(self) -> bool {
        self.segments.is_empty()
    }

    /// Returns whether every dash/gap has a finite positive length.
    pub fn is_valid(self) -> bool {
        self.segments
            .iter()
            .all(|length| length.is_finite() && *length > 0.0)
    }

    /// Returns the total length of one dash cycle.
    pub fn period(self) -> f32 {
        self.segments.iter().copied().sum()
    }

    /// Returns the segment index after wrapping the configured offset.
    pub fn wrapped_offset(self) -> usize {
        if self.segments.is_empty() {
            0
        } else {
            self.offset % self.segments.len()
        }
    }
}

#[inline]
fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}
