//! TAIM design-system component helpers.

use super::*;

/// Creates an [`IconButton`] showing `glyph`, named `label`.
///
/// The label is **required**: an icon-only control carries no visible
/// text, so this is the only name assistive technology can announce.
///
/// [`IconButton`]: crate::icon_button::IconButton
pub fn icon_button<'a, Message, Theme, Renderer>(
    glyph: char,
    label: impl Into<String>,
) -> crate::icon_button::IconButton<'a, Message, Theme, Renderer>
where
    Renderer: core::text::Renderer,
    Theme: crate::icon_button::Catalog,
{
    crate::icon_button::IconButton::new(glyph, label)
}

/// Creates a one-line [`ListTile`] with the given body.
///
/// [`ListTile`]: crate::list_tile::ListTile
pub fn list_tile<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> crate::list_tile::ListTile<'a, Message, Theme, Renderer>
where
    Renderer: core::Renderer,
    Theme: crate::list_tile::Catalog,
{
    crate::list_tile::ListTile::new(content)
}

/// Creates a standard [`AppBar`] with the given title.
///
/// [`AppBar`]: crate::app_bar::AppBar
pub fn app_bar<'a, Message, Theme, Renderer>(
    title: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> crate::app_bar::AppBar<'a, Message, Theme, Renderer>
where
    Renderer: core::Renderer,
    Theme: crate::app_bar::Catalog,
{
    crate::app_bar::AppBar::new(title)
}

/// Creates a modal [`Dialog`] around `content`.
///
/// Layer it over your page with [`stack`]; see the module documentation
/// for why a dialog is a stacked widget rather than an overlay.
///
/// [`Dialog`]: crate::dialog::Dialog
pub fn dialog<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> crate::dialog::Dialog<'a, Message, Theme, Renderer>
where
    Renderer: core::Renderer,
    Theme: crate::dialog::Catalog,
{
    crate::dialog::Dialog::new(content)
}

/// Creates a new [`Scaffold`] widget.
///
/// [`Scaffold`]: crate::scaffold::Scaffold
pub fn scaffold<'a, Message, Theme, Renderer>(
    body: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> crate::scaffold::Scaffold<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: crate::container::Catalog
        + crate::drawer::Catalog
        + crate::bottom_sheet::Catalog
        + crate::bottom_nav_bar::Catalog
        + 'a,
    Renderer: crate::core::Renderer + 'a,
{
    crate::scaffold::Scaffold::new(body)
}

/// Creates a new [`Drawer`] panel, open or closed as given.
///
/// [`Drawer`]: crate::drawer::Drawer
pub fn drawer<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    is_open: bool,
) -> crate::drawer::Drawer<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: crate::drawer::Catalog + 'a,
    Renderer: crate::core::Renderer + 'a,
{
    crate::drawer::Drawer::new(content, is_open)
}

/// Creates a new [`BottomSheet`], presented or dismissed as given.
///
/// [`BottomSheet`]: crate::bottom_sheet::BottomSheet
pub fn bottom_sheet<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    is_open: bool,
) -> crate::bottom_sheet::BottomSheet<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: crate::bottom_sheet::Catalog + 'a,
    Renderer: crate::core::Renderer + 'a,
{
    crate::bottom_sheet::BottomSheet::new(content, is_open)
}

/// Creates a new [`BottomNavigationBar`].
///
/// [`BottomNavigationBar`]: crate::bottom_nav_bar::BottomNavigationBar
pub fn bottom_nav_bar<'a, Message, Theme, Renderer>(
    items: impl IntoIterator<Item = crate::bottom_nav_bar::Item<'a, Message, Theme, Renderer>>,
) -> crate::bottom_nav_bar::BottomNavigationBar<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: crate::bottom_nav_bar::Catalog + 'a,
    Renderer: crate::core::Renderer + 'a,
{
    crate::bottom_nav_bar::BottomNavigationBar::new(items)
}

/// Creates a new [`Tabs`] bar.
///
/// [`Tabs`]: crate::tabs::Tabs
pub fn tabs<'a, Message, Theme, Renderer>(
    tabs: impl IntoIterator<Item = crate::tabs::Tab<'a, Message, Theme, Renderer>>,
) -> crate::tabs::Tabs<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: crate::tabs::Catalog + 'a,
    Renderer: crate::core::Renderer + 'a,
{
    crate::tabs::Tabs::new(tabs)
}

/// Creates a new [`Snackbar`], visible or not as given.
///
/// [`Snackbar`]: crate::snackbar::Snackbar
pub fn snackbar<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    is_visible: bool,
) -> crate::snackbar::Snackbar<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: crate::snackbar::Catalog + 'a,
    Renderer: crate::core::Renderer + 'a,
{
    crate::snackbar::Snackbar::new(content, is_visible)
}

/// Creates a new [`Chip`].
///
/// [`Chip`]: crate::chip::Chip
pub fn chip<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> crate::chip::Chip<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: crate::chip::Catalog + 'a,
    Renderer: crate::core::Renderer + 'a,
{
    crate::chip::Chip::new(content)
}

/// Creates a new [`Badge`] marking `content`.
///
/// [`Badge`]: crate::badge::Badge
pub fn badge<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> crate::badge::Badge<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: crate::badge::Catalog + 'a,
    Renderer: crate::core::text::Renderer + 'a,
{
    crate::badge::Badge::new(content)
}

/// Creates a new [`Avatar`] for `name`.
///
/// [`Avatar`]: crate::avatar::Avatar
pub fn avatar<'a, Theme>(name: impl Into<String>) -> crate::avatar::Avatar<'a, Theme>
where
    Theme: crate::avatar::Catalog,
{
    crate::avatar::Avatar::new(name)
}

/// Creates a new [`Skeleton`] placeholder.
///
/// [`Skeleton`]: crate::skeleton::Skeleton
pub fn skeleton<'a, Theme>() -> crate::skeleton::Skeleton<'a, Theme>
where
    Theme: crate::skeleton::Catalog,
{
    crate::skeleton::Skeleton::new()
}

/// Creates a new [`SearchBar`] showing `value`, prompting with `placeholder`.
///
/// [`SearchBar`]: crate::search_bar::SearchBar
pub fn search_bar<'a, Message, Theme, Renderer>(
    placeholder: &str,
    value: &str,
) -> crate::search_bar::SearchBar<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: crate::search_bar::Catalog + crate::text_input::Catalog + 'a,
    <Theme as crate::text_input::Catalog>::Class<'a>: From<crate::text_input::StyleFn<'a, Theme>>,
    Renderer: crate::core::text::Renderer + 'a,
{
    crate::search_bar::SearchBar::new(placeholder, value)
}

/// Creates a new [`PullToRefresh`] around `content`.
///
/// [`PullToRefresh`]: crate::pull_to_refresh::PullToRefresh
pub fn pull_to_refresh<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    is_refreshing: bool,
) -> crate::pull_to_refresh::PullToRefresh<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: crate::pull_to_refresh::Catalog + 'a,
    Renderer: crate::core::Renderer + 'a,
{
    crate::pull_to_refresh::PullToRefresh::new(content, is_refreshing)
}

/// Creates a new [`SegmentedControl`].
///
/// [`SegmentedControl`]: crate::segmented_control::SegmentedControl
pub fn segmented_control<'a, Message, Theme, Renderer>(
    segments: impl IntoIterator<Item = crate::segmented_control::Segment<'a, Message, Theme, Renderer>>,
) -> crate::segmented_control::SegmentedControl<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: crate::segmented_control::Catalog + 'a,
    Renderer: crate::core::Renderer + 'a,
{
    crate::segmented_control::SegmentedControl::new(segments)
}

/// Creates a new [`Stepper`] showing `value`.
///
/// [`Stepper`]: crate::stepper::Stepper
pub fn stepper<'a, Message, Theme>(value: i32) -> crate::stepper::Stepper<'a, Message, Theme>
where
    Theme: crate::stepper::Catalog,
{
    crate::stepper::Stepper::new(value)
}

/// Creates a new [`Accordion`].
///
/// [`Accordion`]: crate::accordion::Accordion
pub fn accordion<'a, Message, Theme, Renderer>(
    sections: impl IntoIterator<Item = crate::accordion::Section<'a, Message, Theme, Renderer>>,
) -> crate::accordion::Accordion<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: crate::accordion::Catalog + 'a,
    Renderer: crate::core::text::Renderer + 'a,
{
    crate::accordion::Accordion::new(sections)
}

/// Creates a new [`Carousel`] over `pages`.
///
/// [`Carousel`]: crate::carousel::Carousel
pub fn carousel<'a, Message, Theme, Renderer>(
    pages: impl IntoIterator<Item = Element<'a, Message, Theme, Renderer>>,
) -> crate::carousel::Carousel<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: crate::carousel::Catalog + 'a,
    Renderer: crate::core::Renderer + 'a,
{
    crate::carousel::Carousel::new(pages)
}

/// Creates a new [`DatePicker`] showing the month `visible` falls in.
///
/// [`DatePicker`]: crate::date_picker::DatePicker
pub fn date_picker<'a, Message, Theme>(
    visible: crate::date::Date,
) -> crate::date_picker::DatePicker<'a, Message, Theme>
where
    Theme: crate::date_picker::Catalog,
{
    crate::date_picker::DatePicker::new(visible)
}

/// Creates a new [`TimePicker`] showing `value`.
///
/// [`TimePicker`]: crate::time_picker::TimePicker
pub fn time_picker<'a, Message, Theme>(
    value: crate::time_picker::Time,
) -> crate::time_picker::TimePicker<'a, Message, Theme>
where
    Theme: crate::time_picker::Catalog,
{
    crate::time_picker::TimePicker::new(value)
}
