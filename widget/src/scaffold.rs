//! The structural layout that glues an application's chrome together.
//!
//! # Example
//! ```no_run
//! # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
//! # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
//! use taim::widget::{drawer, scaffold, text};
//!
//! #[derive(Clone)]
//! enum Message {
//!     DrawerDismissed,
//! }
//!
//! # struct State { drawer_open: bool }
//! fn view(state: &State) -> Element<'_, Message> {
//!     scaffold(text("the page"))
//!         .app_bar(text("Title"))
//!         .drawer(
//!             drawer(text("navigation"), state.drawer_open)
//!                 .on_dismiss(Message::DrawerDismissed),
//!         )
//!         .into()
//! }
//! ```
//!
//! # What a scaffold does, and what it deliberately does not
//!
//! The scaffold stacks an application's structural slots in the right
//! order: the app bar, body and bottom bar in a column, then the
//! floating action button, drawer, and sheet layered over them.
//!
//! It does **not** manage their state. An earlier version of this widget
//! took `(drawer, is_open, on_close)` and synthesized its own scrim out
//! of a `mouse_area` -- which meant the scrim could not fade, the panel
//! could not slide, and a drag could not reach the drawer at all. Both
//! [`Drawer`](crate::drawer::Drawer) and
//! [`BottomSheet`](crate::bottom_sheet::BottomSheet) now own their open
//! state, animation, scrim, and gestures, so the scaffold only decides
//! stacking order.

use crate::bottom_nav_bar::BottomNavigationBar;
use crate::bottom_sheet::BottomSheet;
use crate::core::{Element, Length, Padding};
use crate::drawer::Drawer;
use crate::{column, container, stack};

/// How much room is left around a floating action button.
const FAB_MARGIN: f32 = 16.0;

/// The structural layout that glues an application's chrome together.
#[allow(missing_debug_implementations)]
pub struct Scaffold<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: crate::container::Catalog
        + crate::drawer::Catalog
        + crate::bottom_sheet::Catalog
        + crate::bottom_nav_bar::Catalog,
{
    body: Element<'a, Message, Theme, Renderer>,
    app_bar: Option<Element<'a, Message, Theme, Renderer>>,
    bottom_nav: Option<BottomNavigationBar<'a, Message, Theme, Renderer>>,
    drawer: Option<Drawer<'a, Message, Theme, Renderer>>,
    end_drawer: Option<Drawer<'a, Message, Theme, Renderer>>,
    bottom_sheet: Option<BottomSheet<'a, Message, Theme, Renderer>>,
    fab: Option<Element<'a, Message, Theme, Renderer>>,
}

impl<'a, Message, Theme, Renderer> Scaffold<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: crate::container::Catalog
        + crate::drawer::Catalog
        + crate::bottom_sheet::Catalog
        + crate::bottom_nav_bar::Catalog
        + 'a,
    Renderer: crate::core::Renderer + 'a,
{
    /// Creates a scaffold around the application's body.
    pub fn new(body: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            body: body.into(),
            app_bar: None,
            bottom_nav: None,
            drawer: None,
            end_drawer: None,
            bottom_sheet: None,
            fab: None,
        }
    }

    /// Sets the bar across the top of the application.
    pub fn app_bar(mut self, app_bar: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        self.app_bar = Some(app_bar.into());
        self
    }

    /// Sets the navigation bar across the bottom of the application.
    pub fn bottom_navigation_bar(
        mut self,
        nav: BottomNavigationBar<'a, Message, Theme, Renderer>,
    ) -> Self {
        self.bottom_nav = Some(nav);
        self
    }

    /// Sets the drawer that slides in from the leading edge.
    ///
    /// The drawer carries its own open state; see
    /// [`Drawer`](crate::drawer::Drawer).
    pub fn drawer(mut self, drawer: Drawer<'a, Message, Theme, Renderer>) -> Self {
        self.drawer = Some(drawer.side(crate::drawer::Side::Start));
        self
    }

    /// Sets the drawer that slides in from the trailing edge.
    pub fn end_drawer(mut self, drawer: Drawer<'a, Message, Theme, Renderer>) -> Self {
        self.end_drawer = Some(drawer.side(crate::drawer::Side::End));
        self
    }

    /// Sets the sheet that slides up from the bottom.
    pub fn bottom_sheet(mut self, sheet: BottomSheet<'a, Message, Theme, Renderer>) -> Self {
        self.bottom_sheet = Some(sheet);
        self
    }

    /// Sets the floating action button, anchored to the bottom trailing
    /// corner of the body.
    pub fn floating_action_button(
        mut self,
        fab: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.fab = Some(fab.into());
        self
    }
}

impl<'a, Message, Theme, Renderer> From<Scaffold<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: crate::container::Catalog
        + crate::drawer::Catalog
        + crate::bottom_sheet::Catalog
        + crate::bottom_nav_bar::Catalog
        + 'a,
    Renderer: crate::core::Renderer + 'a,
{
    fn from(scaffold: Scaffold<'a, Message, Theme, Renderer>) -> Self {
        // The structural column: chrome above and below, body taking
        // whatever is left.
        let mut page = column::Column::new()
            .width(Length::Fill)
            .height(Length::Fill);

        if let Some(app_bar) = scaffold.app_bar {
            page = page.push(app_bar);
        }

        page = page.push(
            container(scaffold.body)
                .width(Length::Fill)
                .height(Length::FillPortion(1)),
        );

        if let Some(bottom_nav) = scaffold.bottom_nav {
            page = page.push(bottom_nav);
        }

        let mut layers = stack::Stack::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .push(page);

        // The FAB floats over the body but under any modal surface: a
        // drawer that did not cover it would look like a rendering bug.
        if let Some(fab) = scaffold.fab {
            layers = layers.push(
                container(fab)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(Padding::new(FAB_MARGIN))
                    .align_right(Length::Fill)
                    .align_bottom(Length::Fill),
            );
        }

        // Modal surfaces last, in the order they should overlap. Each
        // fills the stack and draws its own scrim, so a closed one costs
        // a layout pass and nothing else.
        if let Some(sheet) = scaffold.bottom_sheet {
            layers = layers.push(sheet);
        }

        if let Some(drawer) = scaffold.drawer {
            layers = layers.push(drawer);
        }

        if let Some(end_drawer) = scaffold.end_drawer {
            layers = layers.push(end_drawer);
        }

        layers.into()
    }
}
