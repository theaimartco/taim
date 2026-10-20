//! TAIM-owned declarative linear-container sizing and layout identity.
//!
//! A configured length is an intent, not a mutable accumulator. The effective
//! size of a Row or Column is recomputed from that intent on every tree diff.
//! This avoids retaining an obsolete Fill/Fluid demand when a child disappears
//! and makes the result invariant to previous diff history.

use crate::core::alignment::Alignment;
use crate::core::layout;
use crate::core::{Length, Padding, Size};

/// Direction in which child sizes accumulate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flow {
    /// Child main-axis lengths combine horizontally.
    Row,
    /// Child main-axis lengths combine vertically.
    Column,
}

/// Resolve the *current* effective length strategies from immutable intent.
///
/// The fixed/filled explicitly configured axes remain fixed/filled. Like the
/// legacy widget contract, child strategies are inspected when either declared
/// axis is Fit; unlike that contract, previous resolutions never feed back into
/// the next diff. An empty child list therefore restores the original intent.
pub(crate) fn measure_intent(
    declared: Size<Length>,
    flow: Flow,
    children: impl IntoIterator<Item = Size<Length>>,
) -> Size<Length> {
    if !declared.width.is_fit() && !declared.height.is_fit() {
        return declared;
    }

    children.into_iter().fold(declared, |current, child| match flow {
        Flow::Row => Size::new(
            current.width.stack(child.width),
            current.height.cross(child.height),
        ),
        Flow::Column => Size::new(
            current.width.cross(child.width),
            current.height.stack(child.height),
        ),
    })
}

/// The cache key represents the complete resolved linear-container contract.
///
/// Widget::diff conservatively clears the state cache if any child's content
/// changes. This fingerprint handles changes in container geometry/options
/// even if the widget survives with the same retained Tree instance.
pub(crate) fn fingerprint(
    child_count: usize,
    resolved: Size<Length>,
    padding: Padding,
    spacing: f32,
    align: Alignment,
) -> layout::cache::Fingerprint {
    layout::cache::Fingerprint::build(|hash| {
        hash.write_usize(child_count);
        resolved.width.hash_stable(hash);
        resolved.height.hash_stable(hash);
        hash.write_u32(spacing.to_bits());
        hash.write_u32(padding.top.to_bits());
        hash.write_u32(padding.right.to_bits());
        hash.write_u32(padding.bottom.to_bits());
        hash.write_u32(padding.left.to_bits());
        hash.write_u8(match align {
            Alignment::Start => 0,
            Alignment::Center => 1,
            Alignment::End => 2,
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::length::Constraint;

    fn fit() -> Size<Length> {
        Size::new(Length::Fit, Length::Fit)
    }

    #[test]
    fn removing_row_fill_restores_declared_fit_without_state_leak() {
        let filled = measure_intent(
            fit(),
            Flow::Row,
            [Size::new(Length::Fill, Length::Fit)],
        );
        assert_eq!(filled.width, Length::Fill);
        let after_removal = measure_intent(fit(), Flow::Row, []);
        assert_eq!(after_removal, fit());
    }

    #[test]
    fn removing_column_fill_restores_declared_fit_without_state_leak() {
        let filled = measure_intent(
            fit(),
            Flow::Column,
            [Size::new(Length::Fit, Length::Fill)],
        );
        assert_eq!(filled.height, Length::Fill);
        assert_eq!(measure_intent(fit(), Flow::Column, []), fit());
    }

    #[test]
    fn explicit_fixed_length_is_not_overridden_by_child_fill() {
        let declared = Size::new(Length::Fixed(80.0), Length::Fit);
        let got = measure_intent(
            declared,
            Flow::Row,
            [Size::new(Length::Fill, Length::Fill)],
        );
        assert_eq!(got.width, Length::Fixed(80.0));
        assert_eq!(got.height, Length::Fill);
    }

    #[test]
    fn vertical_fluid_minima_accumulate_only_on_main_axis() {
        let result = measure_intent(
            fit(),
            Flow::Column,
            [
                Size::new(Length::Fit, Length::Fluid(Constraint::Min(5.0))),
                Size::new(Length::Fit, Length::Fluid(Constraint::Min(7.0))),
            ],
        );
        assert_eq!(result.height, Length::Fluid(Constraint::Min(12.0)));
    }

    #[test]
    fn resolved_output_is_history_independent() {
        let children = [
            Size::new(Length::Fit, Length::Fill),
            Size::new(Length::Fill, Length::Fit),
        ];
        let a = measure_intent(fit(), Flow::Row, children);
        let _intermediate = measure_intent(fit(), Flow::Row, []);
        let b = measure_intent(fit(), Flow::Row, children);
        assert_eq!(a, b);
    }

    #[test]
    fn first_render_row_can_infer_fill_without_any_previous_diff() {
        let effective = measure_intent(
            fit(), Flow::Row,
            [Size::new(Length::Fill, Length::Fit)],
        );
        assert_eq!(effective.width, Length::Fill);
        assert_eq!(effective.height, Length::Fit);
    }

    #[test]
    fn first_render_column_can_infer_fill_without_any_previous_diff() {
        let effective = measure_intent(
            fit(), Flow::Column,
            [Size::new(Length::Fit, Length::Fill)],
        );
        assert_eq!(effective.width, Length::Fit);
        assert_eq!(effective.height, Length::Fill);
    }

    #[test]
    fn fresh_child_inputs_do_not_retain_prior_fluid_constraints() {
        let fluid = [Size::new(Length::Fluid(Constraint::Min(5.0)), Length::Fit)];
        assert_eq!(measure_intent(fit(), Flow::Row, fluid).width, Length::Fluid(Constraint::Min(5.0)));
        assert_eq!(measure_intent(fit(), Flow::Row, []).width, Length::Fit);
    }

    #[test]
    fn fingerprint_tracks_all_structural_inputs() {
        let key = fingerprint(2, fit(), Padding::ZERO, 4.0, Alignment::Start);
        assert_ne!(key, fingerprint(3, fit(), Padding::ZERO, 4.0, Alignment::Start));
        assert_ne!(key, fingerprint(2, fit(), Padding::ZERO, 5.0, Alignment::Start));
        assert_ne!(key, fingerprint(2, fit(), Padding::ZERO, 4.0, Alignment::End));
        assert_ne!(key, fingerprint(
            2,
            Size::new(Length::Fill, Length::Fit),
            Padding::ZERO,
            4.0,
            Alignment::Start,
        ));
    }
}
