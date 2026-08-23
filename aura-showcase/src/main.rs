//! Every Aura widget, in every state worth seeing, in one runnable
//! window.
//!
//! Run it with `cargo run -p aura-showcase`.
//!
//! This exists so the framework can be evaluated without reading its
//! source. Each section below shows one family of controls with its real
//! states — default, selected, disabled, loading, at a limit — because a
//! gallery that only shows the happy path hides exactly the cases that
//! decide whether a widget is finished.

use aura::widget::bottom_nav_bar::Item as NavItem;
use aura::widget::date::Date;
use aura::widget::segmented_control::Segment;
use aura::widget::tabs::Tab;
use aura::widget::time_picker::Time;
use aura::widget::{
    accordion, accordion::Section, avatar, badge, bottom_nav_bar, bottom_sheet, button, carousel,
    checkbox, chip, column, container, date_picker, drawer, progress_bar, pull_to_refresh, row,
    scrollable, search_bar, segmented_control, skeleton, slider, snackbar, stack, stepper, tabs,
    text, text_input, time_picker, toggler, tooltip,
};
use aura::{Center, Element, Fill, Task, Theme};

pub fn main() -> aura::Result {
    aura::application(Showcase::new, Showcase::update, Showcase::view)
        .title("Aura — Showcase")
        .theme(Showcase::theme)
        .run()
}

/// The showcase's sections, which double as the bottom bar's
/// destinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    Controls,
    Collections,
    Pickers,
    Surfaces,
}

impl Group {
    const ALL: [Self; 4] = [
        Self::Controls,
        Self::Collections,
        Self::Pickers,
        Self::Surfaces,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Controls => "Controls",
            Self::Collections => "Collections",
            Self::Pickers => "Pickers",
            Self::Surfaces => "Surfaces",
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Self::Controls => "\u{25C9}",
            Self::Collections => "\u{2261}",
            Self::Pickers => "\u{25F4}",
            Self::Surfaces => "\u{25F1}",
        }
    }
}

struct Showcase {
    dark: bool,
    group: Group,

    // Controls
    toggled: bool,
    checked: bool,
    slider_value: f32,
    quantity: i32,
    query: String,
    filters: [bool; 3],
    view_mode: usize,

    // Collections
    tab: usize,
    page: usize,
    section: Option<usize>,
    loading: bool,

    // Pickers
    visible_month: Date,
    chosen_date: Date,
    chosen_time: Time,

    // Surfaces
    drawer_open: bool,
    sheet_open: bool,
    sheet_detent: usize,
    notice: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    ThemeToggled(bool),
    GroupSelected(usize),

    Toggled(bool),
    Checked(bool),
    SliderChanged(f32),
    QuantityChanged(i32),
    QueryChanged(String),
    QueryCleared,
    FilterToggled(usize),
    ViewModeChanged(usize),

    TabSelected(usize),
    PageChanged(usize),
    SectionToggled(usize),
    LoadingToggled,

    MonthChanged(Date),
    DateChosen(Date),
    TimeChosen(Time),

    DrawerOpened,
    DrawerDismissed,
    SheetOpened,
    SheetDismissed,
    SheetResized(usize),
    NoticeShown,
    NoticeDismissed,
}

impl Showcase {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                dark: true,
                group: Group::Controls,
                toggled: true,
                checked: false,
                slider_value: 0.4,
                quantity: 3,
                query: String::new(),
                filters: [true, false, false],
                view_mode: 0,
                tab: 0,
                page: 0,
                section: Some(0),
                loading: false,
                visible_month: Date::new(2026, 8, 1),
                chosen_date: Date::new(2026, 8, 13),
                chosen_time: Time::new(9, 30),
                drawer_open: false,
                sheet_open: false,
                sheet_detent: 0,
                notice: None,
            },
            Task::none(),
        )
    }

    fn theme(&self) -> Theme {
        if self.dark { Theme::Dark } else { Theme::Light }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ThemeToggled(dark) => self.dark = dark,
            Message::GroupSelected(index) => {
                if let Some(group) = Group::ALL.get(index) {
                    self.group = *group;
                }
            }

            Message::Toggled(value) => self.toggled = value,
            Message::Checked(value) => self.checked = value,
            Message::SliderChanged(value) => self.slider_value = value,
            Message::QuantityChanged(value) => self.quantity = value,
            Message::QueryChanged(query) => self.query = query,
            Message::QueryCleared => self.query.clear(),
            Message::FilterToggled(index) => {
                if let Some(filter) = self.filters.get_mut(index) {
                    *filter = !*filter;
                }
            }
            Message::ViewModeChanged(index) => self.view_mode = index,

            Message::TabSelected(index) => self.tab = index,
            Message::PageChanged(page) => self.page = page,
            Message::SectionToggled(index) => {
                // Pressing the open section closes it, which is what
                // makes the control reversible.
                self.section = if self.section == Some(index) {
                    None
                } else {
                    Some(index)
                };
            }
            Message::LoadingToggled => self.loading = !self.loading,

            Message::MonthChanged(month) => self.visible_month = month,
            Message::DateChosen(date) => {
                self.chosen_date = date;
                self.notice = Some(format!("Chose {} {}", date.day, date.month));
            }
            Message::TimeChosen(time) => self.chosen_time = time,

            Message::DrawerOpened => self.drawer_open = true,
            Message::DrawerDismissed => self.drawer_open = false,
            Message::SheetOpened => self.sheet_open = true,
            Message::SheetDismissed => self.sheet_open = false,
            Message::SheetResized(detent) => self.sheet_detent = detent,
            Message::NoticeShown => self.notice = Some("Something happened".to_owned()),
            Message::NoticeDismissed => self.notice = None,
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let body = scrollable(
            container(match self.group {
                Group::Controls => self.controls(),
                Group::Collections => self.collections(),
                Group::Pickers => self.pickers(),
                Group::Surfaces => self.surfaces(),
            })
            .padding(24)
            .width(Fill),
        )
        .height(Fill);

        let nav = bottom_nav_bar(Group::ALL.into_iter().enumerate().map(|(index, group)| {
            NavItem::new(
                column![text(group.glyph()).size(18), text(group.label()).size(11)]
                    .spacing(2)
                    .align_x(Center),
                group.label(),
            )
            .on_press(Message::GroupSelected(index))
        }))
        .selected(
            Group::ALL
                .iter()
                .position(|group| *group == self.group)
                .unwrap_or(0),
        );

        let header = container(
            row![
                text("Aura Showcase").size(20),
                toggler(self.dark)
                    .label("Dark")
                    .on_toggle(Message::ThemeToggled),
            ]
            .spacing(16)
            .align_y(Center),
        )
        .padding(16);

        let page = column![header, body, nav];

        stack![
            page,
            drawer(self.navigation(), self.drawer_open)
                .on_open(Message::DrawerOpened)
                .on_dismiss(Message::DrawerDismissed),
            bottom_sheet(self.sheet_body(), self.sheet_open)
                .detents([0.35, 0.8])
                .detent(self.sheet_detent)
                .on_detent_change(Message::SheetResized)
                .on_dismiss(Message::SheetDismissed),
            snackbar(
                text(self.notice.as_deref().unwrap_or_default()).size(14),
                self.notice.is_some(),
            )
            .on_dismiss(Message::NoticeDismissed),
        ]
        .into()
    }

    // ── Sections ────────────────────────────────────────────────────

    fn controls(&self) -> Element<'_, Message> {
        const FILTERS: [&str; 3] = ["Unread", "Starred", "Muted"];
        const MODES: [&str; 3] = ["List", "Grid", "Compact"];

        column![
            heading("Buttons"),
            row![
                button(text("Default")).on_press(Message::NoticeShown),
                // No handler: the disabled state is the one people
                // forget to check.
                button(text("Disabled")),
                tooltip(
                    button(text("With tooltip")).on_press(Message::NoticeShown),
                    container(text("Explains itself").size(12)).padding(8),
                    tooltip::Position::Top,
                ),
            ]
            .spacing(12),
            heading("Selection"),
            row![
                toggler(self.toggled)
                    .label("Toggler")
                    .on_toggle(Message::Toggled),
                checkbox(self.checked)
                    .label("Checkbox")
                    .on_toggle(Message::Checked),
            ]
            .spacing(24)
            .align_y(Center),
            heading("Segmented control"),
            segmented_control(MODES.into_iter().enumerate().map(|(index, mode)| {
                Segment::new(text(mode).size(13), mode).on_press(Message::ViewModeChanged(index))
            }))
            .selected(self.view_mode),
            heading("Chips"),
            row(FILTERS.into_iter().enumerate().map(|(index, filter)| {
                chip(text(filter).size(13))
                    .selected(self.filters[index])
                    .on_press(Message::FilterToggled(index))
                    .into()
            }))
            .spacing(8),
            heading("Stepper"),
            row![
                stepper(self.quantity)
                    .range(1..=5)
                    .label("Quantity")
                    .on_change(Message::QuantityChanged),
                text(format!("{}", self.quantity)).size(16),
            ]
            .spacing(12)
            .align_y(Center),
            heading("Slider and progress"),
            slider(0.0..=1.0, self.slider_value, Message::SliderChanged).step(0.01),
            progress_bar(0.0..=1.0, self.slider_value),
            heading("Text entry"),
            text_input("A plain text input", &self.query).on_input(Message::QueryChanged),
            search_bar("Search", &self.query)
                .on_input(Message::QueryChanged)
                .on_clear(Message::QueryCleared),
        ]
        .spacing(16)
        .into()
    }

    fn collections(&self) -> Element<'_, Message> {
        const SECTIONS: [&str; 3] = ["Overview", "Activity", "Settings"];

        let listing: Element<'_, Message> = if self.loading {
            column![
                skeleton().width(240).height(16),
                skeleton().width(200).height(16),
                skeleton().width(220).height(16),
                // A still placeholder next to animated ones, so the
                // difference is visible.
                skeleton().width(180).height(16).still(),
            ]
            .spacing(10)
            .into()
        } else {
            column![
                row![
                    avatar("Ada Lovelace").size(36),
                    text("Ada Lovelace").size(15),
                    badge(text("Inbox").size(14)).count(3).label("unread"),
                ]
                .spacing(12)
                .align_y(Center),
                row![
                    avatar("Grace Hopper").size(36),
                    text("Grace Hopper").size(15),
                    badge(text("Alerts").size(14)).dot().label("new"),
                ]
                .spacing(12)
                .align_y(Center),
            ]
            .spacing(12)
            .into()
        };

        column![
            heading("Tabs"),
            tabs(SECTIONS.into_iter().enumerate().map(|(index, section)| {
                Tab::new(text(section).size(14), section).on_press(Message::TabSelected(index))
            }))
            .selected(self.tab),
            heading("Pull to refresh"),
            container(
                pull_to_refresh(
                    container(text("Drag me down").size(14))
                        .center_x(Fill)
                        .padding(24),
                    self.loading,
                )
                .on_refresh(Message::LoadingToggled),
            )
            .height(120),
            heading("Avatars, badges and skeletons"),
            button(text(if self.loading {
                "Show content"
            } else {
                "Show loading"
            }))
            .on_press(Message::LoadingToggled),
            listing,
            heading("Carousel"),
            container(
                carousel(
                    (1..=3)
                        .map(|page| {
                            container(text(format!("Page {page}")).size(24))
                                .center_x(Fill)
                                .center_y(Fill)
                                .into()
                        })
                        .collect::<Vec<_>>(),
                )
                .page(self.page)
                .on_page_change(Message::PageChanged),
            )
            .height(160),
            heading("Accordion"),
            accordion([
                Section::new("Account", text("Account settings live here.").size(13)),
                Section::new("Privacy", text("Privacy settings live here.").size(13)),
                Section::new("About", text("Version information.").size(13)),
            ])
            .expanded(self.section)
            .on_toggle(Message::SectionToggled),
        ]
        .spacing(16)
        .into()
    }

    fn pickers(&self) -> Element<'_, Message> {
        column![
            heading("Date picker"),
            text(format!(
                "Chosen: {} {} {}",
                self.chosen_date.day,
                aura::widget::date::month_name(self.chosen_date.month),
                self.chosen_date.year
            ))
            .size(13),
            date_picker(self.visible_month)
                .selected(self.chosen_date)
                // Passed in rather than read from the clock, so the view
                // is reproducible.
                .today(Date::new(2026, 8, 13))
                .on_select(Message::DateChosen)
                .on_month_change(Message::MonthChanged),
            heading("Time picker"),
            text(format!(
                "Chosen: {:02}:{:02}",
                self.chosen_time.hour, self.chosen_time.minute
            ))
            .size(13),
            time_picker(self.chosen_time)
                .minute_step(15)
                .on_change(Message::TimeChosen),
        ]
        .spacing(16)
        .into()
    }

    fn surfaces(&self) -> Element<'_, Message> {
        column![
            heading("Modal surfaces"),
            text("Each of these owns its own animation, scrim, and gestures.").size(13),
            button(text("Open drawer")).on_press(Message::DrawerOpened),
            text("Or swipe in from the left edge.").size(12),
            button(text("Open bottom sheet")).on_press(Message::SheetOpened),
            text("Drag its handle between detents, or fling it down.").size(12),
            button(text("Show snackbar")).on_press(Message::NoticeShown),
            text("It dismisses itself after four seconds.").size(12),
        ]
        .spacing(12)
        .into()
    }

    fn navigation(&self) -> Element<'_, Message> {
        column![
            text("Aura").size(24),
            text("Showcase drawer").size(13),
            button(text("Close")).on_press(Message::DrawerDismissed),
        ]
        .spacing(12)
        .padding(24)
        .into()
    }

    fn sheet_body(&self) -> Element<'_, Message> {
        column![
            text("Bottom sheet").size(20),
            text("Two detents. Drag the handle, or fling down to dismiss.").size(13),
        ]
        .spacing(12)
        .into()
    }
}

/// A section heading, so the gallery reads as a document rather than a
/// pile of controls.
fn heading(title: &str) -> Element<'_, Message> {
    text(title).size(16).into()
}
