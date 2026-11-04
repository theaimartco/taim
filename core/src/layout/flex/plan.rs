//! TAIM-owned measurement schedule for horizontal and vertical linear groups.
//!
//! Classification happens before any child is measured, so fixed reservations
//! and phase order do not depend on which sibling happens to appear first.

use super::{Length, INLINE_CHILDREN};
use smallvec::SmallVec;

/// Exactly one measuring phase is assigned to each child.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Pass {
    /// Fixed or content-measured child: discover main and cross size early.
    Intrinsic,
    /// Measure cross-flexible content after non-flexible siblings.
    CrossProbe,
    /// Reserve fixed main space while deferring cross-flexible measurement.
    CrossDeferred(f32),
    /// Allocate main-axis space using TAIM's weighted water-fill solver.
    Weighted,
}

/// Immutable classification of one child.
#[derive(Debug, Clone, Copy)]
pub(super) struct Slot {
    pub(super) main: Length,
    pub(super) cross: Length,
    pub(super) pass: Pass,
}

/// An inline-first, immutable measurement order.
#[derive(Debug)]
pub(super) struct Schedule {
    pub(super) slots: SmallVec<[Slot; INLINE_CHILDREN]>,
    pub(super) has_cross_dependents: bool,
    pub(super) reserved_main: f64,
}

impl Schedule {
    /// Create a complete schedule before any widget's layout method runs.
    pub(super) fn build(
        strategies: impl IntoIterator<Item = (Length, Length)>,
        compress_main: bool,
        compress_cross: bool,
    ) -> Self {
        let mut slots = SmallVec::new();
        let mut reserved_main = 0.0_f64;
        let mut has_cross_dependents = false;

        for (main, cross) in strategies {
            let pass = classify(main, cross, compress_main, compress_cross);
            if let Pass::CrossDeferred(extent) = pass {
                reserved_main += f64::from(extent);
            }
            has_cross_dependents |= matches!(pass, Pass::CrossProbe | Pass::CrossDeferred(_));
            slots.push(Slot { main, cross, pass });
        }

        Self { slots, has_cross_dependents, reserved_main }
    }

    /// Main-axis room after between-item spacing and deferred fixed sizes.
    /// Use wide arithmetic to avoid float overflow changing +MAX into -INF.
    pub(super) fn main_budget(&self, max_extent: f32, between: f32) -> f32 {
        if max_extent.is_nan() {
            return 0.0;
        }
        if max_extent == f32::INFINITY {
            return f32::INFINITY;
        }
        if max_extent <= 0.0 {
            return 0.0;
        }
        let spacing = if between.is_finite() { f64::from(between.max(0.0)) } else { 0.0 };
        let gutters = spacing * self.slots.len().saturating_sub(1) as f64;
        (f64::from(max_extent) - gutters - self.reserved_main).clamp(0.0, f64::from(f32::MAX)) as f32
    }
}

fn classify(main: Length, cross: Length, compress_main: bool, compress_cross: bool) -> Pass {
    if !compress_main && main.fill_factor() > 0 {
        return Pass::Weighted;
    }
    if compress_cross && cross.fill_factor() > 0 {
        if let Length::Fixed(size) = main
            && size.is_finite()
            && size >= 0.0
        {
            return Pass::CrossDeferred(size);
        }
        return Pass::CrossProbe;
    }
    Pass::Intrinsic
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_axis_fills_are_weighted_unless_compressed() {
        assert_eq!(classify(Length::Fill, Length::Fit, false, false), Pass::Weighted);
        assert_eq!(classify(Length::Fill, Length::Fit, true, false), Pass::Intrinsic);
    }

    #[test]
    fn fixed_cross_dependent_size_is_reserved_ahead_of_measurements() {
        let schedule = Schedule::build(
            [(Length::Fit, Length::Fit), (Length::Fixed(20.0), Length::Fill)],
            false,
            true,
        );
        assert!(schedule.has_cross_dependents);
        assert_eq!(schedule.slots[1].pass, Pass::CrossDeferred(20.0));
        assert_eq!(schedule.main_budget(100.0, 10.0), 70.0);
    }

    #[test]
    fn nonfinite_fixed_lengths_are_never_reserved() {
        let schedule = Schedule::build(
            [(Length::Fixed(f32::NAN), Length::Fill), (Length::Fixed(f32::INFINITY), Length::Fill)],
            false,
            true,
        );
        assert_eq!(schedule.reserved_main, 0.0);
        assert!(schedule.slots.iter().all(|s| s.pass == Pass::CrossProbe));
    }

    #[test]
    fn adversarial_gutters_cannot_poison_the_budget() {
        let schedule = Schedule::build(
            [(Length::Fit, Length::Fit), (Length::Fit, Length::Fit)],
            false,
            false,
        );
        assert_eq!(schedule.main_budget(100.0, f32::NAN), 100.0);
        assert_eq!(schedule.main_budget(f32::MAX, f32::MAX), 0.0);
        assert_eq!(schedule.main_budget(f32::INFINITY, 5.0), f32::INFINITY);
    }

    #[test]
    fn scheduling_does_not_depend_on_sibling_order() {
        let first = [(Length::Fill, Length::Fit), (Length::Fixed(7.0), Length::Fill)];
        let reverse = [first[1], first[0]];
        let a = Schedule::build(first, false, true);
        let b = Schedule::build(reverse, false, true);
        assert_eq!(a.reserved_main, b.reserved_main);
        assert_eq!(a.main_budget(40.0, 4.0), b.main_budget(40.0, 4.0));
    }
}
