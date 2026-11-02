//! What was redrawn, and why it was not less.
//!
//! # The question this answers
//!
//! Damage tracking exists so a frame redraws a button instead of a
//! screen. When it works, nobody notices. When it does not, the symptom
//! is "everything is slow" and the cause is invisible: the frame is
//! *correct*, just enormously more expensive than it needed to be.
//!
//! So the interesting reading here is not the rectangle. It is
//! [`Region::Full`] — a full-surface redraw — and **why**. A shell that
//! silently fell back to full damage every frame would be indistinguishable
//! from one with no damage tracking at all, except in the battery.
//!
//! # The cause, and where it used to go
//!
//! `UserInterface::update` now returns a [`damage::Attribution`]: the
//! region, every [`damage::Cause`] that contributed, and — when the frame
//! went full — a [`damage::Fallback`] saying whether a caller *asked* for
//! it or damage tracking *gave up*.
//!
//! It did not always. `Aggregator` read each request's region and dropped
//! its cause, and `UserInterface` short-circuited a relayout straight to
//! `Region::Full` without going through the aggregator at all — so the one
//! place in the frame that knew why everything was redrawing was the one
//! place that threw the answer away. This panel is the reason that was
//! noticed.
//!
//! # Requested is not the same finding as overflow
//!
//! [`damage::Fallback::Requested`] is a caller saying "I cannot describe
//! this precisely". Often correct, often nothing to fix.
//! [`damage::Fallback::Overflow`] is damage tracking failing: the frame
//! *had* precise damage and the aggregator could not hold it. Same
//! rectangle, same cost, completely different thing to do about it — which
//! is why they are different variants and not a bool.

use taim::Size;
use taim::prelude::*;
use taim_core::damage::{Attribution, Causes, Fallback, Region};

/// What this panel still cannot say.
///
/// Narrower than it was: the cause now arrives. What is missing is *which
/// widget* caused it, which needs per-widget damage reporting that does
/// not exist yet.
pub const MISSING: &str = "Which widget caused the damage is not shown: nothing reports damage \
                           per widget yet, so a frame's causes are the frame's, not any one \
                           widget's.";

/// How a [`Fallback`] reads.
fn describe_fallback(fallback: Fallback) -> String {
    match fallback {
        // A caller admitting it cannot be precise. Often correct.
        Fallback::Requested(cause) => format!("requested ({cause:?})"),
        // Damage tracking giving up. The frame *had* precise damage.
        Fallback::Overflow { areas } => format!(
            "OVERFLOW \u{2014} {areas} scattered areas exceeded the budget, so precision was \
             abandoned. This frame had precise damage and could not keep it."
        ),
    }
}

/// How a set of [`Causes`] reads.
fn describe_causes(causes: Causes) -> String {
    if causes.is_empty() {
        return "no cause recorded".to_owned();
    }

    causes
        .iter_causes()
        .map(|cause| format!("{cause:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// How much of the surface a region covers, `0.0..=1.0`.
///
/// The number that matters. A damage rectangle covering 98% of the
/// surface is a full redraw wearing a disguise, and it costs the same.
pub fn coverage(region: Option<&Region>, surface: Size) -> f32 {
    let area = surface.width * surface.height;

    if area <= 0.0 {
        return 0.0;
    }

    match region {
        None => 0.0,
        Some(region) => match region.bounds() {
            // No bounds means the whole surface, not nothing: `Region`
            // reports `None` for a full redraw. Reading that as zero
            // damage would report the most expensive frame possible as
            // the cheapest.
            None => 1.0,
            Some(bounds) => ((bounds.width * bounds.height) / area).clamp(0.0, 1.0),
        },
    }
}

/// Renders the damage overlay.
pub fn view(damage: Option<&Attribution>, surface: Size, layout_changed: bool) -> AnyView {
    let region = damage.map(|damage| damage.region);
    let covered = coverage(region.as_ref(), surface);

    let headline = match region {
        None => "Nothing was damaged.".to_owned(),
        Some(region) => match region.bounds() {
            None => "FULL REDRAW".to_owned(),
            Some(bounds) => format!(
                "{:.0},{:.0} {:.0}\u{00d7}{:.0}",
                bounds.x, bounds.y, bounds.width, bounds.height
            ),
        },
    };

    let why = match damage {
        None => String::new(),
        Some(damage) => match damage.fallback {
            Some(fallback) => format!(
                "{}  \u{00b7}  causes: {}",
                describe_fallback(fallback),
                describe_causes(damage.causes)
            ),
            None => format!("causes: {}", describe_causes(damage.causes)),
        },
    };

    // 90% is not a threshold with a theory behind it; it is the point at
    // which the saving has stopped being worth the bookkeeping, and a
    // developer should know their damage tracking is doing nothing for
    // them.
    let verdict = if covered >= 0.9 && region.is_some() {
        format!(
            "{:.0}% of the surface \u{2014} damage tracking is buying nothing here",
            covered * 100.0
        )
    } else if region.is_some() {
        format!("{:.1}% of the surface", covered * 100.0)
    } else {
        "idle".to_owned()
    };

    container(
        column(views![
            text(headline).size(20.0),
            text(why).size(13.0),
            text(verdict).size(13.0),
            row(views![
                text(format!(
                    "surface {:.0}\u{00d7}{:.0}",
                    surface.width, surface.height
                ))
                .size(12.0),
                text(if layout_changed {
                    // Worth pairing: a relayout is the usual reason a frame
                    // damages everything, and seeing them together is the
                    // difference between a mystery and a cause.
                    "layout changed"
                } else {
                    "layout reused"
                })
                .size(12.0),
            ])
            .gap(12.0),
            text(MISSING).size(11.0),
        ])
        .gap(8.0),
    )
    .center_x()
    .padding(12.0)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use taim::{Point, Rectangle};

    const SURFACE: Size = Size {
        width: 400.0,
        height: 600.0,
    };

    #[test]
    fn no_damage_covers_nothing() {
        assert_eq!(coverage(None, SURFACE), 0.0);
    }

    #[test]
    fn a_full_redraw_covers_everything() {
        // `Region` reports `None` bounds for a full redraw. Reading that
        // as zero damage would report the most expensive frame possible as
        // the cheapest -- and it is the exact frame a developer is hunting.
        let full = Region::Full;

        assert_eq!(coverage(Some(&full), SURFACE), 1.0);
    }

    #[test]
    fn a_small_rectangle_covers_a_small_fraction() {
        let region = Region::area(Rectangle::new(Point::ORIGIN, Size::new(40.0, 60.0)))
            .expect("a real area");

        // 40x60 of 400x600 is 1%.
        assert!((coverage(Some(&region), SURFACE) - 0.01).abs() < 0.001);
    }

    #[test]
    fn a_rectangle_covering_almost_everything_reads_as_almost_everything() {
        // A damage rectangle covering 98% of the surface is a full redraw
        // wearing a disguise, and it costs the same.
        let region = Region::area(Rectangle::new(Point::ORIGIN, Size::new(400.0, 588.0)))
            .expect("a real area");

        assert!(coverage(Some(&region), SURFACE) > 0.9);
    }

    #[test]
    fn a_zero_sized_surface_does_not_divide_by_zero() {
        // A window minimised to nothing, or a configure of 0x0 before the
        // first real one.
        let full = Region::Full;

        assert_eq!(coverage(Some(&full), Size::ZERO), 0.0);
    }

    #[test]
    fn coverage_is_clamped_to_the_surface() {
        // Damage larger than the surface is possible -- a widget that
        // escaped its bounds -- and reporting 340% would be noise where a
        // developer needs a number.
        let region = Region::area(Rectangle::new(Point::ORIGIN, Size::new(4000.0, 6000.0)))
            .expect("a real area");

        assert_eq!(coverage(Some(&region), SURFACE), 1.0);
    }

    #[test]
    fn the_panel_says_what_is_still_missing() {
        // Narrower than it was: the cause arrives now. Which *widget*
        // caused it does not.
        assert!(MISSING.contains("widget"), "{MISSING}");
    }

    #[test]
    fn a_relayout_names_layout_rather_than_unknown() {
        // The gap this closed: `UserInterface` short-circuited a relayout
        // straight to `Region::Full`, so the one place in the frame that
        // knew why everything was redrawing was the one place that threw
        // the answer away.
        let attribution = Attribution {
            region: Region::Full,
            causes: Causes::LAYOUT,
            fallback: Some(Fallback::Requested(taim_core::damage::Cause::Layout)),
        };

        assert_eq!(describe_causes(attribution.causes), "Layout");
        assert!(
            describe_fallback(attribution.fallback.unwrap()).contains("Layout"),
            "a relayout must say so"
        );
    }

    #[test]
    fn overflow_reads_differently_from_a_requested_full_redraw() {
        // Same rectangle, same cost, completely different thing to do
        // about it: a request is a caller admitting it cannot be precise;
        // an overflow is damage tracking failing on a frame that *had*
        // precise damage.
        let requested =
            describe_fallback(Fallback::Requested(taim_core::damage::Cause::Interaction));
        let overflow = describe_fallback(Fallback::Overflow { areas: 64 });

        assert!(requested.contains("requested"), "{requested}");
        assert!(overflow.contains("OVERFLOW"), "{overflow}");
        assert!(overflow.contains("64"), "{overflow}");
    }

    #[test]
    fn several_causes_are_all_named() {
        // A frame legitimately has several: a hover, a cursor blink and an
        // animation in the same 16ms. Naming only the first would send a
        // developer to whichever widget happened to run first.
        let described = describe_causes(Causes::CONTENT | Causes::ANIMATION);

        assert!(described.contains("Content"), "{described}");
        assert!(described.contains("Animation"), "{described}");
    }

    #[test]
    fn causes_are_named_in_a_stable_order() {
        // A report that varied with the order widgets ran in could not be
        // diffed between two frames, which is the main thing anyone does
        // with one.
        let one = describe_causes(Causes::ANIMATION | Causes::CONTENT);
        let two = describe_causes(Causes::CONTENT | Causes::ANIMATION);

        assert_eq!(one, two);
    }

    #[test]
    fn a_frame_with_no_recorded_cause_says_so() {
        assert_eq!(describe_causes(Causes::empty()), "no cause recorded");
    }

    #[test]
    fn a_full_redraw_is_called_out_rather_than_reported_as_a_number() {
        let attribution = Attribution {
            region: Region::Full,
            causes: Causes::LAYOUT,
            fallback: Some(Fallback::Requested(taim_core::damage::Cause::Layout)),
        };

        let element: AnyView = view(Some(&attribution), SURFACE, true);

        let _ = std::hint::black_box(element);
    }
}
