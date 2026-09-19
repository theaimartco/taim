use super::{Grid, Sizing, finite_nonnegative};
use crate::core::widget::Tree;
use crate::core::{Length, Pixels, Size};

/// Geometry shared by layout and viewport virtualization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Track {
    pub(super) cells_per_row: usize,
    pub(super) cell_width: f32,
    pub(super) cell_height: Option<f32>,
    total_rows: usize,
}

impl Track {
    #[inline]
    fn estimated_row_height(self, fallback: f32) -> Option<f32> {
        self.cell_height
            .or_else(|| (fallback.is_finite() && fallback > 0.0).then_some(fallback))
    }
}

/// Per-grid track cache. The key includes every input used by geometry.
#[derive(Debug, Default)]
pub(super) struct State {
    track: Option<(TrackKey, Track)>,
}

impl State {
    #[inline]
    pub(super) fn invalidate(&mut self) {
        self.track = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Constraint {
    MaxWidth(Pixels),
    Amount(usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TrackKey {
    available_width_bits: u32,
    available_height_bits: u32,
    spacing_bits: u32,
    child_count: usize,
    columns: Constraint,
    height: Sizing,
}

impl<'a, Message, Theme, Renderer> Grid<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    pub(super) fn track(&self, state: &mut State, available: Size) -> Option<Track> {
        if self.children.is_empty() {
            state.track = None;
            return None;
        }

        let available = Size::new(
            finite_nonnegative(available.width),
            finite_nonnegative(available.height),
        );
        let key = TrackKey {
            available_width_bits: available.width.to_bits(),
            available_height_bits: available.height.to_bits(),
            spacing_bits: self.spacing.to_bits(),
            child_count: self.children.len(),
            columns: self.columns,
            height: self.height,
        };

        if let Some((cached_key, track)) = state.track
            && cached_key == key
        {
            return Some(track);
        }

        let cells_per_row = match self.columns {
            Constraint::MaxWidth(pixels) => {
                ((available.width + self.spacing) / (pixels.0 + self.spacing)).ceil() as usize
            }
            Constraint::Amount(amount) => amount,
        }
        .max(1);

        let horizontal_spacing = f64::from(self.spacing) * cells_per_row.saturating_sub(1) as f64;
        let cell_width = ((f64::from(available.width) - horizontal_spacing).max(0.0)
            / cells_per_row as f64) as f32;
        let total_rows = self.children.len().div_ceil(cells_per_row);
        let cell_height = match self.height {
            Sizing::AspectRatio(ratio) => Some(cell_width / ratio),
            Sizing::EvenlyDistribute(Length::Shrink) => None,
            Sizing::EvenlyDistribute(_) => {
                let vertical_spacing =
                    f64::from(self.spacing) * total_rows.saturating_sub(1) as f64;

                Some(
                    ((f64::from(available.height) - vertical_spacing).max(0.0) / total_rows as f64)
                        as f32,
                )
            }
        };
        let track = Track {
            cells_per_row,
            cell_width,
            cell_height,
            total_rows,
        };

        state.track = Some((key, track));
        Some(track)
    }

    /// Maps a local vertical viewport to the intersecting child range in O(1).
    pub fn visible_range(
        &self,
        tree: &mut Tree,
        available: Size,
        visible: (f32, f32),
        estimated_row_height: f32,
    ) -> std::ops::Range<usize> {
        let state = tree.state.downcast_mut::<State>();

        let Some(track) = self.track(state, available) else {
            return 0..self.children.len();
        };
        let (visible_start, visible_end) = visible;

        if !visible_start.is_finite()
            || !visible_end.is_finite()
            || visible_end <= visible_start
            || visible_end <= 0.0
        {
            return 0..0;
        }

        let Some(row_height) = track.estimated_row_height(estimated_row_height) else {
            return 0..self.children.len();
        };

        if row_height <= 0.0 {
            return 0..0;
        }

        let stride = row_height + self.spacing;
        if !stride.is_finite() || stride <= 0.0 {
            return 0..self.children.len();
        }

        let mut first_row = (visible_start.max(0.0) / stride).floor() as usize;
        if first_row < track.total_rows && first_row as f32 * stride + row_height <= visible_start {
            first_row += 1;
        }

        let last_row = (visible_end.max(0.0) / stride).ceil() as usize;
        let first_row = first_row.min(track.total_rows);
        let last_row = last_row.min(track.total_rows).max(first_row);
        let start = first_row.saturating_mul(track.cells_per_row);
        let end = last_row
            .saturating_mul(track.cells_per_row)
            .min(self.children.len());
        let start = start.min(self.children.len());

        start..end.max(start)
    }
}
