use crate::core::{Rectangle, Transformation};
use crate::layer;

#[derive(Debug)]
pub(crate) struct RetainedFrame {
    pub(crate) key: u64,
    pub(crate) revision: u64,
    pub(crate) bounds: Rectangle,
    pub(crate) transformation: Transformation,
    pub(crate) opacity: f32,
    pub(crate) display_list: layer::Stack,
}

#[derive(Debug)]
pub(crate) struct ActiveRetainedFrame {
    pub(crate) key: u64,
    pub(crate) revision: u64,
    pub(crate) bounds: Rectangle,
    pub(crate) transformation: Transformation,
    pub(crate) opacity: f32,
    pub(crate) range: std::ops::Range<usize>,
    pub(crate) sequence: usize,
}

#[derive(Debug)]
pub(crate) struct RecordingRetainedFrame {
    pub(crate) key: u64,
    pub(crate) revision: u64,
    pub(crate) bounds: Rectangle,
    pub(crate) transformation: Transformation,
    pub(crate) opacity: f32,
    pub(crate) start: usize,
}

pub(crate) fn innermost(mut frames: Vec<ActiveRetainedFrame>) -> Vec<ActiveRetainedFrame> {
    // Recorded ranges are nested or disjoint. Sorting turns containment into
    // a suffix-minimum query instead of scanning every pair of frames.
    frames.sort_unstable_by(|left, right| {
        left.range
            .start
            .cmp(&right.range.start)
            .then_with(|| right.range.end.cmp(&left.range.end))
            .then_with(|| left.sequence.cmp(&right.sequence))
    });

    let mut innermost = Vec::with_capacity(frames.len());
    let mut minimum_later_end = usize::MAX;
    let mut frames = frames.into_iter().rev().peekable();

    while let Some(mut representative) = frames.next() {
        while frames.peek().is_some_and(|next| {
            next.range.start == representative.range.start
                && next.range.end == representative.range.end
        }) {
            // Reverse iteration sees the latest duplicate first. Keeping the
            // earliest recording preserves deterministic replay ownership.
            representative = frames.next().expect("peeked retained frame exists");
        }

        let contains_a_later_range = minimum_later_end < representative.range.end;
        minimum_later_end = minimum_later_end.min(representative.range.end);

        if !contains_a_later_range {
            innermost.push(representative);
        }
    }

    innermost.reverse();
    innermost
}
