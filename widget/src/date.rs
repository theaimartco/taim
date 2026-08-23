//! Proleptic Gregorian calendar arithmetic for the date and time
//! pickers.
//!
//! # Why this exists rather than a dependency
//!
//! The pickers need four things: how many days a month has, which
//! weekday a month starts on, and how to step a month forwards and
//! backwards. That is a few dozen lines of well-understood arithmetic,
//! and pulling a full date-time library into the widget crate for it
//! would cost every application that links a button.
//!
//! This is deliberately *not* a general date library. There is no time
//! zone handling, no parsing, and no formatting, because a calendar grid
//! does not need any of it. An application doing real date work should
//! use a real library and hand the result in as a [`Date`].

/// A date in the proleptic Gregorian calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    /// The year. Negative years are permitted and behave as the
    /// proleptic calendar defines.
    pub year: i32,
    /// The month, `1..=12`.
    pub month: u32,
    /// The day of the month, `1..=31`.
    pub day: u32,
}

/// A day of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Weekday {
    /// Monday, index 0 -- the first column of the calendar grid.
    Monday,
    /// Tuesday.
    Tuesday,
    /// Wednesday.
    Wednesday,
    /// Thursday.
    Thursday,
    /// Friday.
    Friday,
    /// Saturday.
    Saturday,
    /// Sunday.
    Sunday,
}

impl Weekday {
    /// Every weekday, Monday first.
    pub const ALL: [Self; 7] = [
        Self::Monday,
        Self::Tuesday,
        Self::Wednesday,
        Self::Thursday,
        Self::Friday,
        Self::Saturday,
        Self::Sunday,
    ];

    /// The column this weekday occupies, counting from Monday.
    pub const fn index(self) -> u32 {
        match self {
            Self::Monday => 0,
            Self::Tuesday => 1,
            Self::Wednesday => 2,
            Self::Thursday => 3,
            Self::Friday => 4,
            Self::Saturday => 5,
            Self::Sunday => 6,
        }
    }

    /// A one-or-two letter heading for the calendar's column.
    pub const fn short_name(self) -> &'static str {
        match self {
            Self::Monday => "M",
            Self::Tuesday => "T",
            Self::Wednesday => "W",
            Self::Thursday => "T",
            Self::Friday => "F",
            Self::Saturday => "S",
            Self::Sunday => "S",
        }
    }
}

impl Date {
    /// Creates a date, clamping the day into the month's real length.
    ///
    /// Clamping rather than rejecting: the 31st of January stepped to
    /// February has to become the 28th or 29th, and every caller would
    /// otherwise have to implement that themselves.
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        let month = month.clamp(1, 12);
        let day = day.clamp(1, days_in_month(year, month));

        Self { year, month, day }
    }

    /// The first day of this date's month.
    pub fn first_of_month(self) -> Self {
        Self { day: 1, ..self }
    }

    /// The same day one month earlier, clamped into the shorter month.
    pub fn previous_month(self) -> Self {
        let (year, month) = if self.month == 1 {
            (self.year - 1, 12)
        } else {
            (self.year, self.month - 1)
        };

        Self::new(year, month, self.day)
    }

    /// The same day one month later, clamped into the shorter month.
    pub fn next_month(self) -> Self {
        let (year, month) = if self.month == 12 {
            (self.year + 1, 1)
        } else {
            (self.year, self.month + 1)
        };

        Self::new(year, month, self.day)
    }

    /// The weekday this date falls on.
    pub fn weekday(self) -> Weekday {
        // `days_from_civil` puts day 0 at 1970-01-01, which was a
        // Thursday; rebasing by 3 makes Monday the zero column.
        let days = days_from_civil(self.year, self.month, self.day);
        let index = (days + 3).rem_euclid(7);

        Weekday::ALL[index as usize]
    }
}

/// Whether `year` is a leap year in the proleptic Gregorian calendar.
pub const fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// How many days `month` has in `year`.
pub const fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        // Out of range; `Date::new` clamps before this is reached, and
        // returning 31 keeps any other caller's clamp permissive rather
        // than silently truncating a valid day.
        _ => 31,
    }
}

/// The name of `month`, `1..=12`.
pub const fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "",
    }
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
///
/// This is Howard Hinnant's `days_from_civil`, which is exact for the
/// whole range of `i32` years and avoids the iterative year-summing that
/// makes naive implementations both slow and wrong near the epoch.
fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = i64::from(year);
    let month = i64::from(month);
    let day = i64::from(day);

    // Shift the year so it starts in March, which puts the leap day at
    // the end of the year and removes it as a special case.
    let year = if month <= 2 { year - 1 } else { year };

    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;

    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;

    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leap_years_follow_the_gregorian_rule() {
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(2023));

        // The century rule, and the exception to it.
        assert!(!is_leap_year(1900));
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(2100));
        assert!(is_leap_year(2400));
    }

    #[test]
    fn february_knows_its_own_length() {
        assert_eq!(days_in_month(2023, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
    }

    #[test]
    fn every_other_month_has_its_usual_length() {
        let lengths: Vec<u32> = (1..=12).map(|month| days_in_month(2023, month)).collect();

        assert_eq!(
            lengths,
            vec![31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        );
    }

    #[test]
    fn a_day_past_the_end_of_the_month_is_clamped() {
        assert_eq!(Date::new(2023, 2, 31).day, 28);
        assert_eq!(Date::new(2024, 2, 31).day, 29);
        assert_eq!(Date::new(2023, 4, 31).day, 30);
    }

    #[test]
    fn a_month_out_of_range_is_clamped() {
        assert_eq!(Date::new(2024, 0, 15).month, 1);
        assert_eq!(Date::new(2024, 13, 15).month, 12);
    }

    #[test]
    fn stepping_months_wraps_the_year() {
        assert_eq!(
            Date::new(2024, 1, 15).previous_month(),
            Date::new(2023, 12, 15)
        );
        assert_eq!(Date::new(2024, 12, 15).next_month(), Date::new(2025, 1, 15));
    }

    #[test]
    fn stepping_into_a_shorter_month_clamps_the_day() {
        // The case every naive implementation gets wrong.
        assert_eq!(Date::new(2023, 1, 31).next_month(), Date::new(2023, 2, 28));
        assert_eq!(
            Date::new(2024, 3, 31).previous_month(),
            Date::new(2024, 2, 29)
        );
    }

    #[test]
    fn weekdays_match_known_dates() {
        // Checked against the real calendar.
        assert_eq!(Date::new(2024, 1, 1).weekday(), Weekday::Monday);
        assert_eq!(Date::new(2000, 1, 1).weekday(), Weekday::Saturday);
        assert_eq!(Date::new(1970, 1, 1).weekday(), Weekday::Thursday);
        assert_eq!(Date::new(2026, 8, 13).weekday(), Weekday::Thursday);
    }

    #[test]
    fn weekdays_are_correct_across_a_leap_day() {
        assert_eq!(Date::new(2024, 2, 28).weekday(), Weekday::Wednesday);
        assert_eq!(Date::new(2024, 2, 29).weekday(), Weekday::Thursday);
        assert_eq!(Date::new(2024, 3, 1).weekday(), Weekday::Friday);
    }

    #[test]
    fn weekdays_advance_by_one_each_day() {
        // A property rather than a table: walking a whole year must
        // never skip or repeat a column.
        let mut date = Date::new(2024, 1, 1);
        let mut previous = date.weekday().index();

        for _ in 0..365 {
            date = if date.day < days_in_month(date.year, date.month) {
                Date::new(date.year, date.month, date.day + 1)
            } else {
                Date::new(date.year, date.month, 1)
                    .next_month()
                    .first_of_month()
            };

            let current = date.weekday().index();
            assert_eq!(current, (previous + 1) % 7, "weekday skipped at {date:?}");
            previous = current;
        }
    }

    #[test]
    fn weekdays_are_correct_before_the_epoch() {
        // `days_from_civil` has to stay exact for negative day counts,
        // which is where naive division-based implementations break.
        assert_eq!(Date::new(1969, 12, 31).weekday(), Weekday::Wednesday);
        assert_eq!(Date::new(1900, 1, 1).weekday(), Weekday::Monday);
        assert_eq!(Date::new(1, 1, 1).weekday(), Weekday::Monday);
    }

    #[test]
    fn the_first_of_the_month_keeps_its_month() {
        let date = Date::new(2024, 7, 22).first_of_month();

        assert_eq!(date, Date::new(2024, 7, 1));
    }

    #[test]
    fn month_names_cover_the_year() {
        assert_eq!(month_name(1), "January");
        assert_eq!(month_name(12), "December");
        assert_eq!(month_name(13), "");
    }
}
