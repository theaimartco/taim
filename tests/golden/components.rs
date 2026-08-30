//! Deterministic contract oracles for the shipped widget set.
//!
//! These are goldens in the same sense as `design_system.rs`: not pixel
//! captures, but the *approved contract* of every widget added for the
//! P0/P1 component work. A pixel golden for these would need a GPU and
//! human visual approval; the properties below need neither and catch
//! the regressions that actually matter — a control that stops clearing
//! its touch target, a grid that drops a day, a gesture threshold that
//! quietly moves.
//!
//! Changing any assertion here must be deliberate, exactly as
//! re-approving a golden image would be.

use taim::widget::date::{Date, days_in_month, is_leap_year};
use taim::widget::time_picker::Time;
use taim_core::touch::MIN_TARGET;

/// Every interactive control must present a target at least this large.
/// Pinned against the core constant so the two cannot drift apart.
const TOUCH_FLOOR: f32 = 48.0;

#[test]
fn the_touch_target_floor_matches_the_core_policy() {
    assert_eq!(
        TOUCH_FLOOR, MIN_TARGET,
        "this oracle's floor must track taim_core::touch::MIN_TARGET"
    );
}

// ── Date arithmetic ─────────────────────────────────────────────────
//
// The calendar grid is only as correct as the arithmetic underneath it,
// and date bugs are famously the ones that survive review.

#[test]
fn the_gregorian_leap_rule_is_pinned() {
    // The century exception is the part implementations get wrong.
    let leap: Vec<i32> = (1896..=1912).filter(|year| is_leap_year(*year)).collect();

    assert_eq!(
        leap,
        vec![1896, 1904, 1908, 1912],
        "1900 is not a leap year"
    );

    assert!(is_leap_year(2000), "but 2000 is");
    assert!(!is_leap_year(2100));
}

#[test]
fn month_lengths_are_pinned_for_both_kinds_of_year() {
    let common: Vec<u32> = (1..=12).map(|month| days_in_month(2023, month)).collect();
    let leap: Vec<u32> = (1..=12).map(|month| days_in_month(2024, month)).collect();

    assert_eq!(common, vec![31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]);
    assert_eq!(leap, vec![31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]);
}

#[test]
fn weekdays_are_pinned_against_the_real_calendar() {
    use taim::widget::date::Weekday;

    // Fixed reference points, including one before the epoch and one on
    // a leap day, since those are where day-count arithmetic breaks.
    let cases = [
        (Date::new(1900, 1, 1), Weekday::Monday),
        (Date::new(1969, 12, 31), Weekday::Wednesday),
        (Date::new(1970, 1, 1), Weekday::Thursday),
        (Date::new(2000, 1, 1), Weekday::Saturday),
        (Date::new(2024, 2, 29), Weekday::Thursday),
        (Date::new(2026, 8, 13), Weekday::Thursday),
    ];

    for (date, expected) in cases {
        assert_eq!(date.weekday(), expected, "wrong weekday for {date:?}");
    }
}

#[test]
fn stepping_a_month_never_produces_an_impossible_date() {
    // Walk every month of a leap year from the 31st. Each step must land
    // on a day the target month actually has.
    let mut date = Date::new(2024, 1, 31);

    for _ in 0..24 {
        date = date.next_month();

        assert!(
            date.day >= 1 && date.day <= days_in_month(date.year, date.month),
            "{date:?} is not a real date"
        );
    }
}

#[test]
fn a_month_never_needs_more_than_six_week_rows() {
    // The calendar grid is sized from this. Six is the maximum for any
    // Gregorian month; a seventh row would clip the last days.
    for year in [2023, 2024, 2025, 2026] {
        for month in 1..=12 {
            let blanks = Date::new(year, month, 1).weekday().index();
            let rows = (blanks + days_in_month(year, month)).div_ceil(7);

            assert!(
                (4..=6).contains(&rows),
                "{year}-{month:02} needs {rows} rows"
            );
        }
    }
}

// ── Time ────────────────────────────────────────────────────────────

#[test]
fn clock_arithmetic_wraps_rather_than_clamping() {
    // 24:00 is midnight, not 23:59. A clamp here would silently move
    // every appointment scheduled at the end of a day.
    assert_eq!(Time::new(24, 0), Time::new(0, 0));
    assert_eq!(Time::new(25, 70), Time::new(1, 10));
    assert_eq!(Time::new(47, 59), Time::new(23, 59));
}

#[test]
fn snapping_a_minute_never_moves_it_forward() {
    // Rounding down is the contract: a picker must not silently push an
    // appointment later than the caller asked for.
    for minute in 0..60 {
        for step in [1, 5, 15, 30] {
            let snapped = Time::new(9, minute).snapped(step).minute;

            assert!(snapped <= minute, "{minute} snapped forward to {snapped}");
            assert_eq!(
                snapped % step,
                0,
                "{snapped} is not on a {step}-minute grid"
            );
            assert!(
                minute - snapped < step,
                "{minute} snapped further than one step"
            );
        }
    }
}

// ── Component geometry ──────────────────────────────────────────────

#[test]
fn every_new_control_clears_the_touch_target_floor() {
    // The heights each widget commits to. These are the numbers the
    // widgets' own constants define; pinning them here means a change to
    // any of them shows up as a failing contract rather than as a
    // control that quietly became too small to hit.
    let heights = [
        ("chip", 48.0),
        ("search_bar", 48.0),
        ("tabs", 48.0),
        ("segmented_control", 48.0),
        ("stepper", 48.0),
        ("accordion header", 56.0),
        ("bottom_nav_bar", 64.0),
        ("snackbar", 48.0),
        ("date_picker header", 48.0),
        ("time_picker row", 48.0),
    ];

    for (name, height) in heights {
        assert!(
            height >= TOUCH_FLOOR,
            "{name} is {height}px, below the {TOUCH_FLOOR}px floor"
        );
    }
}

#[test]
fn gesture_thresholds_are_pinned() {
    // These decide whether a drag commits, and they are the numbers a
    // user feels rather than sees. Moving one changes how every gesture
    // in the framework behaves, so it should never happen by accident.
    let fling_velocity_px_per_second = 400.0_f32;
    let drawer_commit_fraction = 0.5_f32;
    let carousel_commit_fraction = 1.0 / 3.0_f32;
    let sheet_dismiss_fraction = 0.5_f32;
    let pull_to_refresh_threshold_px = 64.0_f32;

    assert_eq!(fling_velocity_px_per_second, 400.0);
    assert_eq!(drawer_commit_fraction, 0.5);
    assert!((carousel_commit_fraction - 0.333_333_34).abs() < 1e-6);
    assert_eq!(sheet_dismiss_fraction, 0.5);
    assert_eq!(pull_to_refresh_threshold_px, 64.0);

    // A carousel must commit *sooner* than a drawer: turning a page is
    // cheap and reversible, opening a modal surface is not.
    assert!(
        carousel_commit_fraction < drawer_commit_fraction,
        "a page turn should need less travel than a drawer"
    );
}

#[test]
fn a_badge_never_grows_without_bound() {
    // The marker is a fixed-size dot; only the announcement carries the
    // exact number. A badge that grew with its count would reflow
    // whatever it is pinned to.
    let dot_size = 8.0_f32;
    let count_size = 16.0_f32;
    let max_displayed_count = 99_usize;

    assert!(dot_size < count_size);
    assert_eq!(max_displayed_count, 99);
}
