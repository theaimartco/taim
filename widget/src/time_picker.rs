//! Hour and minute selection.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::{time_picker, time_picker::Time};
//!
//! #[derive(Clone)]
//! enum Message {
//!     TimeChosen(Time),
//! }
//!
//! # struct State { time: Time }
//! fn view(state: &State) -> Element<'_, Message> {
//!     time_picker(state.time)
//!         .minute_step(15)
//!         .on_change(Message::TimeChosen)
//!         .into()
//! }
//! ```
//!
//! # Minutes step, they do not scroll freely
//!
//! Most applications want a quarter hour or five minutes, not sixty
//! individually tappable values. [`TimePicker::minute_step`] sets the
//! granularity and the widget only ever produces minutes on that grid,
//! so an application never has to round what it receives.

use crate::core::alignment;
use crate::core::border::Border;
use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::text;
use crate::core::touch;
use crate::core::widget::tree::{self, Tree};
use crate::core::{
    Background, Color, Element, Event, Layout, Length, Point, Rectangle, Shell, Size, Theme, Widget,
};

/// The height of one selectable row.
const ROW_HEIGHT: f32 = 48.0;

/// How many rows are visible in each column at once.
const VISIBLE_ROWS: usize = 5;

/// The text size of the chosen row.
const SELECTED_SIZE: f32 = 20.0;

/// The text size of the rows either side of it.
const DIM_SIZE: f32 = 16.0;

/// Minutes in an hour, and hours in a day.
const MINUTES_PER_HOUR: u32 = 60;
const HOURS_PER_DAY: u32 = 24;

/// A time of day, to the minute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Time {
    /// The hour, `0..=23`.
    pub hour: u32,
    /// The minute, `0..=59`.
    pub minute: u32,
}

impl Time {
    /// Creates a time, wrapping both fields into range.
    ///
    /// Wrapping rather than clamping: 24:00 is midnight and 25:00 is one
    /// in the morning, which is what arithmetic on a clock means. A
    /// clamp would turn both into 23:59.
    pub const fn new(hour: u32, minute: u32) -> Self {
        Self {
            hour: hour % HOURS_PER_DAY,
            minute: minute % MINUTES_PER_HOUR,
        }
    }

    /// The time rounded down onto a minute grid.
    pub const fn snapped(self, step: u32) -> Self {
        if step == 0 {
            return self;
        }

        Self {
            hour: self.hour,
            minute: self.minute / step * step,
        }
    }
}

/// Which column a press landed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Column {
    Hour,
    Minute,
}

/// Hour and minute selection.
#[allow(missing_debug_implementations)]
pub struct TimePicker<'a, Message, Theme = crate::Theme>
where
    Theme: Catalog,
{
    value: Time,
    minute_step: u32,
    on_change: Option<Box<dyn Fn(Time) -> Message + 'a>>,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme> TimePicker<'a, Message, Theme>
where
    Theme: Catalog,
{
    /// Creates a picker showing `value`.
    pub fn new(value: Time) -> Self {
        Self {
            value,
            minute_step: 1,
            on_change: None,
            class: Theme::default(),
        }
    }

    /// Sets the minute granularity.
    ///
    /// A step of zero or one means every minute. A step that does not
    /// divide an hour is accepted -- the last slot is simply shorter --
    /// but a divisor is what most callers want.
    pub fn minute_step(mut self, step: u32) -> Self {
        self.minute_step = step.clamp(1, MINUTES_PER_HOUR);
        self
    }

    /// Produces a message when either column is changed.
    pub fn on_change(mut self, on_change: impl Fn(Time) -> Message + 'a) -> Self {
        self.on_change = Some(Box::new(on_change));
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

    /// The value, snapped onto the configured minute grid.
    pub fn snapped(&self) -> Time {
        self.value.snapped(self.minute_step)
    }

    /// Every minute this picker can produce.
    pub fn minutes(&self) -> Vec<u32> {
        (0..MINUTES_PER_HOUR)
            .step_by(self.minute_step as usize)
            .collect()
    }

    /// Every hour this picker can produce.
    pub fn hours(&self) -> Vec<u32> {
        (0..HOURS_PER_DAY).collect()
    }

    /// The index of the currently chosen row in a column.
    fn selected_index(&self, column: Column) -> usize {
        match column {
            Column::Hour => self.snapped().hour as usize,
            Column::Minute => {
                let minutes = self.minutes();
                let target = self.snapped().minute;

                minutes
                    .iter()
                    .position(|minute| *minute == target)
                    .unwrap_or(0)
            }
        }
    }

    /// A column's bounds.
    fn column_bounds(&self, bounds: Rectangle, column: Column) -> Rectangle {
        let width = bounds.width / 2.0;

        Rectangle {
            x: bounds.x + if column == Column::Hour { 0.0 } else { width },
            y: bounds.y,
            width,
            height: bounds.height,
        }
    }

    /// The row a position falls on within a column, accounting for the
    /// column being scrolled so the selection sits in the middle.
    fn row_at(&self, bounds: Rectangle, column: Column, position: Point) -> Option<usize> {
        let column_bounds = self.column_bounds(bounds, column);

        if !column_bounds.contains(position) {
            return None;
        }

        let offset = ((position.y - column_bounds.y) / ROW_HEIGHT) as i64;

        // The chosen row is centred, so the row under the pointer is
        // measured relative to it rather than to the top of the column.
        let centre = (VISIBLE_ROWS / 2) as i64;
        let index = self.selected_index(column) as i64 + offset - centre;

        let count = match column {
            Column::Hour => self.hours().len(),
            Column::Minute => self.minutes().len(),
        } as i64;

        (index >= 0 && index < count).then_some(index as usize)
    }

    /// Which column a position falls in.
    fn column_at(&self, bounds: Rectangle, position: Point) -> Option<Column> {
        [Column::Hour, Column::Minute]
            .into_iter()
            .find(|column| self.column_bounds(bounds, *column).contains(position))
    }

    /// The time produced by choosing `index` in `column`.
    fn time_for(&self, column: Column, index: usize) -> Time {
        let current = self.snapped();

        match column {
            Column::Hour => Time::new(index as u32, current.minute),
            Column::Minute => {
                let minutes = self.minutes();

                Time::new(
                    current.hour,
                    minutes.get(index).copied().unwrap_or(current.minute),
                )
            }
        }
    }

    /// The accessible name, as a zero-padded 24-hour clock reading.
    fn announcement(&self) -> String {
        let time = self.snapped();

        format!("{:02}:{:02}", time.hour, time.minute)
    }

    /// The height the picker occupies.
    fn height(&self) -> f32 {
        VISIBLE_ROWS as f32 * ROW_HEIGHT
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for TimePicker<'_, Message, Theme>
where
    Renderer: crate::core::text::Renderer,
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

        let enabled = self.on_change.is_some();

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
        let Some(on_change) = &self.on_change else {
            return;
        };

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

        let Some(column) = self.column_at(bounds, position) else {
            return;
        };

        if let Some(index) = self.row_at(bounds, column, position) {
            shell.publish(on_change(self.time_for(column, index)));
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
        if self.on_change.is_none() {
            return mouse::Interaction::None;
        }

        let bounds = layout.bounds();

        let Some(position) = cursor.position_over(bounds) else {
            return mouse::Interaction::None;
        };

        let hit = self
            .column_at(bounds, position)
            .and_then(|column| self.row_at(bounds, column, position));

        if hit.is_some() {
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
        viewport: &Rectangle,
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

        // The selection band across the middle row of both columns. The
        // values scroll behind it rather than the highlight chasing them,
        // which is what makes the control read as a dial.
        let centre = Rectangle {
            x: bounds.x,
            y: bounds.y + (VISIBLE_ROWS / 2) as f32 * ROW_HEIGHT,
            width: bounds.width,
            height: ROW_HEIGHT,
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds: centre,
                border: Border {
                    radius: 8.0.into(),
                    ..Border::default()
                },
                ..renderer::Quad::default()
            },
            appearance.selection,
        );

        // The hours and minutes. Without these the control rendered as a
        // single grey band and nothing else -- a time picker showing no
        // time. Its renderer bound could not draw text at all.
        let middle = (VISIBLE_ROWS / 2) as i64;

        for column in [Column::Hour, Column::Minute] {
            let column_bounds = self.column_bounds(bounds, column);
            let values = match column {
                Column::Hour => self.hours(),
                Column::Minute => self.minutes(),
            };

            let selected = self.selected_index(column) as i64;

            for offset in -middle..=middle {
                let index = selected + offset;

                // The columns do not wrap visually: an hour before 0 or
                // after 23 is empty space, which is what tells a user they
                // have reached the end of the dial.
                let Ok(index) = usize::try_from(index) else {
                    continue;
                };

                let Some(value) = values.get(index) else {
                    continue;
                };

                let row = Rectangle {
                    x: column_bounds.x,
                    y: column_bounds.y + (middle + offset) as f32 * ROW_HEIGHT,
                    width: column_bounds.width,
                    height: ROW_HEIGHT,
                };

                let is_selected = offset == 0;

                renderer.fill_text(
                    text::Text {
                        content: format!("{value:02}"),
                        font: renderer.default_font(),
                        size: crate::core::Pixels(if is_selected {
                            SELECTED_SIZE
                        } else {
                            DIM_SIZE
                        }),
                        line_height: text::LineHeight::default(),
                        bounds: row.size(),
                        align_x: text::Alignment::Center,
                        align_y: alignment::Vertical::Center,
                        shaping: text::Shaping::Basic,
                        wrapping: text::Wrapping::None,
                        ellipsis: text::Ellipsis::default(),
                        hint_factor: None,
                    },
                    row.center(),
                    if is_selected {
                        appearance.text_color
                    } else {
                        appearance.dim_text_color
                    },
                    *viewport,
                );
            }
        }
    }
}

impl<'a, Message, Theme, Renderer> From<TimePicker<'a, Message, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::text::Renderer,
{
    fn from(picker: TimePicker<'a, Message, Theme>) -> Self {
        Self::new(picker)
    }
}

/// The appearance of a [`TimePicker`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The picker's fill.
    pub background: Background,
    /// The band marking the chosen row.
    pub selection: Background,
    /// The colour of the values.
    pub text_color: Color,
    /// The colour of values either side of the selection.
    pub dim_text_color: Color,
    /// The border around the picker.
    pub border: Border,
}

/// The theme catalog of a [`TimePicker`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`TimePicker`].
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

/// The default styling of a [`TimePicker`] for aura's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Background::Color(palette.background.base.color),
        selection: Background::Color(palette.background.weak.color),
        text_color: palette.background.base.text,
        dim_text_color: Color {
            a: 0.5,
            ..palette.background.base.text
        },
        border: Border {
            radius: 12.0.into(),
            ..Border::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock<'a>(hour: u32, minute: u32) -> TimePicker<'a, ()> {
        TimePicker::new(Time::new(hour, minute))
    }

    const BOUNDS: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 240.0,
        height: VISIBLE_ROWS as f32 * ROW_HEIGHT,
    };

    #[test]
    fn a_time_wraps_rather_than_clamping() {
        // Clock arithmetic: 24:00 is midnight, not 23:59.
        assert_eq!(Time::new(24, 0), Time::new(0, 0));
        assert_eq!(Time::new(25, 70), Time::new(1, 10));
    }

    #[test]
    fn minutes_snap_down_onto_the_step() {
        assert_eq!(Time::new(9, 37).snapped(15), Time::new(9, 30));
        assert_eq!(Time::new(9, 45).snapped(15), Time::new(9, 45));
        assert_eq!(Time::new(9, 14).snapped(15), Time::new(9, 0));
    }

    #[test]
    fn a_zero_step_does_not_divide_by_zero() {
        assert_eq!(Time::new(9, 37).snapped(0), Time::new(9, 37));
    }

    #[test]
    fn the_step_bounds_how_many_minutes_are_offered() {
        assert_eq!(clock(9, 0).minute_step(15).minutes(), vec![0, 15, 30, 45]);
        assert_eq!(clock(9, 0).minute_step(30).minutes(), vec![0, 30]);
        assert_eq!(clock(9, 0).minute_step(1).minutes().len(), 60);
    }

    #[test]
    fn a_step_out_of_range_is_coerced_into_one() {
        assert_eq!(clock(9, 0).minute_step(0).minute_step, 1);
        assert_eq!(clock(9, 0).minute_step(999).minute_step, 60);
    }

    #[test]
    fn the_picker_only_ever_produces_minutes_on_its_grid() {
        let picker = clock(9, 37).minute_step(15);

        // The value it was handed is off-grid; what it reports is not.
        assert_eq!(picker.snapped().minute, 30);

        for index in 0..picker.minutes().len() {
            let produced = picker.time_for(Column::Minute, index);

            assert_eq!(
                produced.minute % 15,
                0,
                "produced an off-grid minute: {produced:?}"
            );
        }
    }

    #[test]
    fn every_hour_of_the_day_is_offered() {
        assert_eq!(clock(0, 0).hours().len(), 24);
        assert_eq!(clock(0, 0).hours().first(), Some(&0));
        assert_eq!(clock(0, 0).hours().last(), Some(&23));
    }

    #[test]
    fn choosing_an_hour_keeps_the_minute() {
        let picker = clock(9, 30).minute_step(15);

        assert_eq!(picker.time_for(Column::Hour, 14), Time::new(14, 30));
    }

    #[test]
    fn choosing_a_minute_keeps_the_hour() {
        let picker = clock(9, 0).minute_step(15);

        assert_eq!(picker.time_for(Column::Minute, 3), Time::new(9, 45));
    }

    #[test]
    fn the_two_columns_split_the_width_and_do_not_overlap() {
        let picker = clock(9, 0);

        let hour = picker.column_bounds(BOUNDS, Column::Hour);
        let minute = picker.column_bounds(BOUNDS, Column::Minute);

        assert_eq!(hour.width, minute.width);
        assert_eq!(hour.x + hour.width, minute.x);
        assert_eq!(minute.x + minute.width, BOUNDS.x + BOUNDS.width);
    }

    #[test]
    fn a_press_resolves_to_the_column_it_landed_in() {
        let picker = clock(9, 0);

        assert_eq!(
            picker.column_at(BOUNDS, Point::new(50.0, 100.0)),
            Some(Column::Hour)
        );
        assert_eq!(
            picker.column_at(BOUNDS, Point::new(200.0, 100.0)),
            Some(Column::Minute)
        );
    }

    #[test]
    fn the_middle_row_is_the_current_selection() {
        let picker = clock(9, 0);
        let centre_y = (VISIBLE_ROWS / 2) as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0;

        assert_eq!(
            picker.row_at(BOUNDS, Column::Hour, Point::new(50.0, centre_y)),
            Some(9)
        );
    }

    #[test]
    fn rows_above_and_below_step_the_value() {
        let picker = clock(9, 0);
        let centre = (VISIBLE_ROWS / 2) as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0;

        assert_eq!(
            picker.row_at(BOUNDS, Column::Hour, Point::new(50.0, centre - ROW_HEIGHT)),
            Some(8)
        );
        assert_eq!(
            picker.row_at(BOUNDS, Column::Hour, Point::new(50.0, centre + ROW_HEIGHT)),
            Some(10)
        );
    }

    #[test]
    fn rows_past_either_end_of_the_range_are_not_selectable() {
        // Midnight: there is nothing above hour zero.
        let picker = clock(0, 0);
        let centre = (VISIBLE_ROWS / 2) as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0;

        assert_eq!(
            picker.row_at(BOUNDS, Column::Hour, Point::new(50.0, centre - ROW_HEIGHT)),
            None
        );

        // And nothing below hour 23.
        let picker = clock(23, 0);

        assert_eq!(
            picker.row_at(BOUNDS, Column::Hour, Point::new(50.0, centre + ROW_HEIGHT)),
            None
        );
    }

    #[test]
    fn the_announcement_is_a_zero_padded_clock_reading() {
        assert_eq!(clock(9, 5).announcement(), "09:05");
        assert_eq!(clock(23, 59).announcement(), "23:59");
        assert_eq!(clock(0, 0).announcement(), "00:00");
    }
}
