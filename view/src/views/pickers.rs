//! Calendar and clock pickers.
use super::look::{self, Gesture, Label};
use super::{Align, button, column, row, text};
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::Constraints;
use crate::state::{Prop, State};
use crate::{AnyView, Border, Event, Handled, Point, Rectangle, Size, View, mouse, semantics};

use taim_core::widget;

/// A calendar day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    /// Year, e.g. 2026.
    pub year: i32,
    /// Month, 1 to 12.
    pub month: u8,
    /// Day of the month, from 1.
    pub day: u8,
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

impl Date {
    /// A date, with month and day brought into range.
    pub fn new(year: i32, month: u8, day: u8) -> Self {
        let month = month.clamp(1, 12);
        let day = day.clamp(1, days_in_month(year, month));
        Self { year, month, day }
    }

    /// The first day of this date's month.
    pub fn first_of_month(self) -> Self {
        Self { day: 1, ..self }
    }

    /// The same day `months` months later (earlier when negative), clamped
    /// to the target month's length.
    pub fn add_months(self, months: i32) -> Self {
        let index = self.year * 12 + i32::from(self.month) - 1 + months;
        let (year, month) = (index.div_euclid(12), index.rem_euclid(12) as u8 + 1);
        Self::new(year, month, self.day)
    }

    /// Day of the week, 0 for Monday to 6 for Sunday.
    pub fn weekday(self) -> u8 {
        // 1970-01-01 was a Thursday (3).
        ((days_from_epoch(self) + 3).rem_euclid(7)) as u8
    }

    /// The month's English name.
    pub fn month_name(self) -> &'static str {
        MONTHS[usize::from(self.month - 1)]
    }
}

/// Whether `year` has a 29th of February.
pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Number of days in `month` of `year`.
pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 (proleptic Gregorian).
fn days_from_epoch(date: Date) -> i64 {
    // Shift the year to start in March so the leap day is the last day.
    let (year, month) = if date.month <= 2 {
        (i64::from(date.year) - 1, i64::from(date.month) + 9)
    } else {
        (i64::from(date.year), i64::from(date.month) - 3)
    };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * month + 2) / 5 + i64::from(date.day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// A month calendar: arrows move `visible` between months; clicking a day
/// sets `chosen`.
pub fn date_picker(visible: State<Date>, chosen: State<Date>) -> DatePicker {
    DatePicker {
        visible,
        shown_month: crate::IntoProp::into_prop(visible),
        chosen,
        shown_choice: crate::IntoProp::into_prop(chosen),
        today: None,
        title: Label::new("", Some(15.0)),
        weekdays: ["M", "T", "W", "T", "F", "S", "S"].map(|day| Label::new(day, Some(11.0))),
        days: (1..=31)
            .map(|day| Label::new(day.to_string(), Some(13.0)))
            .collect(),
        arrows: [Label::new("‹", Some(20.0)), Label::new("›", Some(20.0))],
        pressed: None,
        id: widget::Id::unique(),
    }
}

/// See [`date_picker`].
pub struct DatePicker {
    visible: State<Date>,
    shown_month: Prop<Date>,
    chosen: State<Date>,
    shown_choice: Prop<Date>,
    today: Option<Date>,
    title: Label,
    weekdays: [Label; 7],
    days: Vec<Label>,
    arrows: [Label; 2],
    pressed: Option<Target>,
    id: widget::Id,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Previous,
    Next,
    Day(u8),
}

const CELL: f32 = 36.0;
const TITLE: f32 = 40.0;

impl DatePicker {
    /// Rings today's date.
    pub fn today(mut self, today: Date) -> Self {
        self.today = Some(today);
        self
    }

    fn cell(bounds: Rectangle, slot: usize) -> Rectangle {
        let (row, column) = (slot / 7, slot % 7);
        Rectangle::new(
            Point::new(
                bounds.x + column as f32 * CELL,
                bounds.y + TITLE + 20.0 + row as f32 * CELL,
            ),
            Size::new(CELL, CELL),
        )
    }

    fn arrow(bounds: Rectangle, next: bool) -> Rectangle {
        let x = if next {
            bounds.x + bounds.width - CELL
        } else {
            bounds.x
        };
        Rectangle::new(Point::new(x, bounds.y), Size::new(CELL, TITLE))
    }

    fn target_at(&self, bounds: Rectangle, at: Point) -> Option<Target> {
        if Self::arrow(bounds, false).contains(at) {
            return Some(Target::Previous);
        }
        if Self::arrow(bounds, true).contains(at) {
            return Some(Target::Next);
        }

        let month = self.shown_month.get().first_of_month();
        let offset = usize::from(month.weekday());
        let length = usize::from(days_in_month(month.year, month.month));
        (0..length).find_map(|index| {
            Self::cell(bounds, offset + index)
                .contains(at)
                .then_some(Target::Day(index as u8 + 1))
        })
    }
}

impl View for DatePicker {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _constraints: Constraints) -> Size {
        let month = self.shown_month.get();
        self.title
            .set(&format!("{} {}", month.month_name(), month.year));
        let _ = self.title.measure(cx);
        for label in self
            .weekdays
            .iter_mut()
            .chain(&mut self.days)
            .chain(&mut self.arrows)
        {
            let _ = label.measure(cx);
        }
        Size::new(CELL * 7.0, TITLE + 20.0 + CELL * 6.0)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let month = self.shown_month.get().first_of_month();
        let chosen = self.shown_choice.get();
        let ink = cx.text_color();
        let quiet = palette.background.strongest.color;

        let header = Rectangle::new(bounds.position(), Size::new(bounds.width, TITLE));
        self.title.paint_centered(cx, header, ink);
        self.arrows[0].paint_centered(cx, Self::arrow(bounds, false), palette.primary.base.color);
        self.arrows[1].paint_centered(cx, Self::arrow(bounds, true), palette.primary.base.color);

        for (column, label) in self.weekdays.iter().enumerate() {
            let area = Rectangle::new(
                Point::new(bounds.x + column as f32 * CELL, bounds.y + TITLE),
                Size::new(CELL, 20.0),
            );
            label.paint_centered(cx, area, quiet);
        }

        let offset = usize::from(month.weekday());
        for day in 1..=days_in_month(month.year, month.month) {
            let date = Date { day, ..month };
            let area = Self::cell(bounds, offset + usize::from(day) - 1);
            let circle = area.shrink(3.0);

            let color = if date == chosen {
                cx.fill(
                    circle,
                    palette.primary.base.color,
                    Border::default().rounded(CELL / 2.0),
                );
                palette.primary.base.text
            } else {
                if Some(date) == self.today {
                    cx.fill(
                        circle,
                        crate::Color::TRANSPARENT,
                        Border::default()
                            .width(1.0)
                            .color(palette.primary.base.color)
                            .rounded(CELL / 2.0),
                    );
                }
                ink
            };
            self.days[usize::from(day) - 1].paint_centered(cx, area, color);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let bounds = cx.bounds();

        match look::gesture(event, cx.pointer()) {
            Some(Gesture::Down(at)) => {
                self.pressed = self.target_at(bounds, at);
                Handled::from(self.pressed.is_some())
            }
            Some(Gesture::Up(at)) => {
                let Some(pressed) = self.pressed.take() else {
                    return Handled::No;
                };
                if at.and_then(|at| self.target_at(bounds, at)) == Some(pressed) {
                    match pressed {
                        Target::Previous => {
                            self.visible.update(|month| *month = month.add_months(-1));
                        }
                        Target::Next => self.visible.update(|month| *month = month.add_months(1)),
                        Target::Day(day) => {
                            let month = self.shown_month.get();
                            let _ = self.chosen.set(Date { day, ..month });
                        }
                    }
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        let chosen = self.shown_choice.get();
        Some(
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::Group).with_name(
                format!(
                    "Calendar, chosen {} {} {}",
                    chosen.day,
                    chosen.month_name(),
                    chosen.year
                ),
            ),
        )
    }

    fn cursor(&self) -> Option<mouse::Interaction> {
        Some(mouse::Interaction::Pointer)
    }
}

/// A time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time {
    /// Hour, 0 to 23.
    pub hour: u8,
    /// Minute, 0 to 59.
    pub minute: u8,
}

impl Time {
    /// A time, with hour and minute brought into range.
    pub fn new(hour: u8, minute: u8) -> Self {
        Self {
            hour: hour.min(23),
            minute: minute.min(59),
        }
    }

    /// The time `minutes` later (earlier when negative), wrapping at
    /// midnight.
    pub fn add_minutes(self, minutes: i32) -> Self {
        let total =
            (i32::from(self.hour) * 60 + i32::from(self.minute) + minutes).rem_euclid(24 * 60);
        Self::new((total / 60) as u8, (total % 60) as u8)
    }
}

/// Hour and minute steppers bound to `time`; minutes move by `step`.
pub fn time_picker(time: State<Time>, step: u8) -> AnyView {
    let step = i32::from(step.max(1));
    let shift = move |minutes: i32| move || time.update(|time| *time = time.add_minutes(minutes));

    let column_of = |label: &'static str, up: i32, down: i32, show: fn(Time) -> u8| {
        column(crate::views![
            button("▲").on_press(shift(up)),
            text(move || format!("{:02}", show(time.get()))).size(24),
            button("▼").on_press(shift(down)),
            text(label).size(11),
        ])
        .gap(6.0)
        .align(Align::Center)
    };

    row(crate::views![
        column_of("hour", 60, -60, |time| time.hour),
        text(":").size(24),
        column_of("minute", step, -step, |time| time.minute),
    ])
    .gap(12.0)
    .align(Align::Center)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekdays_match_known_dates() {
        assert_eq!(Date::new(1970, 1, 1).weekday(), 3); // Thursday
        assert_eq!(Date::new(2000, 2, 29).weekday(), 1); // Tuesday
        assert_eq!(Date::new(2026, 10, 22).weekday(), 3); // Thursday
    }

    #[test]
    fn months_wrap_and_clamp_days() {
        assert_eq!(Date::new(2026, 1, 31).add_months(1), Date::new(2026, 2, 28));
        assert_eq!(Date::new(2026, 12, 5).add_months(1), Date::new(2027, 1, 5));
        assert_eq!(Date::new(2026, 1, 5).add_months(-1), Date::new(2025, 12, 5));
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
    }

    #[test]
    fn times_wrap_at_midnight() {
        assert_eq!(Time::new(23, 50).add_minutes(15), Time::new(0, 5));
        assert_eq!(Time::new(0, 5).add_minutes(-10), Time::new(23, 55));
    }
}
