//! A month calendar for choosing a date.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::{date_picker, date::Date};
//!
//! #[derive(Clone)]
//! enum Message {
//!     DateChosen(Date),
//!     MonthChanged(Date),
//! }
//!
//! # struct State { selected: Date, visible: Date }
//! fn view(state: &State) -> Element<'_, Message> {
//!     date_picker(state.visible)
//!         .selected(state.selected)
//!         .on_select(Message::DateChosen)
//!         .on_month_change(Message::MonthChanged)
//!         .into()
//! }
//! ```
//!
//! # The visible month and the chosen date are separate
//!
//! Paging to March does not choose a date in March, so the two are
//! independent inputs. A picker that conflated them would either change
//! the value as the user browsed, or refuse to show a month other than
//! the one already chosen.
//!
//! # Known gap: per-day semantics
//!
//! The grid is one widget, so it reports one node — `Role::Group` named
//! with the chosen date. A screen-reader user can hear what is selected
//! but cannot yet walk the grid cell by cell, which needs each day to be
//! a widget of its own. That is a real limitation and is recorded here
//! rather than left to be discovered.

use crate::core::border::Border;
use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::touch;
use crate::core::widget::tree::{self, Tree};
use crate::core::{
    Background, Color, Element, Event, Layout, Length, Point, Rectangle, Shell, Size, Theme, Widget,
};

use crate::date::{Date, Weekday, days_in_month, month_name};

/// The header's height, sized so the paging controls clear the
/// touch-target policy.
const HEADER_HEIGHT: f32 = 48.0;

/// The height of the weekday heading row.
const HEADINGS_HEIGHT: f32 = 24.0;

/// The height of one week's row of days.
const ROW_HEIGHT: f32 = 40.0;

/// The most week rows a month can span: a 31-day month starting on a
/// Sunday reaches into a sixth row.
const MAX_ROWS: usize = 6;

/// Columns in the grid, one per weekday.
const COLUMNS: usize = 7;

/// The width of a paging control at either end of the header.
const PAGE_BUTTON_WIDTH: f32 = 48.0;

/// The size of the chevrons drawn in those controls.
const CHEVRON: f32 = 10.0;

/// The stroke weight of the chevrons.
const CHEVRON_STROKE: f32 = 2.0;

/// Which paging control a press landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Previous,
    Next,
}

/// A month calendar for choosing a date.
#[allow(missing_debug_implementations)]
pub struct DatePicker<'a, Message, Theme = crate::Theme>
where
    Theme: Catalog,
{
    visible: Date,
    selected: Option<Date>,
    today: Option<Date>,
    on_select: Option<Box<dyn Fn(Date) -> Message + 'a>>,
    on_month_change: Option<Box<dyn Fn(Date) -> Message + 'a>>,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme> DatePicker<'a, Message, Theme>
where
    Theme: Catalog,
{
    /// Creates a picker showing the month `visible` falls in.
    pub fn new(visible: Date) -> Self {
        Self {
            visible,
            selected: None,
            today: None,
            on_select: None,
            on_month_change: None,
            class: Theme::default(),
        }
    }

    /// Sets the chosen date.
    pub fn selected(mut self, selected: Date) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Marks a date as today, drawn with an outline.
    ///
    /// Passed in rather than read from the clock: a widget that consulted
    /// the system time would make every golden test depend on the day it
    /// ran.
    pub fn today(mut self, today: Date) -> Self {
        self.today = Some(today);
        self
    }

    /// Produces a message when a day is chosen.
    pub fn on_select(mut self, on_select: impl Fn(Date) -> Message + 'a) -> Self {
        self.on_select = Some(Box::new(on_select));
        self
    }

    /// Produces a message naming the newly visible month when the user
    /// pages.
    pub fn on_month_change(mut self, on_month_change: impl Fn(Date) -> Message + 'a) -> Self {
        self.on_month_change = Some(Box::new(on_month_change));
        self
    }

    /// Sets the style of the picker.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Which column the visible month's first day falls in.
    pub fn leading_blanks(&self) -> u32 {
        self.visible.first_of_month().weekday().index()
    }

    /// How many week rows the visible month spans.
    ///
    /// Between four and six. Four is February in a common year beginning
    /// on a Monday; six is a 31-day month beginning on a Saturday or
    /// Sunday. The [`MAX_ROWS`] clamp is belt and braces -- the
    /// arithmetic cannot exceed it for any Gregorian month, and a golden
    /// oracle proves that across several years -- but a grid that grew a
    /// seventh row would silently clip the end of the month rather than
    /// fail, so it is capped here rather than trusted.
    pub fn rows(&self) -> usize {
        let days = days_in_month(self.visible.year, self.visible.month);
        let cells = self.leading_blanks() + days;

        (cells.div_ceil(COLUMNS as u32) as usize).min(MAX_ROWS)
    }

    /// The day at a grid position, or `None` for a leading or trailing
    /// blank.
    pub fn day_at(&self, row: usize, column: usize) -> Option<u32> {
        if column >= COLUMNS {
            return None;
        }

        let cell = row * COLUMNS + column;
        let blanks = self.leading_blanks() as usize;

        let day = (cell.checked_sub(blanks)? + 1) as u32;

        (day <= days_in_month(self.visible.year, self.visible.month)).then_some(day)
    }

    /// The height the picker needs for the visible month.
    fn height(&self) -> f32 {
        HEADER_HEIGHT + HEADINGS_HEIGHT + self.rows() as f32 * ROW_HEIGHT
    }

    /// A paging control's bounds.
    fn page_bounds(&self, bounds: Rectangle, page: Page) -> Rectangle {
        let x = match page {
            Page::Previous => bounds.x,
            Page::Next => bounds.x + bounds.width - PAGE_BUTTON_WIDTH,
        };

        Rectangle {
            x,
            y: bounds.y,
            width: PAGE_BUTTON_WIDTH,
            height: HEADER_HEIGHT,
        }
    }

    /// One grid cell's bounds.
    fn cell_bounds(&self, bounds: Rectangle, row: usize, column: usize) -> Rectangle {
        let width = bounds.width / COLUMNS as f32;

        Rectangle {
            x: bounds.x + column as f32 * width,
            y: bounds.y + HEADER_HEIGHT + HEADINGS_HEIGHT + row as f32 * ROW_HEIGHT,
            width,
            height: ROW_HEIGHT,
        }
    }

    /// The grid cell a position falls in, if any.
    fn cell_at(&self, bounds: Rectangle, position: Point) -> Option<(usize, usize)> {
        let grid_top = bounds.y + HEADER_HEIGHT + HEADINGS_HEIGHT;

        if position.y < grid_top {
            return None;
        }

        let row = ((position.y - grid_top) / ROW_HEIGHT) as usize;
        let column = ((position.x - bounds.x) / (bounds.width / COLUMNS as f32)) as usize;

        (row < self.rows() && column < COLUMNS).then_some((row, column))
    }

    /// The accessible name: what is chosen, or what month is showing.
    fn announcement(&self) -> String {
        match self.selected {
            Some(date) => format!("{} {} {}", date.day, month_name(date.month), date.year),
            None => format!(
                "{} {}, no date selected",
                month_name(self.visible.month),
                self.visible.year
            ),
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for DatePicker<'_, Message, Theme>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(self.height()))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let height = self.height();

        layout::Node::new(limits.resolve(
            Length::Fill,
            Length::Fixed(height),
            Size::new(0.0, height),
        ))
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        let enabled = self.on_select.is_some();

        Some(
            Node::new(tree.id().clone(), layout.bounds(), Role::Group)
                .with_name(self.announcement().as_str())
                .with_state(if enabled {
                    SemanticState::FOCUSABLE
                } else {
                    SemanticState::DISABLED
                })
                .with_actions(if enabled {
                    Actions::FOCUS | Actions::SET_VALUE
                } else {
                    Actions::empty()
                }),
        )
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if !matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. })
        ) {
            return;
        }

        let bounds = layout.bounds();

        let Some(position) = cursor.position_over(bounds) else {
            return;
        };

        // Paging first: the controls sit in the header, above the grid.
        if let Some(on_month_change) = &self.on_month_change {
            for page in [Page::Previous, Page::Next] {
                if self.page_bounds(bounds, page).contains(position) {
                    let moved = match page {
                        Page::Previous => self.visible.previous_month(),
                        Page::Next => self.visible.next_month(),
                    };

                    shell.publish(on_month_change(moved));
                    shell.capture_event();
                    return;
                }
            }
        }

        let Some(on_select) = &self.on_select else {
            return;
        };

        if let Some((row, column)) = self.cell_at(bounds, position)
            && let Some(day) = self.day_at(row, column)
        {
            shell.publish(on_select(Date::new(
                self.visible.year,
                self.visible.month,
                day,
            )));

            shell.capture_event();
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();

        let Some(position) = cursor.position_over(bounds) else {
            return mouse::Interaction::None;
        };

        let on_page = self.on_month_change.is_some()
            && [Page::Previous, Page::Next]
                .into_iter()
                .any(|page| self.page_bounds(bounds, page).contains(position));

        let on_day = self.on_select.is_some()
            && self
                .cell_at(bounds, position)
                .and_then(|(row, column)| self.day_at(row, column))
                .is_some();

        if on_page || on_day {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let appearance = theme.style(&self.class);
        let bounds = layout.bounds();

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: appearance.border,
                ..renderer::Quad::default()
            },
            appearance.background,
        );

        // The paging chevrons.
        for page in [Page::Previous, Page::Next] {
            let button = self.page_bounds(bounds, page);
            let centre = Point::new(
                button.x + button.width / 2.0,
                button.y + button.height / 2.0,
            );

            let direction = if page == Page::Previous { -1.0 } else { 1.0 };

            for arm in [-1.0_f32, 1.0] {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: centre.x + direction * CHEVRON / 4.0 - CHEVRON_STROKE / 2.0,
                            y: centre.y + arm * CHEVRON / 4.0 - CHEVRON / 4.0,
                            width: CHEVRON_STROKE,
                            height: CHEVRON / 2.0,
                        },
                        ..renderer::Quad::default()
                    },
                    Background::Color(appearance.chevron_color),
                );
            }
        }

        // The day cells. Selection and today are drawn differently on
        // purpose: a filled disc for the choice, an outline for today, so
        // the two are distinguishable when they land on the same day.
        for row in 0..self.rows() {
            for column in 0..COLUMNS {
                let Some(day) = self.day_at(row, column) else {
                    continue;
                };

                let cell = self.cell_bounds(bounds, row, column);
                let date = Date::new(self.visible.year, self.visible.month, day);

                let disc = Rectangle {
                    x: cell.x + (cell.width - ROW_HEIGHT) / 2.0,
                    y: cell.y,
                    width: ROW_HEIGHT,
                    height: ROW_HEIGHT,
                };

                if self.selected == Some(date) {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: disc,
                            border: Border {
                                radius: (ROW_HEIGHT / 2.0).into(),
                                ..Border::default()
                            },
                            ..renderer::Quad::default()
                        },
                        appearance.selected,
                    );
                } else if self.today == Some(date) {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: disc,
                            border: Border {
                                radius: (ROW_HEIGHT / 2.0).into(),
                                width: 1.0,
                                color: appearance.today_outline,
                            },
                            ..renderer::Quad::default()
                        },
                        Background::Color(Color::TRANSPARENT),
                    );
                }
            }
        }
    }
}

impl<'a, Message, Theme, Renderer> From<DatePicker<'a, Message, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(picker: DatePicker<'a, Message, Theme>) -> Self {
        Self::new(picker)
    }
}

/// The appearance of a [`DatePicker`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The calendar's fill.
    pub background: Background,
    /// The disc behind the chosen day.
    pub selected: Background,
    /// The ring around today.
    pub today_outline: Color,
    /// The colour of the paging chevrons.
    pub chevron_color: Color,
    /// The colour of the day numbers.
    pub text_color: Color,
    /// The border around the calendar.
    pub border: Border,
}

/// The theme catalog of a [`DatePicker`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`DatePicker`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// The default styling of a [`DatePicker`] for aura's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Background::Color(palette.background.base.color),
        selected: Background::Color(palette.primary.base.color),
        today_outline: palette.primary.base.color,
        chevron_color: palette.background.base.text,
        text_color: palette.background.base.text,
        border: Border {
            radius: 12.0.into(),
            ..Border::default()
        },
    }
}

/// The weekday column headings, Monday first.
pub fn headings() -> [&'static str; COLUMNS] {
    [
        Weekday::Monday.short_name(),
        Weekday::Tuesday.short_name(),
        Weekday::Wednesday.short_name(),
        Weekday::Thursday.short_name(),
        Weekday::Friday.short_name(),
        Weekday::Saturday.short_name(),
        Weekday::Sunday.short_name(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calendar<'a>(year: i32, month: u32) -> DatePicker<'a, ()> {
        DatePicker::new(Date::new(year, month, 1))
    }

    #[test]
    fn a_month_starting_on_monday_has_no_leading_blanks() {
        // 1 January 2024 was a Monday.
        assert_eq!(calendar(2024, 1).leading_blanks(), 0);
        assert_eq!(calendar(2024, 1).day_at(0, 0), Some(1));
    }

    #[test]
    fn a_month_starting_midweek_is_offset_into_its_first_row() {
        // 1 August 2026 is a Saturday, the sixth column.
        let picker = calendar(2026, 8);

        assert_eq!(picker.leading_blanks(), 5);
        assert_eq!(picker.day_at(0, 4), None);
        assert_eq!(picker.day_at(0, 5), Some(1));
        assert_eq!(picker.day_at(0, 6), Some(2));
        assert_eq!(picker.day_at(1, 0), Some(3));
    }

    #[test]
    fn the_grid_stops_at_the_end_of_the_month() {
        let picker = calendar(2024, 2); // 29 days, starts Thursday.

        assert_eq!(picker.day_at(0, 3), Some(1));
        assert_eq!(picker.day_at(0, 2), None);

        // Nothing past the 29th.
        let mut last = None;

        for row in 0..picker.rows() {
            for column in 0..COLUMNS {
                if let Some(day) = picker.day_at(row, column) {
                    last = Some(day);
                }
            }
        }

        assert_eq!(last, Some(29));
    }

    #[test]
    fn every_day_of_the_month_appears_exactly_once() {
        for (year, month) in [(2024, 1), (2024, 2), (2023, 2), (2026, 8), (2025, 12)] {
            let picker = calendar(year, month);

            let mut days = Vec::new();

            for row in 0..picker.rows() {
                for column in 0..COLUMNS {
                    days.extend(picker.day_at(row, column));
                }
            }

            let expected: Vec<u32> = (1..=days_in_month(year, month)).collect();

            assert_eq!(days, expected, "grid wrong for {year}-{month}");
        }
    }

    #[test]
    fn a_long_month_starting_late_needs_six_rows() {
        // March 2025: 31 days beginning on a Saturday, so five leading
        // blanks plus 31 days is 36 cells and spills into a sixth row.
        assert_eq!(calendar(2025, 3).rows(), 6);

        // One day shorter, same starting weekday, and five rows is
        // enough -- 35 cells fit exactly. This is the boundary the row
        // count has to get right.
        assert_eq!(calendar(2024, 6).rows(), 5);
    }

    #[test]
    fn a_short_month_starting_on_monday_needs_only_four_rows() {
        // February 2021: 28 days beginning Monday, exactly four weeks.
        assert_eq!(calendar(2021, 2).rows(), 4);
    }

    #[test]
    fn a_column_past_the_week_is_never_a_day() {
        assert_eq!(calendar(2024, 1).day_at(0, COLUMNS), None);
        assert_eq!(calendar(2024, 1).day_at(0, 99), None);
    }

    #[test]
    fn the_grid_never_exceeds_six_rows() {
        // Every month of a leap year and a common year.
        for year in [2023, 2024] {
            for month in 1..=12 {
                assert!(
                    calendar(year, month).rows() <= MAX_ROWS,
                    "{year}-{month} needs more than {MAX_ROWS} rows"
                );
            }
        }
    }

    #[test]
    fn paging_controls_sit_at_either_end_of_the_header() {
        let picker = calendar(2024, 1);
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 350.0,
            height: picker.height(),
        };

        let previous = picker.page_bounds(bounds, Page::Previous);
        let next = picker.page_bounds(bounds, Page::Next);

        assert!(previous.width >= 44.0 && previous.height >= 44.0);
        assert_eq!(previous.x, bounds.x);
        assert_eq!(next.x + next.width, bounds.x + bounds.width);
        assert!(previous.x + previous.width <= next.x);
    }

    #[test]
    fn a_press_in_the_header_is_not_a_day() {
        let picker = calendar(2024, 1);
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 350.0,
            height: picker.height(),
        };

        assert_eq!(picker.cell_at(bounds, Point::new(175.0, 10.0)), None);
    }

    #[test]
    fn a_press_in_the_grid_resolves_to_its_cell() {
        let picker = calendar(2024, 1);
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 350.0,
            height: picker.height(),
        };

        let grid_top = HEADER_HEIGHT + HEADINGS_HEIGHT;

        assert_eq!(
            picker.cell_at(bounds, Point::new(25.0, grid_top + 5.0)),
            Some((0, 0))
        );
        assert_eq!(
            picker.cell_at(bounds, Point::new(25.0, grid_top + ROW_HEIGHT + 5.0)),
            Some((1, 0))
        );
    }

    #[test]
    fn the_announcement_names_the_chosen_date() {
        let picker = calendar(2026, 8).selected(Date::new(2026, 8, 13));

        assert_eq!(picker.announcement(), "13 August 2026");
    }

    #[test]
    fn an_unset_picker_says_so_rather_than_staying_silent() {
        assert_eq!(
            calendar(2026, 8).announcement(),
            "August 2026, no date selected"
        );
    }

    #[test]
    fn there_are_seven_headings() {
        assert_eq!(headings().len(), COLUMNS);
    }
}
