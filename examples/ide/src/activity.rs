//! Activity Bar & Sidebar View Management for Taim Studio.
//! Provides the primary left-rail navigation matching VS Code.

/// Active view displayed in the primary sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarView {
    #[default]
    Explorer,
    Search,
    SourceControl,
    Debug,
    Extensions,
    Settings,
}

impl SidebarView {
    /// Returns the icon glyph for the activity bar button.
    pub fn icon(&self) -> &'static str {
        match self {
            SidebarView::Explorer => "◧",
            SidebarView::Search => "⌕",
            SidebarView::SourceControl => "⑂",
            SidebarView::Debug => "▷",
            SidebarView::Extensions => "❖",
            SidebarView::Settings => "⚙",
        }
    }

    /// Human-readable label for the view.
    pub fn label(&self) -> &'static str {
        match self {
            SidebarView::Explorer => "Explorer",
            SidebarView::Search => "Search",
            SidebarView::SourceControl => "Source Control",
            SidebarView::Debug => "Run and Debug",
            SidebarView::Extensions => "Extensions",
            SidebarView::Settings => "Settings",
        }
    }

    /// Shortcut description for tooltips.
    pub fn shortcut(&self) -> &'static str {
        match self {
            SidebarView::Explorer => "Cmd+Shift+E",
            SidebarView::Search => "Cmd+Shift+F",
            SidebarView::SourceControl => "Cmd+Shift+G",
            SidebarView::Debug => "Cmd+Shift+D",
            SidebarView::Extensions => "Cmd+Shift+X",
            SidebarView::Settings => "Cmd+,",
        }
    }
}

/// Activity bar state managing current selection and notification badges.
#[derive(Debug, Clone, Default)]
pub struct ActivityBarState {
    pub active_view: SidebarView,
    pub git_changes_badge: usize,
    pub diagnostics_badge: usize,
    pub extensions_badge: usize,
}

impl ActivityBarState {
    pub fn new() -> Self {
        Self {
            active_view: SidebarView::Explorer,
            git_changes_badge: 2,
            diagnostics_badge: 1,
            extensions_badge: 1,
        }
    }

    /// Selects a sidebar view. If already selected, returns true to signal a toggle.
    pub fn select(&mut self, view: SidebarView) -> bool {
        if self.active_view == view {
            true
        } else {
            self.active_view = view;
            false
        }
    }

    /// Returns badge count for a specific view, if any.
    pub fn badge_for(&self, view: SidebarView) -> Option<usize> {
        match view {
            SidebarView::SourceControl if self.git_changes_badge > 0 => Some(self.git_changes_badge),
            SidebarView::Debug if self.diagnostics_badge > 0 => Some(self.diagnostics_badge),
            SidebarView::Extensions if self.extensions_badge > 0 => Some(self.extensions_badge),
            _ => None,
        }
    }
}
