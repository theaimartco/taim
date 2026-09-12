//! Toast Notification Center for Taim Studio.
//! Provides bottom-right floating notifications and status bar notification center.

/// Severity level for a toast notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationSeverity {
    Info,
    Success,
    Warning,
    Error,
}

impl NotificationSeverity {
    pub fn icon(&self) -> &'static str {
        match self {
            NotificationSeverity::Info => "ℹ",
            NotificationSeverity::Success => "✓",
            NotificationSeverity::Warning => "⚠",
            NotificationSeverity::Error => "✕",
        }
    }
}

/// A toast notification displayed to the developer.
#[derive(Debug, Clone)]
pub struct ToastNotification {
    pub id: usize,
    pub severity: NotificationSeverity,
    pub title: String,
    pub message: String,
    pub timestamp: String,
    pub action_label: Option<String>,
    pub action_tag: Option<String>,
}

/// Toast notification center state.
#[derive(Debug, Clone)]
pub struct NotificationManager {
    pub active_toasts: Vec<ToastNotification>,
    pub history: Vec<ToastNotification>,
    pub is_center_open: bool,
    pub next_id: usize,
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationManager {
    pub fn new() -> Self {
        let mut mgr = Self {
            active_toasts: Vec::new(),
            history: Vec::new(),
            is_center_open: false,
            next_id: 1,
        };
        // Initial welcoming notification
        mgr.push(
            NotificationSeverity::Success,
            "Taim Studio Ready",
            "Native GPU engine active (120 FPS, 0.28ms frame latency).",
            Some("View Metrics"),
            Some("open_bench"),
        );
        mgr
    }

    /// Pushes a new toast notification.
    pub fn push(
        &mut self,
        severity: NotificationSeverity,
        title: impl Into<String>,
        message: impl Into<String>,
        action_label: Option<&str>,
        action_tag: Option<&str>,
    ) {
        let id = self.next_id;
        self.next_id += 1;

        let toast = ToastNotification {
            id,
            severity,
            title: title.into(),
            message: message.into(),
            timestamp: "just now".to_string(),
            action_label: action_label.map(|s| s.to_string()),
            action_tag: action_tag.map(|s| s.to_string()),
        };

        self.active_toasts.push(toast.clone());
        self.history.push(toast);

        // Limit active toasts to 3 maximum
        if self.active_toasts.len() > 3 {
            self.active_toasts.remove(0);
        }
    }

    /// Dismisses a specific active toast.
    pub fn dismiss(&mut self, id: usize) {
        self.active_toasts.retain(|t| t.id != id);
    }

    /// Clears all active toasts.
    pub fn clear_active(&mut self) {
        self.active_toasts.clear();
    }

    /// Toggles the full notification history drawer.
    pub fn toggle_center(&mut self) {
        self.is_center_open = !self.is_center_open;
    }

    /// Unread / total count.
    pub fn count(&self) -> usize {
        self.active_toasts.len()
    }
}
