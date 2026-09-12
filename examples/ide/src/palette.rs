//! Quick Open & Command Palette for Taim Studio (Cmd+P / Ctrl+P).

/// The type of item in the command palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteKind {
    /// Jump to / open file.
    File,
    /// Execute an IDE command.
    Action,
}

/// An entry in the command palette.
#[derive(Debug, Clone)]
pub struct PaletteItem {
    pub id: usize,
    pub kind: PaletteKind,
    pub title: String,
    pub subtitle: String,
    pub shortcut: Option<String>,
    pub icon: &'static str,
    /// Associated action payload or file path.
    pub payload: String,
}

/// State for the quick open / command palette modal.
#[derive(Debug, Clone)]
pub struct PaletteState {
    pub is_open: bool,
    pub query: String,
    pub selected_index: usize,
    pub all_items: Vec<PaletteItem>,
}

impl Default for PaletteState {
    fn default() -> Self {
        Self::new()
    }
}

impl PaletteState {
    /// Creates a new palette state populated with available files and commands.
    pub fn new() -> Self {
        let items = vec![
            // Files
            PaletteItem {
                id: 1,
                kind: PaletteKind::File,
                title: "main.rs".to_string(),
                subtitle: "src/main.rs — Application Entry Point".to_string(),
                shortcut: None,
                icon: "◈",
                payload: "src/main.rs".to_string(),
            },
            PaletteItem {
                id: 2,
                kind: PaletteKind::File,
                title: "raster.rs".to_string(),
                subtitle: "src/raster.rs — Tiled Compute Rasterizer".to_string(),
                shortcut: None,
                icon: "◈",
                payload: "src/raster.rs".to_string(),
            },
            PaletteItem {
                id: 3,
                kind: PaletteKind::File,
                title: "scene.rs".to_string(),
                subtitle: "src/scene.rs — Retained Scene Tree Arena".to_string(),
                shortcut: None,
                icon: "◈",
                payload: "src/scene.rs".to_string(),
            },
            PaletteItem {
                id: 4,
                kind: PaletteKind::File,
                title: "compute_raster.wgsl".to_string(),
                subtitle: "shaders/compute_raster.wgsl — GPU Compute Pipeline".to_string(),
                shortcut: None,
                icon: "⚡",
                payload: "shaders/compute_raster.wgsl".to_string(),
            },
            PaletteItem {
                id: 5,
                kind: PaletteKind::File,
                title: "Cargo.toml".to_string(),
                subtitle: "Package Manifest & Dependencies".to_string(),
                shortcut: None,
                icon: "▤",
                payload: "Cargo.toml".to_string(),
            },
            PaletteItem {
                id: 6,
                kind: PaletteKind::File,
                title: "README.md".to_string(),
                subtitle: "Project Documentation & Guide".to_string(),
                shortcut: None,
                icon: "≡",
                payload: "README.md".to_string(),
            },
            PaletteItem {
                id: 7,
                kind: PaletteKind::File,
                title: "config.json".to_string(),
                subtitle: "Engine Runtime Settings".to_string(),
                shortcut: None,
                icon: "{ }",
                payload: "config.json".to_string(),
            },
            // Actions
            PaletteItem {
                id: 101,
                kind: PaletteKind::Action,
                title: "> Run: Launch Application (GPU)".to_string(),
                subtitle: "Compile and present hardware window".to_string(),
                shortcut: Some("F5".to_string()),
                icon: "▷",
                payload: "action:run".to_string(),
            },
            PaletteItem {
                id: 102,
                kind: PaletteKind::Action,
                title: "> Build: Cargo Build Release".to_string(),
                subtitle: "Trigger release compilation profile".to_string(),
                shortcut: Some("Cmd+Shift+B".to_string()),
                icon: "⚡",
                payload: "action:build".to_string(),
            },
            PaletteItem {
                id: 103,
                kind: PaletteKind::Action,
                title: "> View: Toggle Dual Split-Editor".to_string(),
                subtitle: "Side-by-side editing layout".to_string(),
                shortcut: Some("Cmd+\\".to_string()),
                icon: "◫",
                payload: "action:split".to_string(),
            },
            PaletteItem {
                id: 104,
                kind: PaletteKind::Action,
                title: "> View: Toggle Project Explorer".to_string(),
                subtitle: "Show/hide left sidebar".to_string(),
                shortcut: Some("Cmd+B".to_string()),
                icon: "◧",
                payload: "action:toggle_sidebar".to_string(),
            },
            PaletteItem {
                id: 105,
                kind: PaletteKind::Action,
                title: "> View: Toggle Bottom Console".to_string(),
                subtitle: "Show/hide terminal and build output".to_string(),
                shortcut: Some("Cmd+J".to_string()),
                icon: "⬒",
                payload: "action:toggle_terminal".to_string(),
            },
            PaletteItem {
                id: 106,
                kind: PaletteKind::Action,
                title: "> Theme: Dark Mode".to_string(),
                subtitle: "Deep dark linear-light palette".to_string(),
                shortcut: None,
                icon: "☾",
                payload: "action:theme_dark".to_string(),
            },
            PaletteItem {
                id: 107,
                kind: PaletteKind::Action,
                title: "> Theme: High Contrast Mode".to_string(),
                subtitle: "Maximum accessibility contrast floor".to_string(),
                shortcut: None,
                icon: "☼",
                payload: "action:theme_high_contrast".to_string(),
            },
            PaletteItem {
                id: 108,
                kind: PaletteKind::Action,
                title: "> Editor: New Scratchpad".to_string(),
                subtitle: "Create temporary Rust buffer".to_string(),
                shortcut: Some("Cmd+N".to_string()),
                icon: "＋",
                payload: "action:new_scratch".to_string(),
            },
            PaletteItem {
                id: 109,
                kind: PaletteKind::Action,
                title: "> View: Show Explorer".to_string(),
                subtitle: "Show project file tree".to_string(),
                shortcut: Some("Cmd+Shift+E".to_string()),
                icon: "◧",
                payload: "action:view_explorer".to_string(),
            },
            PaletteItem {
                id: 110,
                kind: PaletteKind::Action,
                title: "> View: Show Global Search".to_string(),
                subtitle: "Search and replace across all files".to_string(),
                shortcut: Some("Cmd+Shift+F".to_string()),
                icon: "⌕",
                payload: "action:view_search".to_string(),
            },
            PaletteItem {
                id: 111,
                kind: PaletteKind::Action,
                title: "> View: Show Source Control (Git)".to_string(),
                subtitle: "View git staging, branches, and diffs".to_string(),
                shortcut: Some("Cmd+Shift+G".to_string()),
                icon: "⑂",
                payload: "action:view_git".to_string(),
            },
            PaletteItem {
                id: 112,
                kind: PaletteKind::Action,
                title: "> View: Show Run and Debug".to_string(),
                subtitle: "Breakpoints, call stack, variables".to_string(),
                shortcut: Some("Cmd+Shift+D".to_string()),
                icon: "▷",
                payload: "action:view_debug".to_string(),
            },
            PaletteItem {
                id: 113,
                kind: PaletteKind::Action,
                title: "> View: Show Extensions Marketplace".to_string(),
                subtitle: "Install and manage language plugins".to_string(),
                shortcut: Some("Cmd+Shift+X".to_string()),
                icon: "❖",
                payload: "action:view_extensions".to_string(),
            },
            PaletteItem {
                id: 114,
                kind: PaletteKind::Action,
                title: "> View: Open Settings".to_string(),
                subtitle: "Configure editor, theme, and keybindings".to_string(),
                shortcut: Some("Cmd+,".to_string()),
                icon: "⚙",
                payload: "action:view_settings".to_string(),
            },
            PaletteItem {
                id: 115,
                kind: PaletteKind::Action,
                title: "> Git: Stage All Changes".to_string(),
                subtitle: "Stage all modified files for commit".to_string(),
                shortcut: None,
                icon: "✓",
                payload: "action:git_stage_all".to_string(),
            },
            PaletteItem {
                id: 116,
                kind: PaletteKind::Action,
                title: "> Git: Toggle Diff View".to_string(),
                subtitle: "Side-by-side git diff comparison".to_string(),
                shortcut: None,
                icon: "⇄",
                payload: "action:git_diff".to_string(),
            },
            PaletteItem {
                id: 117,
                kind: PaletteKind::Action,
                title: "> Debug: Start Debugging".to_string(),
                subtitle: "Launch native debugger session".to_string(),
                shortcut: Some("F5".to_string()),
                icon: "▷",
                payload: "action:debug_start".to_string(),
            },
            PaletteItem {
                id: 118,
                kind: PaletteKind::Action,
                title: "> Terminal: Create New Terminal".to_string(),
                subtitle: "Open additional shell instance".to_string(),
                shortcut: Some("Cmd+Shift+`".to_string()),
                icon: "❯_",
                payload: "action:term_new".to_string(),
            },
        ];

        Self {
            is_open: false,
            query: String::new(),
            selected_index: 0,
            all_items: items,
        }
    }

    /// Opens the palette in file or action mode.
    pub fn open(&mut self, is_action_mode: bool) {
        self.is_open = true;
        self.query = if is_action_mode {
            "> ".to_string()
        } else {
            String::new()
        };
        self.selected_index = 0;
    }

    /// Closes the palette modal.
    pub fn close(&mut self) {
        self.is_open = false;
        self.query.clear();
        self.selected_index = 0;
    }

    /// Updates search query and clamps selection index.
    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.selected_index = 0;
    }

    /// Returns filtered items based on current search query.
    pub fn filtered(&self) -> Vec<&PaletteItem> {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() {
            return self.all_items.iter().collect();
        }

        let is_action_only = q.starts_with('>');
        let clean_q = if is_action_only {
            q.trim_start_matches('>').trim()
        } else {
            &q
        };

        self.all_items
            .iter()
            .filter(|item| {
                if is_action_only && item.kind != PaletteKind::Action {
                    return false;
                }
                if clean_q.is_empty() {
                    return true;
                }
                item.title.to_lowercase().contains(clean_q)
                    || item.subtitle.to_lowercase().contains(clean_q)
                    || item.payload.to_lowercase().contains(clean_q)
            })
            .collect()
    }

    /// Moves selection down.
    pub fn select_next(&mut self) {
        let count = self.filtered().len();
        if count > 0 {
            self.selected_index = (self.selected_index + 1) % count;
        }
    }

    /// Moves selection up.
    pub fn select_prev(&mut self) {
        let count = self.filtered().len();
        if count > 0 {
            self.selected_index = if self.selected_index == 0 {
                count - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    /// Returns the currently highlighted item.
    pub fn selected_item(&self) -> Option<&PaletteItem> {
        let list = self.filtered();
        list.get(self.selected_index).copied()
    }
}
