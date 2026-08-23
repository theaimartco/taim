use aura::widget::bottom_nav_bar::Item;
// Aliased: this example already has its own `Tab` for the bottom bar's
// destinations, which is a different thing from a tab in a tab bar.
use aura::widget::tabs::Tab as TabItem;
use aura::widget::{
    avatar, badge, bottom_nav_bar, bottom_sheet, button, chip, column, container, drawer,
    pull_to_refresh, row, scaffold, search_bar, skeleton, snackbar, stack, tabs, text,
};
use aura::{Center, Element, Fill, Task};

pub fn main() -> aura::Result {
    aura::application(Shell::new, Shell::update, Shell::view)
        .title("Aura — Mobile Shell")
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Home,
    Search,
    Profile,
}

impl Tab {
    const ALL: [Self; 3] = [Self::Home, Self::Search, Self::Profile];

    fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Search => "Search",
            Self::Profile => "Profile",
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Self::Home => "\u{2302}",
            Self::Search => "\u{2315}",
            Self::Profile => "\u{263A}",
        }
    }
}

struct Shell {
    tab: Tab,
    section: usize,
    query: String,
    filters: [bool; 3],
    refreshing: bool,
    drawer_open: bool,
    sheet_open: bool,
    sheet_detent: usize,
    notice: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    TabSelected(usize),
    SectionSelected(usize),
    QueryChanged(String),
    QueryCleared,
    FilterToggled(usize),
    Refreshed,
    DrawerOpened,
    DrawerDismissed,
    SheetOpened,
    SheetDismissed,
    SheetResized(usize),
    NoticeDismissed,
    Undo,
}

impl Shell {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                tab: Tab::Home,
                section: 0,
                query: String::new(),
                filters: [true, false, false],
                refreshing: false,
                drawer_open: false,
                sheet_open: false,
                sheet_detent: 0,
                notice: None,
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(index) => {
                if let Some(tab) = Tab::ALL.get(index) {
                    self.tab = *tab;
                    self.section = 0;
                }
            }
            Message::SectionSelected(index) => self.section = index,
            Message::QueryChanged(query) => self.query = query,
            Message::QueryCleared => self.query.clear(),
            Message::FilterToggled(index) => {
                if let Some(filter) = self.filters.get_mut(index) {
                    *filter = !*filter;
                }
            }
            // A real application would start work here and clear the flag
            // when it finished; this one just acknowledges the gesture.
            Message::Refreshed => {
                self.refreshing = false;
                self.notice = Some("Refreshed".to_owned());
            }
            Message::DrawerOpened => self.drawer_open = true,
            Message::DrawerDismissed => self.drawer_open = false,
            Message::SheetOpened => self.sheet_open = true,
            Message::SheetDismissed => {
                self.sheet_open = false;
                self.notice = Some("Sheet dismissed".to_owned());
            }
            Message::SheetResized(detent) => self.sheet_detent = detent,
            Message::NoticeDismissed => self.notice = None,
            Message::Undo => {
                self.notice = None;
                self.sheet_open = true;
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        const SECTIONS: [&str; 3] = ["Overview", "Activity", "Settings"];

        let sections = tabs(SECTIONS.into_iter().enumerate().map(|(index, section)| {
            TabItem::new(text(section).size(14), section).on_press(Message::SectionSelected(index))
        }))
        .selected(self.section);

        const FILTERS: [&str; 3] = ["Unread", "Starred", "Muted"];

        let filters = row(FILTERS.into_iter().enumerate().map(|(index, filter)| {
            chip(text(filter).size(13))
                .selected(self.filters[index])
                .on_press(Message::FilterToggled(index))
                .into()
        }))
        .spacing(8);

        // A skeleton row stands in for the list while a refresh runs, so
        // the pull has something to reveal.
        let listing: Element<'_, Message> = if self.refreshing {
            column![
                skeleton().width(240).height(16),
                skeleton().width(200).height(16),
                skeleton().width(220).height(16),
            ]
            .spacing(10)
            .into()
        } else {
            row![
                avatar("Ada Lovelace").size(36),
                badge(text("Inbox").size(16)).count(3).label("unread"),
            ]
            .spacing(12)
            .align_y(Center)
            .into()
        };

        let page = container(
            column![
                search_bar("Search", &self.query)
                    .on_input(Message::QueryChanged)
                    .on_clear(Message::QueryCleared),
                filters,
                text(self.tab.label()).size(32),
                text(SECTIONS[self.section.min(SECTIONS.len() - 1)]).size(16),
                listing,
                text("Swipe in from the left edge to open the drawer.").size(14),
                button(text("Show bottom sheet")).on_press(Message::SheetOpened),
            ]
            .spacing(16)
            .align_x(Center),
        )
        .center_x(Fill)
        .center_y(Fill);

        let body = column![
            sections,
            pull_to_refresh(page, self.refreshing).on_refresh(Message::Refreshed),
        ];

        let navigation = column![
            text("Aura").size(24),
            text("Mobile shell example").size(13),
            button(text("Close")).on_press(Message::DrawerDismissed),
        ]
        .spacing(12)
        .padding(24);

        let sheet_body = column![
            text("Bottom sheet").size(20),
            text("Drag the handle to switch detents, or fling it down to dismiss.").size(13),
        ]
        .spacing(12);

        let tabs = bottom_nav_bar(Tab::ALL.into_iter().enumerate().map(|(index, tab)| {
            Item::new(
                column![text(tab.glyph()).size(20), text(tab.label()).size(11)]
                    .spacing(2)
                    .align_x(Center),
                tab.label(),
            )
            .on_press(Message::TabSelected(index))
        }))
        .selected(
            Tab::ALL
                .iter()
                .position(|tab| *tab == self.tab)
                .unwrap_or(0),
        );

        // The snackbar is layered over the whole shell rather than
        // placed inside the body, so it floats above the bottom bar the
        // way a real notice does.
        let shell = scaffold(body)
            .app_bar(container(text("Aura").size(20)).padding(16))
            .bottom_navigation_bar(tabs)
            .drawer(
                drawer(navigation, self.drawer_open)
                    .on_open(Message::DrawerOpened)
                    .on_dismiss(Message::DrawerDismissed),
            )
            .bottom_sheet(
                bottom_sheet(sheet_body, self.sheet_open)
                    .detents([0.35, 0.85])
                    .detent(self.sheet_detent)
                    .on_detent_change(Message::SheetResized)
                    .on_dismiss(Message::SheetDismissed),
            );

        stack![
            shell,
            snackbar(
                text(self.notice.as_deref().unwrap_or_default()).size(14),
                self.notice.is_some(),
            )
            .action(button(text("Undo")).on_press(Message::Undo))
            .on_dismiss(Message::NoticeDismissed),
        ]
        .into()
    }
}
