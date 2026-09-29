//! Virgo — native IDE built on Taim.

pub mod activity;
pub mod debug;
pub mod document;
mod editor_chrome;
pub mod extensions;
pub mod git;
pub mod icons;
pub mod lsp;
pub mod notifications;
pub mod palette;
pub mod recovery;
pub mod search;
pub mod terminal;
pub mod tree;

pub use activity::{ActivityBarState, SidebarView};
pub use debug::{DebugState, DebugStatus};
pub use document::{Document, DocumentBacking, DocumentManager};
pub use extensions::{ExtensionFilter, ExtensionsState};
pub use git::{DiffLine, DiffLineKind, GitFileStatus, GitState};
pub use lsp::{AutocompleteState, HoverInfo};
pub use notifications::{NotificationManager, NotificationSeverity};
pub use palette::PaletteState;
pub use recovery::{RecoveryCandidate, RecoveryEntry, RecoveryStore};
pub use search::{GlobalSearchState, ReplaceBatch, ReplaceFileChange};
pub use terminal::{BENCHMARK_DATA, LogLevel, OutputChannel, PanelTab, TerminalState};
pub use tree::{FileNode, FileTree};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use taim::widget::{
    button, column, container, mouse_area, row, rule, scrollable, space, stack, svg, text,
    text_editor, text_input, tooltip,
};
use taim::{Center, Color, Element, Fill, Length, Padding, Pixels, Subscription, Task, Theme};

/// Available UI theme variants for Taim Studio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    HighContrast,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrashEntry {
    pub stored_name: String,
    pub original_path: String,
    pub is_dir: bool,
}

/// Messages emitted and handled by Taim Studio.
#[derive(Debug, Clone)]
pub enum Message {
    /// Switch theme mode.
    ThemeSelected(ThemeMode),
    /// Toggle visibility of the primary sidebar.
    ToggleSidebar,
    /// Toggle visibility of the bottom console panel.
    ToggleTerminal,
    /// Switch active view in the Activity Bar.
    SelectSidebarView(SidebarView),
    RevealSidebarView(SidebarView),
    /// Adjust font zoom.
    ZoomIn,
    ZoomOut,
    ZoomReset,
    /// Toggle directory expansion in tree.
    TreeNodeToggled(usize),
    /// Select / open a file from tree.
    FileSelected(usize),
    /// Switch active editor tab in primary pane.
    TabSelected(usize),
    /// Close an editor tab.
    TabClosed(usize),
    CloseActiveTab,
    CycleEditor(i32),
    /// Create a new scratchpad document.
    NewScratchpad,
    OpenNewFile,
    NewFilePathChanged(String),
    CreateFile,
    OpenRenameSelected,
    RenameNameChanged(String),
    ConfirmRename,
    CancelRename,
    OpenDeleteSelected,
    ConfirmDelete,
    CancelDelete,
    RestoreTrash(usize),
    RefreshWorkspace,
    WorkspacePoll,
    /// Action performed inside the primary text editor.
    EditorAction(text_editor::Action),
    /// Action performed inside the secondary split text editor.
    SplitEditorAction(text_editor::Action),
    EditorInteraction(usize, text_editor::Action),
    EditorGoToLine(usize, usize),
    EditorScrollToLine(usize, usize),
    /// Select next occurrence of current word / selection (Cmd+D / Ctrl+D).
    SelectNextOccurrence,
    /// Clear all secondary multi-cursors (Escape).
    ClearExtraCursors,
    /// Toggle side-by-side split dual editor.
    ToggleSplitEditor,
    /// Switch active editor tab in secondary split pane.
    SplitTabSelected(usize),
    /// Focus specific pane (0 = primary, 1 = secondary).
    FocusPane(usize),
    /// Toggle in-editor Find bar.
    ToggleFind,
    RevealFind,
    CloseFind,
    /// Find query changed.
    FindQueryChanged(String),
    /// Find next match.
    FindNext,
    /// Find previous match.
    FindPrev,
    /// Open Command Palette (is_action_mode: true for `>`, false for files).
    OpenCommandPalette(bool),
    /// Close Command Palette.
    CloseCommandPalette,
    /// Palette query string changed.
    PaletteQueryChanged(String),
    /// Palette move selection down.
    PaletteSelectNext,
    /// Palette move selection up.
    PaletteSelectPrev,
    /// User chose/clicked an item from the command palette.
    PaletteItemChosen(usize),
    /// Select bottom console tab.
    TerminalTabSelected(PanelTab),
    /// Clear console logs.
    ClearTerminal,
    /// Apply recommended quick fix on a diagnostic.
    ApplyDiagnosticFix(usize),
    /// Save current document buffer.
    SaveDocument,
    OpenSaveAs,
    SaveAsPathChanged(String),
    ConfirmSaveAs,
    CancelSaveAs,
    PersistRecovery(u64),
    RestoreRecovery(usize),
    DiscardRecovery(usize),
    /// Run cargo build simulation.
    TriggerBuild,
    /// Run cargo check simulation.
    TriggerCheck,
    /// Run application simulation.
    TriggerRun,
    /// Simulated asynchronous build pipeline step.
    BuildStep(usize),
    WorkspaceCommandFinished(&'static str, bool, String),

    // --- Global Workspace Search ---
    GlobalSearchQueryChanged(String),
    SearchFinished(Option<(u64, GlobalSearchState)>),
    GlobalSearchReplaceQueryChanged(String),
    SearchHitSelected(String, usize, usize),
    ToggleSearchCase,
    ToggleSearchWord,
    ToggleSearchRegex,
    SearchIncludeChanged(String),
    SearchExcludeChanged(String),
    ToggleSearchReplace,
    OpenSearchReplace,
    PreviewWorkspaceReplace,
    ConfirmWorkspaceReplace,
    CancelWorkspaceReplace,
    UndoWorkspaceReplace,
    ToggleSearchGroup(String),

    // --- Source Control (Git) ---
    GitStageFile(String),
    GitUnstageFile(String),
    GitStageAll,
    GitUnstageAll,
    GitRefresh,
    GitCommitMessageChanged(String),
    GitCommit,
    GitToggleDiff(String),

    // --- Run & Debug ---
    DebugStart,
    DebugPause,
    DebugContinue,
    DebugStepOver,
    DebugStop,
    DebugToggleBreakpoint(String, usize),
    DebugWatchInputChanged(String),
    DebugAddWatch,
    DebugRemoveWatch(usize),

    // --- Extensions Marketplace ---
    ExtensionQueryChanged(String),
    ExtensionFilterSelected(ExtensionFilter),
    ExtensionToggleInstall(usize),
    ExtensionToggleEnabled(usize),

    // --- Multi-Terminal & Output ---
    NewTerminalTab,
    CloseTerminalTab(usize),
    SelectTerminalInstance(usize),
    TerminalInputChanged(String),
    TerminalSubmit,
    TerminalInterrupt,
    TerminalOutput(usize, Vec<u8>),
    TerminalExited(usize),
    TerminalRawInput(Vec<u8>),
    TerminalFocusChanged(bool),
    TerminalScroll(i32),
    ToggleTerminalMaximize,
    TerminalIgnored,
    SelectOutputChannel(OutputChannel),
    DebugReplInputChanged(String),
    DebugReplSubmit,

    // --- Notifications ---
    DismissNotification(usize),
    ToggleNotificationCenter,

    // --- Settings ---
    SettingsSearchChanged(String),

    // --- Autocomplete ---
    TriggerAutocomplete,
    AutocompleteSelectNext,
    AutocompleteSelectPrev,
    SelectAutocomplete(usize),
    CloseAutocomplete,
    LanguageServerEvent(lsp::ServerEvent),

    // --- Navigation & LSP ---
    GoToDefinition,
    FindReferences,
    ShowHover,
    NavigateToLocation(String, usize, usize),

    // --- Merge Conflict Resolution ---
    AcceptConflictCurrent(usize),
    AcceptConflictIncoming(usize),
    AcceptConflictBoth(usize),
    AcceptAllConflicts(document::ConflictChoice),

    // --- Code Folding ---
    ToggleFold(usize, usize),
    FoldAll,
    UnfoldAll,
}

/// Root state for Taim Studio.
pub struct Studio {
    /// Root of a real disk workspace. None is the legacy visual demo used by old tests.
    pub workspace_root: Option<PathBuf>,
    pub tree: FileTree,
    pub docs: DocumentManager,
    pub terminal: TerminalState,
    terminal_sessions: HashMap<usize, terminal::TerminalSession>,
    terminal_raw_focus: bool,
    terminal_expanded: bool,
    pub palette: PaletteState,
    pub activity: ActivityBarState,
    pub search: GlobalSearchState,
    pub replace_preview: Option<ReplaceBatch>,
    pub replace_undo: Option<ReplaceBatch>,
    search_revision: Arc<AtomicU64>,
    pub git: GitState,
    pub debug: DebugState,
    pub extensions: ExtensionsState,
    pub notifications: NotificationManager,
    pub autocomplete: AutocompleteState,
    pub hover: HoverInfo,
    language_server: Option<lsp::LanguageServer>,
    pub language_server_status: lsp::ServerStatus,
    lsp_completion_request: Option<u64>,
    lsp_definition_request: Option<u64>,
    lsp_references_request: Option<u64>,
    lsp_hover_request: Option<u64>,
    pub active_line_blame: Option<String>,
    pub show_sidebar: bool,
    pub show_terminal: bool,
    pub theme_mode: ThemeMode,
    pub font_size: f32,
    pub build_phase: usize,
    pub settings_search: String,
    pub new_file_path: String,
    pub creating_file: bool,
    pub save_as_path: String,
    pub saving_as: bool,
    pub rename_name: String,
    pub renaming_file: bool,
    pub deleting_path: Option<String>,
    pub trash_entries: Vec<TrashEntry>,
    recovery_store: Option<RecoveryStore>,
    recovery_revision: u64,
    recovery_error_reported: bool,
    workspace_fingerprint: u64,
}

/// Extracts file path, line, and column from compiler error lines or diagnostic strings.
pub fn parse_compiler_location(text: &str) -> Option<(String, usize, usize)> {
    let trimmed = text.trim();
    let stripped = trimmed.strip_prefix("--> ").unwrap_or(trimmed);
    for part in stripped.split_whitespace() {
        let clean = part.trim_matches(|c: char| c == '(' || c == ')' || c == '[' || c == ']' || c == '\'' || c == '\"');
        let mut segments = clean.split(':');
        if let (Some(file), Some(line_str), Some(col_str)) = (segments.next(), segments.next(), segments.next()) {
            if (file.ends_with(".rs") || file.ends_with(".toml") || file.ends_with(".json") || file.ends_with(".md"))
                && let (Ok(line), Ok(col)) = (line_str.parse::<usize>(), col_str.parse::<usize>())
            {
                return Some((file.to_string(), line, col));
            }
        }
    }
    None
}

#[derive(Clone, Copy, Hash)]
struct ShortcutContext {
    terminal_focused: bool,
    palette_open: bool,
    palette_selection: Option<usize>,
    find_open: bool,
    autocomplete_open: bool,
    autocomplete_selection: usize,
}

fn shortcut_message(
    key: taim::keyboard::Key<&str>,
    modifiers: taim::keyboard::Modifiers,
    context: ShortcutContext,
) -> Option<Message> {
    use taim::keyboard::Key;
    use taim::keyboard::key::Named;

    if context.autocomplete_open && !context.terminal_focused {
        match key {
            Key::Named(Named::Escape) => return Some(Message::CloseAutocomplete),
            Key::Named(Named::ArrowDown) => return Some(Message::AutocompleteSelectNext),
            Key::Named(Named::ArrowUp) => return Some(Message::AutocompleteSelectPrev),
            Key::Named(Named::Enter) | Key::Named(Named::Tab) => {
                return Some(Message::SelectAutocomplete(context.autocomplete_selection));
            }
            _ => {}
        }
    } else if context.palette_open {
        match key {
            Key::Named(Named::Escape) => return Some(Message::CloseCommandPalette),
            Key::Named(Named::ArrowDown) => return Some(Message::PaletteSelectNext),
            Key::Named(Named::ArrowUp) => return Some(Message::PaletteSelectPrev),
            Key::Named(Named::Enter) => {
                return context.palette_selection.map(Message::PaletteItemChosen);
            }
            _ => {}
        }
    } else if context.find_open && !context.terminal_focused {
        match key {
            Key::Named(Named::Escape) => return Some(Message::CloseFind),
            Key::Named(Named::Enter) if modifiers.shift() => return Some(Message::FindPrev),
            Key::Named(Named::Enter) => return Some(Message::FindNext),
            _ => {}
        }
    }

    if matches!(key, Key::Named(Named::Escape)) {
        return Some(Message::ClearExtraCursors);
    }

    if matches!(key, Key::Named(Named::F1)) {
        return Some(Message::OpenCommandPalette(true));
    }
    if matches!(key, Key::Named(Named::F5)) {
        return Some(Message::TriggerRun);
    }
    if matches!(key, Key::Named(Named::F12)) {
        return Some(if modifiers.shift() {
            Message::FindReferences
        } else {
            Message::GoToDefinition
        });
    }
    if modifiers.control() && matches!(key, Key::Named(Named::Tab)) {
        return Some(Message::CycleEditor(if modifiers.shift() { -1 } else { 1 }));
    }
    let character = match key {
        Key::Character(value) if value.len() == 1 => value.as_bytes()[0].to_ascii_lowercase(),
        _ => 0,
    };
    if modifiers.control() && matches!(character, b'`' | b'~') {
        return Some(if modifiers.shift() {
            Message::NewTerminalTab
        } else {
            Message::ToggleTerminal
        });
    }
    if modifiers.control() && character == b' ' {
        return Some(Message::TriggerAutocomplete);
    }
    if character == b'g'
        && modifiers.shift()
        && ((cfg!(target_os = "macos") && modifiers.control())
            || (!cfg!(target_os = "macos") && modifiers.command()))
    {
        return Some(Message::RevealSidebarView(SidebarView::SourceControl));
    }

    if modifiers.command() && !modifiers.alt() {
        match (character, modifiers.shift()) {
            (b'p', true) => return Some(Message::OpenCommandPalette(true)),
            (b'p', false) => return Some(Message::OpenCommandPalette(false)),
            (b'e', true) => return Some(Message::RevealSidebarView(SidebarView::Explorer)),
            (b'f', true) => return Some(Message::RevealSidebarView(SidebarView::Search)),
            (b'h', true) => return Some(Message::OpenSearchReplace),
            (b'g', true) if cfg!(target_os = "macos") => return Some(Message::FindPrev),
            (b'd', true) => return Some(Message::RevealSidebarView(SidebarView::Debug)),
            (b'd', false) => return Some(Message::SelectNextOccurrence),
            (b'x', true) => return Some(Message::RevealSidebarView(SidebarView::Extensions)),
            (b'b', true) => return Some(Message::TriggerBuild),
            (b'b', false) => return Some(Message::ToggleSidebar),
            (b'j', false) => return Some(Message::ToggleTerminal),
            (b'\\', false) => return Some(Message::ToggleSplitEditor),
            (b'f', false) => return Some(Message::RevealFind),
            (b'g', false) if cfg!(target_os = "macos") => return Some(Message::FindNext),
            (b's', false) => return Some(Message::SaveDocument),
            (b's', true) => return Some(Message::OpenSaveAs),
            (b'n', false) => return Some(Message::OpenNewFile),
            (b'w', false) => return Some(Message::CloseActiveTab),
            (b',', false) => return Some(Message::RevealSidebarView(SidebarView::Settings)),
            (b'+' | b'=', _) => return Some(Message::ZoomIn),
            (b'-', false) => return Some(Message::ZoomOut),
            (b'0', false) => return Some(Message::ZoomReset),
            _ => {}
        }
    }
    if context.terminal_focused {
        return terminal::keyboard_bytes(key, modifiers).map(Message::TerminalRawInput);
    }
    None
}

#[cfg(test)]
mod shortcut_tests {
    use super::*;
    use taim::keyboard::{Key, Modifiers, key::Named};

    fn context() -> ShortcutContext {
        ShortcutContext {
            terminal_focused: false,
            palette_open: false,
            palette_selection: None,
            find_open: false,
            autocomplete_open: false,
            autocomplete_selection: 0,
        }
    }

    #[test]
    fn primary_shortcuts_route_to_working_actions() {
        let cmd = Modifiers::COMMAND;
        assert!(matches!(
            shortcut_message(Key::Character("p"), cmd, context()),
            Some(Message::OpenCommandPalette(false))
        ));
        assert!(matches!(
            shortcut_message(Key::Character("p"), cmd | Modifiers::SHIFT, context()),
            Some(Message::OpenCommandPalette(true))
        ));
        assert!(matches!(
            shortcut_message(Key::Character("s"), cmd, context()),
            Some(Message::SaveDocument)
        ));
        assert!(matches!(
            shortcut_message(Key::Character("w"), cmd, context()),
            Some(Message::CloseActiveTab)
        ));
        assert!(matches!(
            shortcut_message(Key::Character("f"), cmd | Modifiers::SHIFT, context()),
            Some(Message::RevealSidebarView(SidebarView::Search))
        ));
        assert!(matches!(
            shortcut_message(Key::Character("d"), cmd, context()),
            Some(Message::SelectNextOccurrence)
        ));
        assert!(matches!(
            shortcut_message(Key::Named(Named::Escape), Modifiers::NONE, context()),
            Some(Message::ClearExtraCursors)
        ));
    }

    #[test]
    fn palette_find_and_terminal_keep_their_own_keys() {
        let palette = ShortcutContext {
            palette_open: true,
            palette_selection: Some(42),
            ..context()
        };
        assert!(matches!(
            shortcut_message(Key::Named(Named::Enter), Modifiers::NONE, palette),
            Some(Message::PaletteItemChosen(42))
        ));
        assert!(matches!(
            shortcut_message(Key::Named(Named::ArrowDown), Modifiers::NONE, palette),
            Some(Message::PaletteSelectNext)
        ));
        let find = ShortcutContext {
            find_open: true,
            ..context()
        };
        assert!(matches!(
            shortcut_message(Key::Named(Named::Enter), Modifiers::SHIFT, find),
            Some(Message::FindPrev)
        ));
        let terminal = ShortcutContext {
            terminal_focused: true,
            ..context()
        };
        assert!(
            matches!(shortcut_message(Key::Character("c"), Modifiers::CTRL, terminal), Some(Message::TerminalRawInput(bytes)) if bytes == [3])
        );
        assert!(matches!(
            shortcut_message(Key::Character("p"), Modifiers::COMMAND, terminal),
            Some(Message::OpenCommandPalette(false))
        ));
        assert!(matches!(
            shortcut_message(Key::Named(Named::Tab), Modifiers::CTRL, terminal),
            Some(Message::CycleEditor(1))
        ));
    }
}

impl Default for Studio {
    fn default() -> Self {
        Self::new().0
    }
}

impl Studio {
    /// Initializes Taim Studio state with pre-populated project and documents.
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                workspace_root: None,
                tree: FileTree::default_project(),
                docs: DocumentManager::new(),
                terminal: TerminalState::new(),
                terminal_sessions: HashMap::new(),
                terminal_raw_focus: false,
                terminal_expanded: false,
                palette: PaletteState::new(),
                activity: ActivityBarState::new(),
                search: GlobalSearchState::new(),
                replace_preview: None,
                replace_undo: None,
                search_revision: Arc::new(AtomicU64::new(0)),
                git: GitState::new(),
                debug: DebugState::new(),
                extensions: ExtensionsState::new(),
                notifications: NotificationManager::new(),
                autocomplete: AutocompleteState::new(),
                hover: HoverInfo::new(),
                language_server: None,
                language_server_status: lsp::ServerStatus::Unavailable,
                lsp_completion_request: None,
                lsp_definition_request: None,
                lsp_references_request: None,
                lsp_hover_request: None,
                active_line_blame: None,
                show_sidebar: true,
                show_terminal: true,
                theme_mode: ThemeMode::Dark,
                font_size: 13.0,
                build_phase: 0,
                settings_search: String::new(),
                new_file_path: String::new(),
                creating_file: false,
                save_as_path: String::new(),
                saving_as: false,
                rename_name: String::new(),
                renaming_file: false,
                deleting_path: None,
                trash_entries: Vec::new(),
                recovery_store: None,
                recovery_revision: 0,
                recovery_error_reported: false,
                workspace_fingerprint: 0,
            },
            Task::none(),
        )
    }

    /// Opens a real directory without loading any of the prototype's sample documents.
    pub fn with_workspace(root: PathBuf) -> std::io::Result<(Self, Task<Message>)> {
        let (mut studio, task) = Self::with_workspace_core(root)?;
        let recovery = RecoveryStore::default_base().and_then(|base| {
            RecoveryStore::open(
                &base,
                studio.workspace_root.as_ref().expect("workspace root"),
            )
        });
        match recovery {
            Ok(store) => studio.install_recovery(store),
            Err(error) => studio.report_error("Recovery unavailable", error.to_string()),
        }
        Ok((studio, task))
    }

    /// Opens a workspace with a specified recovery location, useful for portable deployments.
    pub fn with_workspace_and_recovery(
        root: PathBuf,
        recovery_base: PathBuf,
    ) -> std::io::Result<(Self, Task<Message>)> {
        let (mut studio, task) = Self::with_workspace_core(root)?;
        let store = RecoveryStore::open(
            &recovery_base,
            studio.workspace_root.as_ref().expect("workspace root"),
        )?;
        studio.install_recovery(store);
        Ok((studio, task))
    }

    fn with_workspace_core(root: PathBuf) -> std::io::Result<(Self, Task<Message>)> {
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "workspace must be a directory",
            ));
        }
        let tree = FileTree::from_path(&root);
        let git = GitState::for_workspace(&root);
        let workspace_fingerprint = workspace_fingerprint(&root, &tree);
        let selected_id = tree.selected_id;
        let mut palette = PaletteState::new();
        palette.all_items.retain(|item| {
            matches!(item.kind, palette::PaletteKind::Action)
                && !matches!(
                    item.payload.as_str(),
                    "action:run"
                        | "action:git_stage_all"
                        | "action:git_diff"
                        | "action:debug_start"
                )
        });
        if let Some(item) = palette
            .all_items
            .iter_mut()
            .find(|item| item.payload == "action:new_scratch")
        {
            item.title = "> File: New File".to_string();
            item.subtitle = "Create a file in this workspace".to_string();
            item.payload = "action:new_file".to_string();
        }
        let git_changes_badge = git.total_changes();
        let mut studio = Self {
            workspace_root: Some(root.clone()),
            tree,
            docs: DocumentManager::empty_workspace(),
            terminal: TerminalState::empty(),
            terminal_sessions: HashMap::new(),
            terminal_raw_focus: false,
            terminal_expanded: false,
            palette,
            activity: ActivityBarState {
                active_view: SidebarView::Explorer,
                git_changes_badge,
                diagnostics_badge: 0,
                extensions_badge: 0,
            },
            search: GlobalSearchState::default(),
            replace_preview: None,
            replace_undo: None,
            search_revision: Arc::new(AtomicU64::new(0)),
            git,
            debug: DebugState::empty(),
            extensions: ExtensionsState::new(),
            notifications: NotificationManager::new(),
            autocomplete: AutocompleteState::new(),
            hover: HoverInfo::new(),
            language_server: None,
            language_server_status: lsp::ServerStatus::Unavailable,
            lsp_completion_request: None,
            lsp_definition_request: None,
            lsp_references_request: None,
            lsp_hover_request: None,
            active_line_blame: None,
            show_sidebar: true,
            show_terminal: true,
            theme_mode: ThemeMode::Dark,
            font_size: 13.0,
            build_phase: 0,
            settings_search: String::new(),
            new_file_path: String::new(),
            creating_file: false,
            save_as_path: String::new(),
            saving_as: false,
            rename_name: String::new(),
            renaming_file: false,
            deleting_path: None,
            trash_entries: Vec::new(),
            recovery_store: None,
            recovery_revision: 0,
            recovery_error_reported: false,
            workspace_fingerprint,
        };
        studio.refresh_trash_entries();
        studio.refresh_palette_files();
        if let Some(id) = selected_id {
            studio.open_file(id);
        }
        Ok((studio, Task::none()))
    }

    fn install_recovery(&mut self, store: RecoveryStore) {
        let count = store.pending().len();
        let unreadable = store.unreadable_count();
        self.recovery_store = Some(store);
        if count > 0 {
            self.notifications.push(
                NotificationSeverity::Warning,
                "Unsaved drafts found",
                format!("Review {count} draft(s) in Explorer"),
                None,
                None,
            );
        }
        if unreadable > 0 {
            self.notifications.push(
                NotificationSeverity::Warning,
                "Recovery drafts need inspection",
                format!("{unreadable} unreadable snapshot(s) were preserved on disk"),
                None,
                None,
            );
        }
    }

    /// Starts rust-analyzer for Rust workspaces. Missing servers remain an explicit state.
    pub fn start_language_server(&mut self) -> Task<Message> {
        let Some(root) = self.workspace_root.clone() else {
            return Task::none();
        };
        if !root.join("Cargo.toml").is_file() {
            self.language_server_status = lsp::ServerStatus::Unavailable;
            return Task::none();
        }
        match lsp::LanguageServer::start(&root) {
            Ok((server, task)) => {
                self.language_server = Some(server);
                self.language_server_status = lsp::ServerStatus::Starting;
                self.terminal.log(LogLevel::Info, "Starting rust-analyzer");
                task
            }
            Err(error) => {
                self.language_server_status = lsp::ServerStatus::Unavailable;
                self.terminal.output_logs.push(terminal::LogEntry {
                    time: "now".to_string(),
                    level: LogLevel::Warning,
                    message: format!("rust-analyzer unavailable: {error}"),
                });
                Task::none()
            }
        }
    }

    fn sync_open_documents_to_language_server(&mut self) {
        let documents: Vec<(String, String, String)> = self
            .docs
            .documents
            .iter()
            .filter(|doc| doc.disk_snapshot.is_some())
            .map(|doc| {
                (
                    doc.path.clone(),
                    doc.text(),
                    lsp::language_id(&doc.path).to_string(),
                )
            })
            .collect();
        let Some(server) = &mut self.language_server else {
            return;
        };
        for (path, text, language) in documents {
            if let Err(error) = server.did_open(&path, &text, &language) {
                self.language_server_status = lsp::ServerStatus::Failed;
                self.report_error("Language server sync failed", error.to_string());
                break;
            }
        }
    }

    fn sync_document_change_to_language_server(&mut self, index: usize) {
        if self.language_server_status != lsp::ServerStatus::Ready {
            return;
        }
        let Some(doc) = self.docs.documents.get(index) else {
            return;
        };
        let (path, content) = (doc.path.clone(), doc.text());
        let error = self
            .language_server
            .as_mut()
            .and_then(|server| server.did_change(&path, &content).err());
        if let Some(error) = error {
            self.language_server_status = lsp::ServerStatus::Failed;
            self.report_error("Language server sync failed", error.to_string());
        }
    }

    fn publish_language_diagnostics(&mut self, value: &serde_json::Value) {
        let Some((uri, diagnostics)) = lsp::parse_diagnostics(value) else {
            return;
        };
        let Some(path) = self
            .language_server
            .as_ref()
            .and_then(|server| server.relative_path(uri))
        else {
            return;
        };
        self.terminal
            .diagnostics
            .retain(|diagnostic| diagnostic.file != path);
        self.terminal
            .diagnostics
            .extend(
                diagnostics
                    .into_iter()
                    .map(|diagnostic| terminal::Diagnostic {
                        file: path.clone(),
                        line: diagnostic.line,
                        col: diagnostic.column,
                        severity: match diagnostic.severity {
                            1 => LogLevel::Error,
                            2 => LogLevel::Warning,
                            _ => LogLevel::Info,
                        },
                        code: diagnostic.code,
                        message: diagnostic.message,
                        suggested_fix: None,
                    }),
            );
        self.activity.diagnostics_badge = self.terminal.diagnostics.len();
    }

    /// Number of drafts awaiting an explicit restore or discard decision.
    pub fn pending_recovery_count(&self) -> usize {
        self.recovery_store
            .as_ref()
            .map_or(0, |store| store.pending().len())
    }

    fn refresh_palette_files(&mut self) {
        self.palette
            .all_items
            .retain(|item| item.kind != palette::PaletteKind::File);
        fn add_files(node: &FileNode, palette: &mut PaletteState) {
            if node.is_dir {
                for child in &node.children {
                    add_files(child, palette);
                }
            } else {
                palette.all_items.push(palette::PaletteItem {
                    id: node.id + 10_000,
                    kind: palette::PaletteKind::File,
                    title: node.name.clone(),
                    subtitle: node.path.clone(),
                    shortcut: None,
                    icon: node.icon(),
                    payload: node.path.clone(),
                });
            }
        }
        add_files(&self.tree.root, &mut self.palette);
    }

    fn create_workspace_file(&mut self) {
        let relative = self.new_file_path.trim().to_string();
        let Some(root) = self.workspace_root.as_ref() else {
            return;
        };
        let Some(path) = self.disk_path(&relative) else {
            self.report_error(
                "Create failed",
                "Enter a relative file path without '..'".to_string(),
            );
            return;
        };
        let parent = match path.parent().and_then(|p| p.canonicalize().ok()) {
            Some(parent) if parent.starts_with(root) => parent,
            _ => {
                self.report_error(
                    "Create failed",
                    "The parent folder must exist inside this workspace".to_string(),
                );
                return;
            }
        };
        let Some(name) = path.file_name() else {
            self.report_error("Create failed", "Enter a file name".to_string());
            return;
        };
        let target = parent.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(_) => {
                self.tree = FileTree::from_path(root);
                self.refresh_palette_files();
                if let Some(node) = self.tree.root.find_by_path(&relative) {
                    self.open_file(node.id);
                }
                self.creating_file = false;
                self.new_file_path.clear();
                self.terminal
                    .log(LogLevel::Success, format!("Created {}", relative));
                self.refresh_git();
            }
            Err(error) => self.report_error("Create failed", format!("{}: {}", relative, error)),
        }
    }

    fn save_as_workspace_file(&mut self) {
        let Some(root) = self.workspace_root.clone() else {
            self.report_error("Save As failed", "Open a workspace first".to_string());
            return;
        };
        let Some(doc) = self.docs.active() else {
            self.report_error("Save As failed", "No document is open".to_string());
            return;
        };
        let content = doc.text();
        let relative = self.save_as_path.trim().to_string();
        let Some(target) = self.disk_path(&relative) else {
            self.report_error(
                "Save As failed",
                "Enter a relative path without '..'".to_string(),
            );
            return;
        };
        let parent = match target.parent().and_then(|path| path.canonicalize().ok()) {
            Some(parent) if parent.starts_with(&root) => parent,
            _ => {
                self.report_error(
                    "Save As failed",
                    "The destination folder must exist inside the workspace".to_string(),
                );
                return;
            }
        };
        let Some(name) = target.file_name().and_then(|name| name.to_str()) else {
            self.report_error("Save As failed", "The file name is invalid".to_string());
            return;
        };
        let target = parent.join(name);
        if let Err(error) = write_new_atomic(&target, content.as_bytes()) {
            self.report_error("Save As failed", format!("{relative}: {error}"));
            return;
        }
        self.tree.rescan(&root);
        self.refresh_palette_files();
        let node_id = self.tree.root.find_by_path(&relative).map(|node| node.id);
        let syntax = tree::syntax_for_path(&target).to_string();
        if let Some(doc) = self.docs.active_mut() {
            doc.id = node_id.unwrap_or(usize::MAX);
            doc.path = relative.clone();
            doc.name = name.to_string();
            doc.syntax = syntax;
            doc.disk_snapshot = Some(content.clone());
            doc.is_dirty = false;
        }
        self.tree.selected_id = node_id;
        self.saving_as = false;
        self.save_as_path.clear();
        if self.language_server_status == lsp::ServerStatus::Ready {
            if let Some(server) = &mut self.language_server {
                let _ = server.did_open(&relative, &content, lsp::language_id(&relative));
            }
        }
        self.flush_recovery();
        self.refresh_git();
        self.terminal
            .log(LogLevel::Success, format!("Saved as {relative}"));
    }

    fn rename_selected_file(&mut self) {
        let Some(id) = self.tree.selected_id else {
            self.report_error("Rename failed", "Select a file first".to_string());
            return;
        };
        let Some(node) = self.tree.find(id).filter(|node| !node.path.is_empty()) else {
            self.report_error("Rename failed", "Select a file or folder first".to_string());
            return;
        };
        let old_path = node.path.clone();
        let is_dir = node.is_dir;
        let new_name = self.rename_name.trim().to_string();
        let mut components = Path::new(&new_name).components();
        if !matches!(components.next(), Some(std::path::Component::Normal(_)))
            || components.next().is_some()
        {
            self.report_error(
                "Rename failed",
                "Enter one file name without a path".to_string(),
            );
            return;
        }
        let Some(source) = self.disk_path(&old_path) else {
            self.report_error("Rename failed", "Invalid source path".to_string());
            return;
        };
        let Some(parent) = source.parent() else {
            self.report_error("Rename failed", "Invalid source path".to_string());
            return;
        };
        if parent.canonicalize().ok().as_ref().is_none_or(|path| {
            !self
                .workspace_root
                .as_ref()
                .is_some_and(|root| path.starts_with(root))
        }) {
            self.report_error(
                "Rename failed",
                "The file's parent folder is outside the workspace".to_string(),
            );
            return;
        }
        let target = parent.join(&new_name);
        if target.exists() {
            self.report_error(
                "Rename failed",
                format!("{new_name} already exists in this folder"),
            );
            return;
        }
        let new_path = old_path
            .rsplit_once('/')
            .map(|(parent, _)| format!("{parent}/{new_name}"))
            .unwrap_or_else(|| new_name.clone());
        let result = (|| -> std::io::Result<()> {
            if source.symlink_metadata()?.file_type().is_symlink() {
                return Err(std::io::Error::other(
                    "symbolic links cannot be renamed here",
                ));
            }
            if is_dir {
                std::fs::rename(&source, &target)?;
            } else {
                std::fs::hard_link(&source, &target)?;
                if let Err(error) = std::fs::remove_file(&source) {
                    let _ = std::fs::remove_file(&target);
                    return Err(error);
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.report_error("Rename failed", format!("{old_path}: {error}"));
            return;
        }
        let Some(root) = self.workspace_root.clone() else {
            return;
        };
        self.tree.rescan(&root);
        self.refresh_palette_files();
        let node_id = self.tree.root.find_by_path(&new_path).map(|node| node.id);
        let mut renamed_documents = Vec::new();
        for doc in &mut self.docs.documents {
            if path_matches_entry(&doc.path, &old_path) {
                let previous_path = doc.path.clone();
                let suffix = doc.path.strip_prefix(&old_path).unwrap_or_default();
                doc.path = format!("{new_path}{suffix}");
                doc.name = Path::new(&doc.path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&doc.name)
                    .to_string();
                doc.syntax = tree::syntax_for_path(Path::new(&doc.path)).to_string();
                doc.id = self
                    .tree
                    .root
                    .find_by_path(&doc.path)
                    .map_or(usize::MAX, |node| node.id);
                renamed_documents.push((
                    previous_path,
                    doc.path.clone(),
                    doc.text(),
                    lsp::language_id(&doc.path),
                ));
            }
        }
        if self.language_server_status == lsp::ServerStatus::Ready {
            if let Some(server) = &mut self.language_server {
                for (old, new, text, language) in renamed_documents {
                    let _ = server.did_close(&old);
                    let _ = server.did_open(&new, &text, language);
                }
            }
        }
        self.tree.selected_id = node_id;
        self.renaming_file = false;
        self.rename_name.clear();
        self.flush_recovery();
        self.refresh_git();
        self.terminal.log(
            LogLevel::Success,
            format!("Renamed {old_path} to {new_path}"),
        );
    }

    fn delete_selected_entry(&mut self) {
        let Some(relative) = self.deleting_path.clone() else {
            return;
        };
        if relative.is_empty() {
            self.report_error(
                "Delete failed",
                "The workspace root cannot be deleted".to_string(),
            );
            return;
        }
        if self
            .docs
            .documents
            .iter()
            .any(|doc| doc.is_dirty && path_matches_entry(&doc.path, &relative))
        {
            self.report_error(
                "Delete blocked",
                "Save or close dirty files inside this entry before deleting it".to_string(),
            );
            return;
        }
        let Some(root) = self.workspace_root.clone() else {
            return;
        };
        let Some(source) = self.disk_path(&relative) else {
            self.report_error("Delete failed", "Invalid workspace path".to_string());
            return;
        };
        let result = (|| -> std::io::Result<(PathBuf, TrashEntry)> {
            let metadata = source.symlink_metadata()?;
            if metadata.file_type().is_symlink() {
                return Err(std::io::Error::other(
                    "symbolic links cannot be deleted here",
                ));
            }
            let parent = source
                .parent()
                .and_then(|path| path.canonicalize().ok())
                .ok_or_else(|| std::io::Error::other("entry parent is unavailable"))?;
            if !parent.starts_with(&root) {
                return Err(std::io::Error::other("entry is outside the workspace"));
            }
            let trash = root.join(".virgo-trash");
            std::fs::create_dir_all(&trash)?;
            if !trash.canonicalize()?.starts_with(&root) {
                return Err(std::io::Error::other(
                    "trash folder is outside the workspace",
                ));
            }
            let name = source
                .file_name()
                .ok_or_else(|| std::io::Error::other("entry has no name"))?
                .to_string_lossy();
            for _ in 0..100 {
                let target = trash.join(format!(
                    "{}-{}-{name}",
                    std::process::id(),
                    SAVE_ID.fetch_add(1, Ordering::Relaxed)
                ));
                if target.exists() {
                    continue;
                }
                std::fs::rename(&source, &target)?;
                let entry = TrashEntry {
                    stored_name: target
                        .file_name()
                        .expect("trash target has a name")
                        .to_string_lossy()
                        .to_string(),
                    original_path: relative.clone(),
                    is_dir: metadata.is_dir(),
                };
                let manifest = trash_manifest_path(&trash, &entry.stored_name);
                let encoded = serde_json::to_vec(&entry).map_err(std::io::Error::other)?;
                if let Err(error) = write_new_atomic(&manifest, &encoded) {
                    let _ = std::fs::rename(&target, &source);
                    return Err(error);
                }
                return Ok((target, entry));
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "could not allocate a unique trash name",
            ))
        })();
        let (target, _) = match result {
            Ok(result) => result,
            Err(error) => {
                self.report_error("Delete failed", format!("{relative}: {error}"));
                return;
            }
        };
        let closed_paths: Vec<String> = self
            .docs
            .documents
            .iter()
            .filter(|doc| path_matches_entry(&doc.path, &relative))
            .map(|doc| doc.path.clone())
            .collect();
        if let Some(server) = &mut self.language_server {
            for path in &closed_paths {
                let _ = server.did_close(path);
            }
        }
        self.docs
            .remove_matching(|doc| path_matches_entry(&doc.path, &relative));
        self.tree.rescan(&root);
        self.refresh_palette_files();
        self.tree.selected_id = None;
        self.deleting_path = None;
        self.refresh_trash_entries();
        self.flush_recovery();
        self.refresh_git();
        self.terminal.log(
            LogLevel::Success,
            format!("Moved {relative} to {}", target.display()),
        );
    }

    fn refresh_trash_entries(&mut self) {
        self.trash_entries.clear();
        let Some(root) = &self.workspace_root else {
            return;
        };
        let trash = root.join(".virgo-trash");
        let Ok(items) = std::fs::read_dir(&trash) else {
            return;
        };
        for item in items.flatten() {
            let path = item.path();
            if !item.file_type().is_ok_and(|kind| kind.is_file())
                || !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".virgo-trash.json"))
            {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let Ok(entry) = serde_json::from_slice::<TrashEntry>(&bytes) else {
                continue;
            };
            if Path::new(&entry.stored_name).components().count() != 1
                || self.disk_path(&entry.original_path).is_none()
                || !trash.join(&entry.stored_name).exists()
            {
                continue;
            }
            self.trash_entries.push(entry);
        }
        self.trash_entries
            .sort_by(|a, b| a.original_path.cmp(&b.original_path));
    }

    fn restore_trash_entry(&mut self, index: usize) {
        let Some(entry) = self.trash_entries.get(index).cloned() else {
            return;
        };
        let Some(root) = self.workspace_root.clone() else {
            return;
        };
        let Some(target) = self.disk_path(&entry.original_path) else {
            self.report_error("Restore failed", "Invalid original path".to_string());
            return;
        };
        let result = (|| -> std::io::Result<()> {
            if target.exists() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "the original path is already occupied",
                ));
            }
            let parent = target
                .parent()
                .and_then(|path| path.canonicalize().ok())
                .ok_or_else(|| std::io::Error::other("the original parent folder is missing"))?;
            if !parent.starts_with(&root) {
                return Err(std::io::Error::other(
                    "the original parent is outside the workspace",
                ));
            }
            let trash = root.join(".virgo-trash");
            if !trash.canonicalize()?.starts_with(&root) {
                return Err(std::io::Error::other(
                    "trash folder is outside the workspace",
                ));
            }
            let source = trash.join(&entry.stored_name);
            if source.symlink_metadata()?.file_type().is_symlink() {
                return Err(std::io::Error::other("trash entry is a symbolic link"));
            }
            std::fs::rename(&source, &target)?;
            let manifest = trash_manifest_path(&trash, &entry.stored_name);
            if let Err(error) = std::fs::remove_file(&manifest) {
                let _ = std::fs::rename(&target, &source);
                return Err(error);
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.report_error(
                "Restore failed",
                format!("{}: {error}", entry.original_path),
            );
            return;
        }
        self.tree.rescan(&root);
        self.refresh_palette_files();
        self.refresh_trash_entries();
        self.workspace_fingerprint = workspace_fingerprint(&root, &self.tree);
        self.refresh_git();
        self.terminal.log(
            LogLevel::Success,
            format!("Restored {}", entry.original_path),
        );
    }

    fn disk_path(&self, relative: &str) -> Option<PathBuf> {
        let root = self.workspace_root.as_ref()?;
        let path = Path::new(relative);
        if path.is_absolute()
            || path
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return None;
        }
        Some(root.join(path))
    }

    fn reconcile_workspace(&mut self, announce: bool) {
        let Some(root) = self.workspace_root.clone() else {
            return;
        };
        self.tree.rescan(&root);
        let mut vanished = Vec::new();
        for doc in &mut self.docs.documents {
            let disk_path = root.join(&doc.path);
            if doc.disk_snapshot.is_none() {
                continue;
            }
            let safe_file = disk_path
                .parent()
                .and_then(|parent| parent.canonicalize().ok())
                .is_some_and(|parent| parent.starts_with(&root))
                && disk_path
                    .symlink_metadata()
                    .is_ok_and(|metadata| metadata.file_type().is_file());
            if !safe_file {
                if !doc.is_dirty {
                    vanished.push(doc.path.clone());
                }
                continue;
            }
            match std::fs::read_to_string(&disk_path) {
                Ok(content) if !doc.is_dirty && doc.disk_snapshot.as_ref() != Some(&content) => {
                    doc.content = text_editor::Content::with_text(&content);
                    doc.disk_snapshot = Some(content.clone());
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound && !doc.is_dirty => {
                    vanished.push(doc.path.clone());
                }
                Err(_) => {}
            }
            doc.id = self
                .tree
                .root
                .find_by_path(&doc.path)
                .map_or(usize::MAX, |node| node.id);
        }
        if !vanished.is_empty() {
            self.docs
                .remove_matching(|doc| vanished.iter().any(|path| path == &doc.path));
        }
        self.tree.selected_id = self
            .docs
            .active()
            .and_then(|doc| self.tree.root.find_by_path(&doc.path).map(|node| node.id));
        self.refresh_palette_files();
        self.refresh_trash_entries();
        self.workspace_fingerprint = workspace_fingerprint(&root, &self.tree);
        if announce {
            self.terminal.log(LogLevel::Info, "Explorer refreshed");
        }
        self.refresh_git();
        if self.show_sidebar
            && self.activity.active_view == SidebarView::Search
            && !self.search.query.is_empty()
        {
            self.refresh_search();
        }
    }

    fn open_file(&mut self, id: usize) {
        let Some(node) = self.tree.find(id).filter(|n| !n.is_dir) else {
            return;
        };
        let (path, name, syntax) = (node.path.clone(), node.name.clone(), node.syntax);
        if let Some(index) = self.docs.documents.iter().position(|doc| doc.path == path) {
            self.docs.active_tab = index;
            self.docs.focused_pane = 0;
            self.tree.selected_id = Some(id);
            return;
        }
        if let Some(disk_path) = self.disk_path(&path) {
            match self.docs.open_disk_file(id, &path, &name, syntax, &disk_path) {
                Ok(()) => {
                    self.docs.focused_pane = 0;
                    self.tree.selected_id = Some(id);
                }
                Err(error) => self.report_error("Open failed", format!("{}: {}", path, error)),
            }
        } else {
            self.docs.open_or_focus(id, &path, &name, syntax);
            self.tree.selected_id = Some(id);
        }
        if self.language_server_status == lsp::ServerStatus::Ready {
            let index = self.docs.active_tab;
            if let Some(doc) = self.docs.documents.get(index) {
                let (path, content, language) =
                    (doc.path.clone(), doc.text(), lsp::language_id(&doc.path));
                if let Some(error) = self
                    .language_server
                    .as_mut()
                    .and_then(|server| server.did_open(&path, &content, language).err())
                {
                    self.language_server_status = lsp::ServerStatus::Failed;
                    self.report_error("Language server sync failed", error.to_string());
                }
            }
        }
        self.refresh_active_line_blame();
    }

    pub fn jump_to_file_location(&mut self, path: &str, line: usize, column: usize) {
        let clean_path = path.trim_start_matches("./").replace('\\', "/");
        let node_id = self.tree.root.find_by_path(&clean_path).map(|node| node.id);
        if let Some(id) = node_id {
            self.open_file(id);
        } else if let Some(disk_path) = self.disk_path(&clean_path) {
            let id = self.docs.reserve_virtual_id();
            let name = clean_path.split('/').last().unwrap_or(&clean_path).to_string();
            let syntax = crate::tree::syntax_for_path(&disk_path);
            let _ = self.docs.open_disk_file(id, &clean_path, &name, syntax, &disk_path);
        }
        if let Some(doc) = self.docs.active_mut().filter(|d| d.path == clean_path || d.path.ends_with(&clean_path)) {
            let line_index = line.saturating_sub(1).min(doc.line_count().saturating_sub(1));
            let byte_column = doc
                .content
                .line(line_index)
                .map(|l| {
                    l.text
                        .char_indices()
                        .nth(column.saturating_sub(1))
                        .map(|(byte, _)| byte)
                        .unwrap_or(l.text.len())
                })
                .unwrap_or(0);
            doc.clear_extra_cursors();
            doc.content
                .move_to(text_editor::Cursor::new(text_editor::Position::new(
                    line_index,
                    byte_column,
                )));
        }
        self.refresh_active_line_blame();
    }

    pub fn fast_simd_definition_jump(&mut self, token: &str) -> bool {
        // 1. Search open documents first (0 latency)
        for doc in &self.docs.documents {
            let line_count = doc.line_count();
            for i in 0..line_count {
                if let Some(line) = doc.content.line(i) {
                    let trimmed = line.text.trim();
                    if trimmed.starts_with(&format!("fn {token}"))
                        || trimmed.starts_with(&format!("pub fn {token}"))
                        || trimmed.starts_with(&format!("struct {token}"))
                        || trimmed.starts_with(&format!("pub struct {token}"))
                        || trimmed.starts_with(&format!("enum {token}"))
                        || trimmed.starts_with(&format!("pub enum {token}"))
                        || trimmed.starts_with(&format!("trait {token}"))
                        || trimmed.starts_with(&format!("pub trait {token}"))
                        || trimmed.starts_with(&format!("type {token}"))
                        || trimmed.starts_with(&format!("pub type {token}"))
                        || trimmed.starts_with(&format!("const {token}"))
                        || trimmed.starts_with(&format!("let {token}"))
                        || trimmed.starts_with(&format!("let mut {token}"))
                        || trimmed.starts_with(&format!("def {token}"))
                        || trimmed.starts_with(&format!("class {token}"))
                    {
                        let path = doc.path.clone();
                        self.jump_to_file_location(&path, i + 1, 1);
                        self.terminal.log(LogLevel::Success, format!("Jumped to definition of `{token}` in {}:{}:1", path, i + 1));
                        return true;
                    }
                }
            }
        }

        // 2. Search workspace files on disk via tree & memory map
        if let Some(root) = &self.workspace_root {
            let patterns = [
                format!("fn {token}"),
                format!("struct {token}"),
                format!("enum {token}"),
                format!("trait {token}"),
                format!("type {token}"),
                format!("class {token}"),
                format!("def {token}"),
            ];
            fn search_node(
                root: &std::path::Path,
                node: &FileNode,
                patterns: &[String],
            ) -> Option<(String, usize)> {
                if node.is_dir {
                    for child in &node.children {
                        if let Some(res) = search_node(root, child, patterns) {
                            return Some(res);
                        }
                    }
                } else {
                    let ext = node.path.split('.').last().unwrap_or("");
                    if matches!(ext, "rs" | "toml" | "py" | "js" | "ts" | "c" | "cpp" | "h") {
                        let disk_path = root.join(&node.path);
                        if let Ok(file) = std::fs::File::open(&disk_path) {
                            if let Ok(mmap) = unsafe { memmap2::Mmap::map(&file) } {
                                for pat in patterns {
                                    let pat_bytes = pat.as_bytes();
                                    if let Some(offset) = mmap
                                        .windows(pat_bytes.len())
                                        .position(|window| window == pat_bytes)
                                    {
                                        let line_num = mmap[..offset]
                                            .iter()
                                            .filter(|&&b| b == b'\n')
                                            .count()
                                            + 1;
                                        return Some((node.path.clone(), line_num));
                                    }
                                }
                            }
                        }
                    }
                }
                None
            }
            if let Some((rel_path, line_num)) = search_node(root, &self.tree.root, &patterns) {
                self.jump_to_file_location(&rel_path, line_num, 1);
                self.terminal.log(LogLevel::Success, format!("Instant SIMD jump to definition of `{token}` in {}:{}:1", rel_path, line_num));
                return true;
            }
        }

        self.terminal.log(LogLevel::Info, format!("No definition found for `{token}`"));
        false
    }

    pub fn refresh_active_line_blame(&mut self) {
        if let (Some(root), Some(doc)) = (&self.workspace_root, self.docs.active()) {
            let cursor_line = doc.content.cursor().position.line + 1;
            self.active_line_blame = git::read_blame_for_line(root, &doc.path, cursor_line);
        } else {
            self.active_line_blame = None;
        }
    }

    fn view_merge_conflict_banner(&self) -> Element<'_, Message> {
        let count = self.docs.active().map(|d| d.find_merge_conflicts().len()).unwrap_or(0);
        let banner = row![
            text(format!("⚠️ Merge Conflict ({} conflict{})", count, if count == 1 { "" } else { "s" }))
                .size(11)
                .style(text::warning),
            space().width(Length::Fill),
            button(text("Accept Current").size(10))
                .style(button::primary)
                .padding(Padding { top: 2.0, bottom: 2.0, left: 6.0, right: 6.0 })
                .on_press(Message::AcceptConflictCurrent(0)),
            button(text("Accept Incoming").size(10))
                .style(button::secondary)
                .padding(Padding { top: 2.0, bottom: 2.0, left: 6.0, right: 6.0 })
                .on_press(Message::AcceptConflictIncoming(0)),
            button(text("Accept Both").size(10))
                .style(button::text)
                .padding(Padding { top: 2.0, bottom: 2.0, left: 6.0, right: 6.0 })
                .on_press(Message::AcceptConflictBoth(0)),
        ]
        .spacing(8)
        .padding(Padding { top: 3.0, bottom: 3.0, left: 10.0, right: 10.0 })
        .align_y(Center);

        container(banner)
            .style(container::rounded_box)
            .width(Length::Fill)
            .into()
    }

    fn report_error(&mut self, title: &str, detail: String) {
        self.terminal.log(LogLevel::Error, &detail);
        self.notifications
            .push(NotificationSeverity::Error, title, detail, None, None);
    }

    fn recovery_entries(&self) -> Vec<RecoveryEntry> {
        self.docs
            .documents
            .iter()
            .filter(|doc| doc.is_dirty)
            .map(|doc| RecoveryEntry {
                path: doc.path.clone(),
                name: doc.name.clone(),
                syntax: doc.syntax.clone(),
                text: doc.text(),
                disk_snapshot: doc.disk_snapshot.clone(),
            })
            .collect()
    }

    fn persist_recovery_snapshot(&self) -> std::io::Result<()> {
        if let Some(store) = &self.recovery_store {
            store.persist(self.recovery_entries())?;
        }
        Ok(())
    }

    fn flush_recovery(&mut self) {
        match self.persist_recovery_snapshot() {
            Ok(()) => self.recovery_error_reported = false,
            Err(error) if !self.recovery_error_reported => {
                self.recovery_error_reported = true;
                self.report_error("Recovery backup failed", error.to_string());
            }
            Err(_) => {}
        }
    }

    fn schedule_recovery(&mut self) -> Task<Message> {
        if self.recovery_store.is_none() {
            return Task::none();
        }
        self.recovery_revision = self.recovery_revision.wrapping_add(1);
        let revision = self.recovery_revision;
        Task::perform(
            async move {
                smol::Timer::after(Duration::from_millis(250)).await;
                revision
            },
            Message::PersistRecovery,
        )
    }

    fn restore_recovery(&mut self, index: usize) {
        let Some(candidate) = self
            .recovery_store
            .as_ref()
            .and_then(|store| store.pending().get(index))
            .cloned()
        else {
            return;
        };
        let entry = candidate.entry;
        if self.disk_path(&entry.path).is_none() {
            self.report_error("Restore failed", "Invalid draft path".to_string());
            return;
        }
        if let Some(existing) = self
            .docs
            .documents
            .iter()
            .position(|doc| doc.path == entry.path)
        {
            if self.docs.documents[existing].is_dirty {
                self.report_error(
                    "Restore failed",
                    "This file already has unsaved edits; save or close it first".to_string(),
                );
                return;
            }
        }
        let disk_changed = entry.disk_snapshot.as_ref().is_some_and(|snapshot| {
            self.disk_path(&entry.path)
                .and_then(|path| std::fs::read_to_string(path).ok())
                .as_ref()
                != Some(snapshot)
        });
        let visible_id = self.tree.root.find_by_path(&entry.path).map(|node| node.id);
        let id = visible_id.unwrap_or_else(|| self.docs.reserve_virtual_id());
        let name = Path::new(&entry.path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&entry.name)
            .to_string();
        let syntax = tree::syntax_for_path(Path::new(&entry.path)).to_string();
        let doc_index = if let Some(existing) = self
            .docs
            .documents
            .iter()
            .position(|doc| doc.path == entry.path)
        {
            let doc = &mut self.docs.documents[existing];
            doc.id = id;
            doc.name = name;
            doc.syntax = syntax;
            doc.content = text_editor::Content::with_text(&entry.text);
            doc.disk_snapshot = entry.disk_snapshot;
            doc.is_dirty = true;
            existing
        } else {
            let mut doc = Document::new(id, &entry.path, name, syntax, &entry.text);
            doc.disk_snapshot = entry.disk_snapshot;
            doc.is_dirty = true;
            self.docs.documents.push(doc);
            self.docs.documents.len() - 1
        };
        self.docs.active_tab = doc_index;
        self.docs.focused_pane = 0;
        self.tree.selected_id = visible_id;
        if let Err(error) = self.persist_recovery_snapshot() {
            self.report_error("Recovery backup failed", error.to_string());
            return;
        }
        if let Some(store) = &mut self.recovery_store {
            if let Err(error) = store.dismiss(index) {
                self.report_error("Recovery cleanup failed", error.to_string());
                return;
            }
        }
        if disk_changed {
            self.notifications.push(
                NotificationSeverity::Warning,
                "Disk file changed",
                "Draft restored as unsaved; Save As is available if normal save conflicts",
                None,
                None,
            );
        }
        self.terminal
            .log(LogLevel::Success, format!("Restored draft {}", entry.path));
    }

    fn refresh_git(&mut self) {
        if self.git.repo_root.is_none() {
            if let Some(root) = &self.workspace_root {
                self.git = GitState::for_workspace(root);
            }
        }
        if self.git.repo_root.is_some() {
            if let Err(error) = self.git.refresh_workspace() {
                self.report_error("Git refresh failed", error);
            }
            self.activity.git_changes_badge = self.git.total_changes();
        }
    }

    fn save_active_document(&mut self) {
        let Some(doc) = self.docs.active() else {
            return;
        };
        let path = doc.path.clone();
        let content = doc.text();
        let snapshot = doc.disk_snapshot.clone();
        let Some(snapshot) = snapshot else {
            self.report_error("Save As required", format!("{} has no disk path yet", path));
            return;
        };
        let Some(disk_path) = self.disk_path(&path) else {
            self.report_error("Save failed", "Invalid workspace path".to_string());
            return;
        };
        let result = (|| -> std::io::Result<()> {
            let parent = disk_path
                .parent()
                .and_then(|path| path.canonicalize().ok())
                .ok_or_else(|| std::io::Error::other("file parent is unavailable"))?;
            if !self
                .workspace_root
                .as_ref()
                .is_some_and(|root| parent.starts_with(root))
            {
                return Err(std::io::Error::other(
                    "file parent moved outside workspace; save cancelled",
                ));
            }
            if disk_path.symlink_metadata()?.file_type().is_symlink() {
                return Err(std::io::Error::other(
                    "symbolic-link target changed; save cancelled",
                ));
            }
            if std::fs::read_to_string(&disk_path)? != snapshot {
                return Err(std::io::Error::other(
                    "file changed on disk; save cancelled to protect external edits",
                ));
            }
            write_atomic(&disk_path, content.as_bytes())
        })();
        match result {
            Ok(()) => {
                if let Some(doc) = self.docs.active_mut() {
                    doc.disk_snapshot = Some(content.clone());
                    doc.is_dirty = false;
                }
                self.terminal
                    .log(LogLevel::Success, format!("Saved {}", path));
                if self.language_server_status == lsp::ServerStatus::Ready {
                    if let Some(error) = self
                        .language_server
                        .as_mut()
                        .and_then(|server| server.did_save(&path, &content).err())
                    {
                        self.language_server_status = lsp::ServerStatus::Failed;
                        self.report_error("Language server sync failed", error.to_string());
                    }
                }
                self.flush_recovery();
                self.refresh_git();
            }
            Err(error) => self.report_error("Save failed", format!("{}: {}", path, error)),
        }
    }

    fn refresh_search(&mut self) {
        if let Some(root) = &self.workspace_root {
            if self.search.cached_fingerprint != self.workspace_fingerprint
                || self.search.cached_buffers.is_empty()
            {
                let mut buffers: Vec<(String, Arc<str>)> = Vec::new();
                fn collect(root: &Path, node: &FileNode, buffers: &mut Vec<(String, Arc<str>)>) {
                    if node.is_dir {
                        for child in &node.children {
                            collect(root, child, buffers);
                        }
                    } else {
                        let path = root.join(&node.path);
                        if let Ok(metadata) = path.metadata() {
                            let len = metadata.len();
                            if len > 0 && len <= 100_000_000 {
                                if len <= 1_000_000 {
                                    if let Ok(content) = std::fs::read_to_string(&path) {
                                        buffers.push((node.path.clone(), content.into()));
                                    }
                                } else if let Ok(file) = std::fs::File::open(&path) {
                                    if let Ok(mmap) = unsafe { memmap2::Mmap::map(&file) } {
                                        if let Ok(content) = std::str::from_utf8(&mmap) {
                                            buffers.push((node.path.clone(), content.into()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                collect(root, &self.tree.root, &mut buffers);
                self.search.cached_buffers = Arc::new(buffers);
                self.search.cached_fingerprint = self.workspace_fingerprint;
            }
        }
        let dirty_docs: Vec<(&str, String)> = self
            .docs
            .documents
            .iter()
            .filter(|doc| doc.is_dirty)
            .map(|doc| (doc.path.as_str(), doc.text()))
            .collect();
        let dirty_refs: Vec<(&str, &str)> = dirty_docs
            .iter()
            .map(|(p, c)| (*p, c.as_str()))
            .collect();
        let mut extra_docs = Vec::new();
        for doc in &self.docs.documents {
            if !self.search.cached_buffers.iter().any(|(p, _)| p == &doc.path) {
                extra_docs.push((doc.path.as_str(), doc.text()));
            }
        }
        let extra_refs: Vec<(&str, &str)> = extra_docs
            .iter()
            .map(|(p, c)| (*p, c.as_str()))
            .collect();
        self.search.recompute_search_cached(&dirty_refs, &extra_refs);
    }

    fn checked_workspace_file(&self, relative: &str) -> std::io::Result<PathBuf> {
        let root = self
            .workspace_root
            .as_ref()
            .ok_or_else(|| std::io::Error::other("no workspace is open"))?;
        let path = self
            .disk_path(relative)
            .ok_or_else(|| std::io::Error::other("invalid workspace path"))?;
        let parent = path
            .parent()
            .and_then(|parent| parent.canonicalize().ok())
            .ok_or_else(|| std::io::Error::other("file parent is unavailable"))?;
        if !parent.starts_with(root) {
            return Err(std::io::Error::other("file is outside the workspace"));
        }
        if !path.symlink_metadata()?.file_type().is_file() {
            return Err(std::io::Error::other("path is not a regular file"));
        }
        Ok(path)
    }

    fn preview_workspace_replace(&mut self) {
        if self.workspace_root.is_none() {
            self.report_error("Replace unavailable", "Open a workspace first".to_string());
            return;
        }
        if !self.search.prepare_matcher() {
            self.report_error(
                "Replace unavailable",
                self.search
                    .search_error
                    .clone()
                    .unwrap_or_else(|| "Enter a search pattern".to_string()),
            );
            return;
        }
        let mut paths = std::collections::BTreeSet::new();
        for item in &self.palette.all_items {
            if item.kind == palette::PaletteKind::File {
                paths.insert(item.payload.clone());
            }
        }
        for doc in &self.docs.documents {
            paths.insert(doc.path.clone());
        }
        let mut batch = ReplaceBatch::default();
        let mut retained_bytes = 0usize;
        for path in paths {
            let open = self.docs.documents.iter().find(|doc| doc.path == path);
            let (content, was_open, was_dirty, disk_snapshot) = if let Some(doc) = open {
                (doc.text(), true, doc.is_dirty, doc.disk_snapshot.clone())
            } else {
                let disk_path = match self.checked_workspace_file(&path) {
                    Ok(path) => path,
                    Err(_) => continue,
                };
                if !disk_path
                    .metadata()
                    .is_ok_and(|metadata| metadata.len() <= 1_000_000)
                {
                    continue;
                }
                let Ok(content) = std::fs::read_to_string(disk_path) else {
                    continue;
                };
                (content.clone(), false, false, Some(content))
            };
            let (after, replacements) = match self.search.replace_text(&path, &content) {
                Ok(result) => result,
                Err(error) => {
                    self.report_error("Replace preview failed", error);
                    return;
                }
            };
            if replacements == 0 {
                continue;
            }
            retained_bytes = retained_bytes
                .saturating_add(content.len())
                .saturating_add(after.len());
            if batch.files.len() >= 1_000 || retained_bytes > 16 * 1024 * 1024 {
                self.report_error(
                    "Replace preview too large",
                    "Narrow the search to at most 1,000 files and 16 MB of changes".to_string(),
                );
                return;
            }
            batch.total_replacements += replacements;
            batch.files.push(ReplaceFileChange {
                path,
                before: content,
                after,
                replacements,
                was_open,
                was_dirty,
                disk_snapshot,
            });
        }
        if batch.files.is_empty() {
            self.report_error(
                "Nothing to replace",
                "No matching text was found".to_string(),
            );
            return;
        }
        self.replace_preview = Some(batch);
    }

    fn validate_replace_batch(&self, batch: &ReplaceBatch, undo: bool) -> Result<(), String> {
        for change in &batch.files {
            let expected_buffer = if undo { &change.after } else { &change.before };
            if let Some(doc) = self
                .docs
                .documents
                .iter()
                .find(|doc| doc.path == change.path)
            {
                if &doc.text() != expected_buffer || doc.is_dirty != change.was_dirty {
                    return Err(format!("{} changed after the preview", change.path));
                }
            } else if change.was_open {
                return Err(format!("{} was closed after the preview", change.path));
            }
            if let Some(snapshot) = &change.disk_snapshot {
                let path = self
                    .checked_workspace_file(&change.path)
                    .map_err(|error| format!("{}: {error}", change.path))?;
                let disk = std::fs::read_to_string(path)
                    .map_err(|error| format!("{}: {error}", change.path))?;
                let expected_disk = if change.was_dirty {
                    snapshot
                } else if undo {
                    &change.after
                } else {
                    &change.before
                };
                if &disk != expected_disk {
                    return Err(format!("{} changed on disk", change.path));
                }
            }
        }
        Ok(())
    }

    fn write_replace_batch(&self, batch: &ReplaceBatch, undo: bool) -> Result<(), String> {
        let mut completed: Vec<(&ReplaceFileChange, PathBuf)> = Vec::new();
        for change in &batch.files {
            if change.disk_snapshot.is_none() || change.was_dirty {
                continue;
            }
            let path = self
                .checked_workspace_file(&change.path)
                .map_err(|error| format!("{}: {error}", change.path))?;
            let expected = if undo { &change.after } else { &change.before };
            let current = std::fs::read_to_string(&path)
                .map_err(|error| format!("{}: {error}", change.path))?;
            if &current != expected {
                for (previous, previous_path) in completed.into_iter().rev() {
                    let rollback = if undo {
                        &previous.after
                    } else {
                        &previous.before
                    };
                    let _ = write_atomic(&previous_path, rollback.as_bytes());
                }
                return Err(format!("{} changed on disk", change.path));
            }
            let replacement = if undo { &change.before } else { &change.after };
            if let Err(error) = write_atomic(&path, replacement.as_bytes()) {
                for (previous, previous_path) in completed.into_iter().rev() {
                    let rollback = if undo {
                        &previous.after
                    } else {
                        &previous.before
                    };
                    let _ = write_atomic(&previous_path, rollback.as_bytes());
                }
                return Err(format!("{}: {error}", change.path));
            }
            completed.push((change, path));
        }
        Ok(())
    }

    fn apply_replace_batch(&mut self, undo: bool) {
        let batch = if undo {
            self.replace_undo.clone()
        } else {
            self.replace_preview.clone()
        };
        let Some(batch) = batch else {
            return;
        };
        if let Err(error) = self.validate_replace_batch(&batch, undo) {
            self.report_error(
                if undo {
                    "Undo replace cancelled"
                } else {
                    "Replace cancelled"
                },
                error,
            );
            return;
        }
        if let Err(error) = self.write_replace_batch(&batch, undo) {
            self.report_error(
                if undo {
                    "Undo replace failed"
                } else {
                    "Replace failed"
                },
                error,
            );
            return;
        }
        for change in &batch.files {
            if let Some(doc) = self
                .docs
                .documents
                .iter_mut()
                .find(|doc| doc.path == change.path)
            {
                let text = if undo { &change.before } else { &change.after };
                doc.content = text_editor::Content::with_text(text);
                doc.is_dirty = change.was_dirty;
                if !change.was_dirty {
                    doc.disk_snapshot = Some(text.clone());
                }
            }
        }
        if undo {
            self.replace_undo = None;
        } else {
            self.replace_undo = Some(batch.clone());
            self.replace_preview = None;
        }
        self.flush_recovery();
        self.refresh_search();
        self.refresh_git();
        let action = if undo { "Undid" } else { "Replaced" };
        self.terminal.log(
            LogLevel::Success,
            format!(
                "{action} {} occurrence(s) across {} file(s)",
                batch.total_replacements,
                batch.files.len()
            ),
        );
    }

    fn schedule_search(&mut self, background: bool) -> Task<Message> {
        let revision = self.search_revision.fetch_add(1, Ordering::Relaxed) + 1;
        if self.search.query.trim().is_empty() {
            self.search.results.clear();
            self.search.total_matches = 0;
            self.search.is_searching = false;
            self.search.search_error = None;
            return Task::none();
        }
        if !self.search.prepare_matcher() {
            self.search.results.clear();
            self.search.total_matches = 0;
            self.search.is_searching = false;
            return Task::none();
        }
        let Some(root) = self.workspace_root.clone() else {
            self.refresh_search();
            return Task::none();
        };
        let file_count = self
            .palette
            .all_items
            .iter()
            .filter(|item| item.kind == palette::PaletteKind::File)
            .count();
        // Small workspaces fit comfortably inside a single input event.
        if file_count <= 64 && !background {
            self.refresh_search();
            return Task::none();
        }
        let paths: Vec<String> = self
            .palette
            .all_items
            .iter()
            .filter(|item| item.kind == palette::PaletteKind::File)
            .map(|item| item.payload.clone())
            .collect();
        let dirty: Vec<(String, String)> = self
            .docs
            .documents
            .iter()
            .filter(|doc| doc.is_dirty)
            .map(|doc| (doc.path.clone(), doc.text()))
            .collect();
        let mut state = self.search.clone();
        state.results.clear();
        state.total_matches = 0;
        self.search.is_searching = true;
        self.search.results.clear();
        self.search.total_matches = 0;
        let current = Arc::clone(&self.search_revision);
        Task::perform(
            async move {
                smol::Timer::after(Duration::from_millis(120)).await;
                if current.load(Ordering::Relaxed) != revision {
                    return None;
                }
                smol::unblock(move || {
                    for relative in paths {
                        if current.load(Ordering::Relaxed) != revision {
                            return None;
                        }
                        if let Some((_, content)) = dirty.iter().find(|(path, _)| path == &relative)
                        {
                            state.append_file(&relative, content);
                            continue;
                        }
                        let path = root.join(&relative);
                        if let Ok(metadata) = path.metadata() {
                            let file_size = metadata.len();
                            if file_size > 0 && file_size <= 100_000_000 {
                                if file_size <= 1_000_000 {
                                    if let Ok(content) = std::fs::read_to_string(&path) {
                                        state.append_file(&relative, &content);
                                    }
                                } else if let Ok(file) = std::fs::File::open(&path) {
                                    if let Ok(mmap) = unsafe { memmap2::Mmap::map(&file) } {
                                        if let Ok(content) = std::str::from_utf8(&mmap) {
                                            state.append_file(&relative, content);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    state.is_searching = false;
                    Some((revision, state))
                })
                .await
            },
            Message::SearchFinished,
        )
    }

    fn cargo_task(&mut self, verb: &'static str) -> Task<Message> {
        let Some(root) = self.workspace_root.clone() else {
            return Task::none();
        };
        if !root.join("Cargo.toml").exists() {
            self.report_error(
                "Task unavailable",
                "This workspace has no Cargo.toml".to_string(),
            );
            return Task::none();
        }
        self.terminal.active_tab = PanelTab::BuildConsole;
        self.terminal
            .log_build(LogLevel::Info, format!("$ cargo {}", verb));
        Task::perform(
            async move {
                match std::process::Command::new("cargo")
                    .arg(verb)
                    .current_dir(root)
                    .output()
                {
                    Ok(output) => {
                        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
                        text.push_str(&String::from_utf8_lossy(&output.stderr));
                        (verb, output.status.success(), text)
                    }
                    Err(error) => (verb, false, error.to_string()),
                }
            },
            |(verb, success, output)| Message::WorkspaceCommandFinished(verb, success, output),
        )
    }

    /// Resolves the current `taim::Theme` from active mode.
    pub fn theme(&self) -> Theme {
        match self.theme_mode {
            ThemeMode::Dark if self.workspace_root.is_some() => Theme::custom(
                "Virgo Graphite",
                taim::theme::palette::Seed {
                    background: Color::from_packed_rgba8(0x121212, 1.0),
                    text: Color::from_packed_rgba8(0xE4E4E4, 1.0),
                    primary: Color::from_packed_rgba8(0x505050, 1.0),
                    success: Color::from_packed_rgba8(0x595959, 1.0),
                    warning: Color::from_packed_rgba8(0x666666, 1.0),
                    danger: Color::from_packed_rgba8(0x606060, 1.0),
                },
            ),
            ThemeMode::Dark => Theme::Dark,
            ThemeMode::Light => Theme::Light,
            ThemeMode::HighContrast => Theme::HighContrast,
        }
    }

    /// Resolves highlighter theme.
    pub fn highlighter_theme(&self) -> taim_highlighter::Theme {
        match self.theme_mode {
            ThemeMode::Dark => taim_highlighter::Theme::Dark,
            ThemeMode::Light => taim_highlighter::Theme::Light,
            ThemeMode::HighContrast => taim_highlighter::Theme::HighContrast,
        }
    }

    /// Routes workbench shortcuts using the active input context.
    pub fn subscription(&self) -> Subscription<Message> {
        let context = ShortcutContext {
            terminal_focused: self.terminal_raw_focus
                && self.show_terminal
                && self.terminal.active_tab == PanelTab::Terminal,
            palette_open: self.palette.is_open,
            palette_selection: self.palette.selected_item().map(|item| item.id),
            find_open: self.docs.find_open,
            autocomplete_open: self.autocomplete.is_open,
            autocomplete_selection: self.autocomplete.selected_index,
        };
        let keyboard = taim::keyboard::listen()
            .with(context)
            .filter_map(|(context, event)| {
                if let taim::keyboard::Event::KeyPressed { key, modifiers, .. } = event {
                    shortcut_message(key.as_ref(), modifiers, context)
                } else {
                    None
                }
            });
        if self.workspace_root.is_some() {
            Subscription::batch([
                keyboard,
                taim::time::every(Duration::from_secs(2)).map(|_| Message::WorkspacePoll),
            ])
        } else {
            keyboard
        }
    }

    /// Updates Studio state in response to messages.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ThemeSelected(mode) => {
                self.theme_mode = mode;
                self.terminal
                    .log(LogLevel::Info, format!("Switched theme to {:?}", mode));
                Task::none()
            }
            Message::ToggleSidebar => {
                self.show_sidebar = !self.show_sidebar;
                Task::none()
            }
            Message::SelectSidebarView(view) => {
                if view == SidebarView::SourceControl && self.workspace_root.is_some() {
                    self.refresh_git();
                }
                let toggled_same = self.activity.select(view);
                if toggled_same {
                    self.show_sidebar = !self.show_sidebar;
                } else {
                    self.show_sidebar = true;
                }
                if view == SidebarView::Search
                    && self.show_sidebar
                    && self.workspace_root.is_some()
                    && !self.search.query.is_empty()
                {
                    self.schedule_search(true)
                } else {
                    Task::none()
                }
            }
            Message::RevealSidebarView(view) => {
                self.terminal_raw_focus = false;
                let reveal = if self.show_sidebar && self.activity.active_view == view {
                    Task::none()
                } else {
                    self.update(Message::SelectSidebarView(view))
                };
                let focus = match view {
                    SidebarView::Search => Some("virgo-workspace-search"),
                    SidebarView::Settings => Some("virgo-settings-search"),
                    _ => None,
                };
                if let Some(id) = focus {
                    Task::batch([reveal, taim::widget::operation::focus(id)])
                } else {
                    reveal
                }
            }
            Message::ToggleTerminal => {
                self.show_terminal = !self.show_terminal;
                if !self.show_terminal {
                    self.terminal_raw_focus = false;
                }
                Task::none()
            }
            Message::ToggleSplitEditor => {
                self.docs.toggle_split();
                let is_split = self.docs.split_tab.is_some();
                self.terminal.log(
                    LogLevel::Info,
                    if is_split {
                        "Dual split-editor enabled"
                    } else {
                        "Split-editor closed"
                    },
                );
                Task::none()
            }
            Message::FocusPane(pane) => {
                self.docs.focused_pane = pane;
                Task::none()
            }
            Message::ToggleFind => {
                self.terminal_raw_focus = false;
                self.docs.toggle_find();
                if self.docs.find_open {
                    taim::widget::operation::focus("virgo-editor-find")
                } else {
                    Task::none()
                }
            }
            Message::RevealFind => {
                self.terminal_raw_focus = false;
                if !self.docs.find_open {
                    self.docs.toggle_find();
                }
                taim::widget::operation::focus("virgo-editor-find")
            }
            Message::CloseFind => {
                self.docs.find_open = false;
                Task::none()
            }
            Message::FindQueryChanged(query) => {
                self.docs.set_find_query(query);
                Task::none()
            }
            Message::FindNext => {
                self.docs.find_next();
                Task::none()
            }
            Message::FindPrev => {
                self.docs.find_prev();
                Task::none()
            }
            Message::OpenCommandPalette(action_mode) => {
                self.terminal_raw_focus = false;
                self.palette.open(action_mode);
                taim::widget::operation::focus("virgo-command-palette")
            }
            Message::CloseCommandPalette => {
                self.palette.close();
                Task::none()
            }
            Message::PaletteQueryChanged(q) => {
                self.palette.set_query(q);
                Task::none()
            }
            Message::PaletteSelectNext => {
                self.palette.select_next();
                Task::none()
            }
            Message::PaletteSelectPrev => {
                self.palette.select_prev();
                Task::none()
            }
            Message::PaletteItemChosen(id) => {
                if let Some(item) = self.palette.all_items.iter().find(|i| i.id == id).cloned() {
                    self.palette.close();
                    match item.kind {
                        palette::PaletteKind::File => {
                            if let Some(node) = self.tree.root.find_by_path(&item.payload) {
                                let (node_id, node_path, node_name, node_syntax) =
                                    (node.id, node.path.clone(), node.name.clone(), node.syntax);
                                if self.workspace_root.is_some() {
                                    self.open_file(node_id);
                                } else {
                                    self.tree.selected_id = Some(node_id);
                                    self.docs.open_or_focus(
                                        node_id,
                                        &node_path,
                                        &node_name,
                                        node_syntax,
                                    );
                                }
                                self.terminal
                                    .log(LogLevel::Info, format!("Quick opened {}", node_path));
                            }
                        }
                        palette::PaletteKind::Action => match item.payload.as_str() {
                            "action:run" => return self.update(Message::TriggerRun),
                            "action:build" => return self.update(Message::TriggerBuild),
                            "action:split" => return self.update(Message::ToggleSplitEditor),
                            "action:toggle_sidebar" => return self.update(Message::ToggleSidebar),
                            "action:toggle_terminal" => {
                                return self.update(Message::ToggleTerminal);
                            }
                            "action:theme_dark" => {
                                return self.update(Message::ThemeSelected(ThemeMode::Dark));
                            }
                            "action:theme_high_contrast" => {
                                return self
                                    .update(Message::ThemeSelected(ThemeMode::HighContrast));
                            }
                            "action:new_scratch" => return self.update(Message::NewScratchpad),
                            "action:new_file" => return self.update(Message::OpenNewFile),
                            "action:view_explorer" => {
                                return self
                                    .update(Message::SelectSidebarView(SidebarView::Explorer));
                            }
                            "action:view_search" => {
                                return self
                                    .update(Message::SelectSidebarView(SidebarView::Search));
                            }
                            "action:view_git" => {
                                return self.update(Message::SelectSidebarView(
                                    SidebarView::SourceControl,
                                ));
                            }
                            "action:view_debug" => {
                                return self.update(Message::SelectSidebarView(SidebarView::Debug));
                            }
                            "action:view_extensions" => {
                                return self
                                    .update(Message::SelectSidebarView(SidebarView::Extensions));
                            }
                            "action:view_settings" => {
                                return self
                                    .update(Message::SelectSidebarView(SidebarView::Settings));
                            }
                            "action:git_stage_all" => return self.update(Message::GitStageAll),
                            "action:git_diff" => {
                                return self
                                    .update(Message::GitToggleDiff("src/lib.rs".to_string()));
                            }
                            "action:debug_start" => return self.update(Message::DebugStart),
                            "action:term_new" => return self.update(Message::NewTerminalTab),
                            "action:goto_definition" => return self.update(Message::GoToDefinition),
                            "action:find_references" => return self.update(Message::FindReferences),
                            "action:show_hover" => return self.update(Message::ShowHover),
                            "action:accept_all_current" => {
                                return self.update(Message::AcceptAllConflicts(document::ConflictChoice::AcceptCurrent));
                            }
                            "action:accept_all_incoming" => {
                                return self.update(Message::AcceptAllConflicts(document::ConflictChoice::AcceptIncoming));
                            }
                            "action:fold_all" => return self.update(Message::FoldAll),
                            "action:unfold_all" => return self.update(Message::UnfoldAll),
                            _ => {}
                        },
                    }
                }
                Task::none()
            }
            Message::ZoomIn => {
                self.font_size = (self.font_size + 1.0).min(24.0);
                Task::none()
            }
            Message::ZoomOut => {
                self.font_size = (self.font_size - 1.0).max(10.0);
                Task::none()
            }
            Message::ZoomReset => {
                self.font_size = 13.0;
                Task::none()
            }
            Message::TreeNodeToggled(id) => {
                self.tree.toggle(id);
                Task::none()
            }
            Message::FileSelected(id) => {
                self.terminal_raw_focus = false;
                self.open_file(id);
                Task::none()
            }
            Message::TabSelected(index) => {
                self.docs.select_tab(index);
                self.docs.focused_pane = 0;
                if let Some(doc) = self.docs.active() {
                    self.tree.selected_id = Some(doc.id);
                }
                Task::none()
            }
            Message::SplitTabSelected(index) => {
                self.docs.split_tab = Some(index);
                self.docs.focused_pane = 1;
                Task::none()
            }
            Message::TabClosed(index) => {
                if self.workspace_root.is_some()
                    && self
                        .docs
                        .documents
                        .get(index)
                        .is_some_and(|doc| doc.is_dirty)
                {
                    self.report_error(
                        "Unsaved changes",
                        "Save this document before closing its tab".to_string(),
                    );
                    return Task::none();
                }
                let closed_path = self.docs.documents.get(index).map(|doc| doc.path.clone());
                if self.workspace_root.is_some() && self.docs.documents.len() == 1 && index == 0 {
                    self.docs.documents.clear();
                    self.docs.active_tab = 0;
                    self.docs.split_tab = None;
                    self.docs.focused_pane = 0;
                } else {
                    self.docs.close_tab(index);
                }
                if let (Some(server), Some(path)) = (&mut self.language_server, closed_path) {
                    let _ = server.did_close(&path);
                }
                self.tree.selected_id = self.docs.active().map(|doc| doc.id);
                Task::none()
            }
            Message::CloseActiveTab => {
                if self.docs.documents.is_empty() {
                    Task::none()
                } else {
                    let index = if self.docs.focused_pane == 1 {
                        self.docs.split_tab.unwrap_or(self.docs.active_tab)
                    } else {
                        self.docs.active_tab
                    };
                    self.update(Message::TabClosed(index))
                }
            }
            Message::CycleEditor(direction) => {
                let count = self.docs.documents.len();
                if count > 1 {
                    let next =
                        (self.docs.active_tab as i32 + direction).rem_euclid(count as i32) as usize;
                    self.update(Message::TabSelected(next))
                } else {
                    Task::none()
                }
            }
            Message::NewScratchpad => {
                self.docs.new_scratchpad();
                self.terminal
                    .log(LogLevel::Info, "Created new scratchpad buffer");
                Task::none()
            }
            Message::OpenNewFile => {
                if self.workspace_root.is_some() {
                    self.creating_file = true;
                    self.activity.active_view = SidebarView::Explorer;
                    self.show_sidebar = true;
                } else {
                    return self.update(Message::NewScratchpad);
                }
                taim::widget::operation::focus("virgo-new-file")
            }
            Message::NewFilePathChanged(path) => {
                self.new_file_path = path;
                Task::none()
            }
            Message::CreateFile => {
                self.create_workspace_file();
                Task::none()
            }
            Message::OpenRenameSelected => {
                let Some(node) = self
                    .tree
                    .selected_id
                    .and_then(|id| self.tree.find(id))
                    .filter(|node| !node.path.is_empty())
                else {
                    self.report_error(
                        "Rename unavailable",
                        "Select a file or folder first".to_string(),
                    );
                    return Task::none();
                };
                self.rename_name = node.name.clone();
                self.renaming_file = true;
                self.deleting_path = None;
                self.activity.active_view = SidebarView::Explorer;
                self.show_sidebar = true;
                taim::widget::operation::focus("virgo-rename-file")
            }
            Message::RenameNameChanged(name) => {
                self.rename_name = name;
                Task::none()
            }
            Message::ConfirmRename => {
                self.rename_selected_file();
                Task::none()
            }
            Message::CancelRename => {
                self.renaming_file = false;
                self.rename_name.clear();
                Task::none()
            }
            Message::OpenDeleteSelected => {
                let Some(node) = self
                    .tree
                    .selected_id
                    .and_then(|id| self.tree.find(id))
                    .filter(|node| !node.path.is_empty())
                else {
                    self.report_error(
                        "Delete unavailable",
                        "Select a file or folder first".to_string(),
                    );
                    return Task::none();
                };
                self.deleting_path = Some(node.path.clone());
                self.renaming_file = false;
                self.rename_name.clear();
                self.activity.active_view = SidebarView::Explorer;
                self.show_sidebar = true;
                Task::none()
            }
            Message::ConfirmDelete => {
                self.delete_selected_entry();
                Task::none()
            }
            Message::CancelDelete => {
                self.deleting_path = None;
                Task::none()
            }
            Message::RestoreTrash(index) => {
                self.restore_trash_entry(index);
                Task::none()
            }
            Message::RefreshWorkspace => {
                self.reconcile_workspace(true);
                Task::none()
            }
            Message::WorkspacePoll => {
                if let Some(root) = &self.workspace_root {
                    let fingerprint = workspace_fingerprint(root, &self.tree);
                    if fingerprint != self.workspace_fingerprint {
                        self.reconcile_workspace(false);
                    }
                }
                Task::none()
            }
            Message::EditorAction(action) => {
                let edited = matches!(action, text_editor::Action::Edit(_));
                let index = self.docs.active_tab;
                if let Some(doc) = self.docs.documents.get_mut(index) {
                    doc.perform(action);
                }
                if edited {
                    self.sync_document_change_to_language_server(index);
                }
                let search = if edited
                    && self.workspace_root.is_some()
                    && self.show_sidebar
                    && self.activity.active_view == SidebarView::Search
                {
                    self.schedule_search(true)
                } else {
                    Task::none()
                };
                Task::batch([
                    search,
                    if edited {
                        self.schedule_recovery()
                    } else {
                        Task::none()
                    },
                ])
            }
            Message::SplitEditorAction(action) => {
                let edited = matches!(action, text_editor::Action::Edit(_));
                let index = self.docs.split_tab;
                if let Some(doc) = self.docs.split_doc_mut() {
                    doc.perform(action);
                }
                if edited {
                    if let Some(index) = index {
                        self.sync_document_change_to_language_server(index);
                    }
                }
                let search = if edited
                    && self.workspace_root.is_some()
                    && self.show_sidebar
                    && self.activity.active_view == SidebarView::Search
                {
                    self.schedule_search(true)
                } else {
                    Task::none()
                };
                Task::batch([
                    search,
                    if edited {
                        self.schedule_recovery()
                    } else {
                        Task::none()
                    },
                ])
            }
            Message::EditorInteraction(pane_id, action) => {
                self.terminal_raw_focus = false;
                self.update(if pane_id == 0 {
                    Message::EditorAction(action)
                } else {
                    Message::SplitEditorAction(action)
                })
            }
            Message::SelectNextOccurrence => {
                let index = if self.docs.focused_pane == 1 && self.docs.split_tab.is_some() {
                    self.docs.split_tab.unwrap()
                } else {
                    self.docs.active_tab
                };
                if let Some(doc) = self.docs.documents.get_mut(index) {
                    doc.select_next_occurrence();
                }
                Task::none()
            }
            Message::ClearExtraCursors => {
                let index = if self.docs.focused_pane == 1 && self.docs.split_tab.is_some() {
                    self.docs.split_tab.unwrap()
                } else {
                    self.docs.active_tab
                };
                if let Some(doc) = self.docs.documents.get_mut(index) {
                    doc.clear_extra_cursors();
                }
                Task::none()
            }
            Message::EditorGoToLine(doc_idx, line) => {
                if let Some(doc) = self.docs.documents.get_mut(doc_idx) {
                    doc.content
                        .move_to(text_editor::Cursor::new(text_editor::Position::new(
                            line.min(doc.line_count().saturating_sub(1)),
                            0,
                        )));
                    self.docs.focused_pane = usize::from(self.docs.split_tab == Some(doc_idx));
                }
                Task::none()
            }
            Message::EditorScrollToLine(doc_idx, target) => {
                if let Some(doc) = self.docs.documents.get_mut(doc_idx) {
                    let (first, _) = doc.content.scroll_position();
                    let delta = (target as i64 - first as i64)
                        .clamp(i32::MIN as i64, i32::MAX as i64)
                        as i32;
                    doc.perform(text_editor::Action::Scroll { lines: delta });
                }
                Task::none()
            }
            Message::SaveDocument => {
                if self.workspace_root.is_some() {
                    if self
                        .docs
                        .active()
                        .is_some_and(|doc| doc.disk_snapshot.is_none())
                    {
                        return self.update(Message::OpenSaveAs);
                    }
                    self.save_active_document();
                    return Task::none();
                }
                if let Some(doc) = self.docs.active() {
                    let path = doc.path.clone();
                    let content = doc.text();
                    if let Some(disk_path) = self.disk_path(&path) {
                        match std::fs::write(&disk_path, content) {
                            Ok(()) => {
                                if let Some(doc) = self.docs.active_mut() {
                                    doc.is_dirty = false;
                                }
                                self.terminal
                                    .log(LogLevel::Success, format!("Saved {}", path));
                            }
                            Err(error) => {
                                self.report_error("Save failed", format!("{}: {}", path, error))
                            }
                        }
                    } else if self.workspace_root.is_some() {
                        self.report_error(
                            "Save failed",
                            "Scratchpads need Save As before writing to disk".to_string(),
                        );
                    } else if let Some(doc) = self.docs.active_mut() {
                        doc.is_dirty = false;
                    }
                }
                Task::none()
            }
            Message::OpenSaveAs => {
                if self.workspace_root.is_none() || self.docs.active().is_none() {
                    self.report_error(
                        "Save As unavailable",
                        "Open a document in a workspace".to_string(),
                    );
                    return Task::none();
                }
                self.saving_as = true;
                self.save_as_path.clear();
                self.activity.active_view = SidebarView::Explorer;
                self.show_sidebar = true;
                taim::widget::operation::focus("virgo-save-as")
            }
            Message::SaveAsPathChanged(path) => {
                self.save_as_path = path;
                Task::none()
            }
            Message::ConfirmSaveAs => {
                self.save_as_workspace_file();
                Task::none()
            }
            Message::CancelSaveAs => {
                self.saving_as = false;
                self.save_as_path.clear();
                Task::none()
            }
            Message::ApplyDiagnosticFix(idx) => {
                if self.workspace_root.is_some() {
                    self.report_error(
                        "Quick fix unavailable",
                        "No language server is connected".to_string(),
                    );
                    return Task::none();
                }
                let target_file = self.terminal.diagnostics.get(idx).map(|d| d.file.clone());
                let is_current_file =
                    target_file.as_deref() == self.docs.active().map(|d| d.path.as_str());

                if is_current_file && idx < self.terminal.diagnostics.len() {
                    self.terminal.diagnostics.remove(idx);
                    self.activity.diagnostics_badge = self.terminal.diagnostics.len();
                    self.terminal.log(
                        LogLevel::Success,
                        "Applied quick fix: prefixed unused variable with `_`",
                    );
                    self.notifications.push(
                        NotificationSeverity::Success,
                        "Quick Fix Applied",
                        "Prefixed `raster_time_us` with `_`",
                        None,
                        None,
                    );
                }
                Task::none()
            }
            Message::TerminalTabSelected(tab) => {
                if self.workspace_root.is_some() && tab == PanelTab::Benchmark {
                    return Task::none();
                }
                self.terminal.active_tab = tab;
                if tab != PanelTab::Terminal {
                    self.terminal_raw_focus = false;
                }
                if self.workspace_root.is_some()
                    && tab == PanelTab::Terminal
                    && self.terminal.terminals.is_empty()
                {
                    return self.update(Message::NewTerminalTab);
                }
                Task::none()
            }
            Message::ClearTerminal => {
                if self.workspace_root.is_some() {
                    if self.terminal.active_tab == PanelTab::Terminal {
                        if let Some(instance) = self
                            .terminal
                            .terminals
                            .get_mut(self.terminal.active_terminal_idx)
                        {
                            instance.lines.clear();
                            if let Some(session) = self.terminal_sessions.get_mut(&instance.id) {
                                session.clear_output();
                            }
                        }
                    } else {
                        self.terminal.build_logs.clear();
                    }
                    return Task::none();
                }
                self.terminal.clear();
                Task::none()
            }
            Message::TriggerCheck => {
                if self.workspace_root.is_some() {
                    return self.cargo_task("check");
                }
                self.terminal.active_tab = PanelTab::BuildConsole;
                self.terminal
                    .log_build(LogLevel::Info, "$ cargo check --offline --workspace");
                self.terminal.log_build(
                    LogLevel::Success,
                    "   Finished `dev` profile [unoptimized + debuginfo] in 0.08s",
                );
                self.terminal.log_build(
                    LogLevel::Info,
                    "   Check passed: 0 errors, 1 warning (unused variable)",
                );
                Task::none()
            }
            Message::TriggerBuild => {
                if self.workspace_root.is_some() {
                    return self.cargo_task("build");
                }
                self.terminal.active_tab = PanelTab::BuildConsole;
                self.terminal
                    .log_build(LogLevel::Info, "$ cargo build --offline --release");
                self.build_phase = 1;
                Task::perform(async { 1 }, Message::BuildStep)
            }
            Message::TriggerRun => {
                if let Some(root) = self.workspace_root.as_ref() {
                    if !root.join("Cargo.toml").is_file() {
                        self.report_error(
                            "Run unavailable",
                            "This workspace has no Cargo.toml; run configurations are not implemented yet"
                                .to_string(),
                        );
                        return Task::none();
                    }
                    let task = self.update(Message::NewTerminalTab);
                    if let Some(instance) = self
                        .terminal
                        .terminals
                        .get(self.terminal.active_terminal_idx)
                    {
                        if let Some(session) = self.terminal_sessions.get_mut(&instance.id) {
                            if let Err(error) = session.write(b"cargo run\r") {
                                self.report_error("Run failed", error.to_string());
                            }
                        }
                    }
                    return task;
                }
                self.terminal.active_tab = PanelTab::Terminal;
                self.terminal
                    .log(LogLevel::Info, "$ cargo run --offline --bin taim-app");
                self.terminal
                    .log(LogLevel::Success, "   Compiling taim v0.15.0-dev...");
                self.terminal
                    .log(LogLevel::Success, "   Running `target/release/taim-app`");
                self.terminal.log(
                    LogLevel::Info,
                    "   [taim-runtime] Render window presented (120 FPS Metal/Vulkan)",
                );
                self.notifications.push(
                    NotificationSeverity::Info,
                    "Application Running",
                    "Target presented at 120 FPS native.",
                    None,
                    None,
                );
                Task::none()
            }
            Message::BuildStep(phase) => match phase {
                1 => {
                    self.terminal
                        .log_build(LogLevel::Info, "   Compiling taim-core v0.15.0-dev...");
                    self.terminal.log_build(
                        LogLevel::Info,
                        "   Compiling taim-raster v0.15.0-dev (compute-raster)...",
                    );
                    self.build_phase = 2;
                    Task::perform(async { 2 }, Message::BuildStep)
                }
                2 => {
                    self.terminal
                        .log_build(LogLevel::Info, "   Compiling taim-ide v0.1.0...");
                    self.terminal.log_build(
                        LogLevel::Success,
                        "   Finished `release` profile [optimized] in 0.38s",
                    );
                    self.terminal.log_build(
                        LogLevel::Success,
                        "   Binary stripped size: 2.1 MiB (Pillar XIII verified)",
                    );
                    self.build_phase = 0;
                    self.notifications.push(
                        NotificationSeverity::Success,
                        "Build Finished",
                        "Release binary stripped to 2.1 MiB in 0.38s.",
                        Some("Run Target"),
                        Some("run"),
                    );
                    Task::none()
                }
                _ => Task::none(),
            },
            Message::WorkspaceCommandFinished(verb, success, output) => {
                for line in output.lines().take(500) {
                    self.terminal.log_build(
                        if success {
                            LogLevel::Info
                        } else {
                            LogLevel::Error
                        },
                        line,
                    );
                }
                let result = if success { "succeeded" } else { "failed" };
                self.terminal.log_build(
                    if success {
                        LogLevel::Success
                    } else {
                        LogLevel::Error
                    },
                    format!("cargo {} {}", verb, result),
                );
                Task::none()
            }

            // --- Global Workspace Search ---
            Message::GlobalSearchQueryChanged(q) => {
                self.search.query = q;
                self.replace_preview = None;
                self.schedule_search(false)
            }
            Message::SearchFinished(result) => {
                if let Some((revision, completed)) = result {
                    if self.search_revision.load(Ordering::Relaxed) == revision {
                        self.search.results = completed.results;
                        self.search.total_matches = completed.total_matches;
                        self.search.is_searching = false;
                    }
                }
                Task::none()
            }
            Message::GlobalSearchReplaceQueryChanged(r) => {
                self.search.replace_query = r;
                self.replace_preview = None;
                Task::none()
            }
            Message::SearchHitSelected(path, line, column) => {
                if let Some(node) = self.tree.root.find_by_path(&path) {
                    let id = node.id;
                    self.open_file(id);
                    if let Some(doc) = self.docs.active_mut().filter(|d| d.path == path) {
                        let line_index = line
                            .saturating_sub(1)
                            .min(doc.line_count().saturating_sub(1));
                        let byte_column = doc
                            .content
                            .line(line_index)
                            .map(|line| {
                                line.text
                                    .char_indices()
                                    .nth(column.saturating_sub(1))
                                    .map(|(byte, _)| byte)
                                    .unwrap_or(line.text.len())
                            })
                            .unwrap_or(0);
                        doc.content
                            .move_to(text_editor::Cursor::new(text_editor::Position::new(
                                line_index,
                                byte_column,
                            )));
                    }
                }
                Task::none()
            }
            Message::ToggleSearchCase => {
                self.search.match_case = !self.search.match_case;
                self.replace_preview = None;
                self.schedule_search(false)
            }
            Message::ToggleSearchWord => {
                self.search.whole_word = !self.search.whole_word;
                self.replace_preview = None;
                self.schedule_search(false)
            }
            Message::ToggleSearchRegex => {
                self.search.use_regex = !self.search.use_regex;
                self.replace_preview = None;
                self.schedule_search(false)
            }
            Message::SearchIncludeChanged(value) => {
                self.search.files_to_include = value;
                self.replace_preview = None;
                self.schedule_search(false)
            }
            Message::SearchExcludeChanged(value) => {
                self.search.files_to_exclude = value;
                self.replace_preview = None;
                self.schedule_search(false)
            }
            Message::ToggleSearchReplace => {
                self.search.toggle_replace();
                Task::none()
            }
            Message::OpenSearchReplace => {
                self.search.show_replace = true;
                self.update(Message::RevealSidebarView(SidebarView::Search))
            }
            Message::PreviewWorkspaceReplace => {
                self.preview_workspace_replace();
                Task::none()
            }
            Message::ConfirmWorkspaceReplace => {
                self.apply_replace_batch(false);
                Task::none()
            }
            Message::CancelWorkspaceReplace => {
                self.replace_preview = None;
                Task::none()
            }
            Message::UndoWorkspaceReplace => {
                self.apply_replace_batch(true);
                Task::none()
            }
            Message::ToggleSearchGroup(path) => {
                self.search.toggle_group_expanded(&path);
                Task::none()
            }

            // --- Source Control (Git) ---
            Message::GitRefresh => {
                self.refresh_git();
                Task::none()
            }
            Message::GitStageFile(path) => {
                if self.workspace_root.is_some() {
                    if let Err(error) = self.git.stage_workspace_file(&path) {
                        self.report_error("Git stage failed", error);
                    }
                    self.activity.git_changes_badge = self.git.total_changes();
                    return Task::none();
                }
                self.git.stage_file(&path);
                self.activity.git_changes_badge = self.git.total_changes();
                Task::none()
            }
            Message::GitUnstageFile(path) => {
                if self.workspace_root.is_some() {
                    if let Err(error) = self.git.unstage_workspace_file(&path) {
                        self.report_error("Git unstage failed", error);
                    }
                    self.activity.git_changes_badge = self.git.total_changes();
                    return Task::none();
                }
                self.git.unstage_file(&path);
                self.activity.git_changes_badge = self.git.total_changes();
                Task::none()
            }
            Message::GitStageAll => {
                if self.workspace_root.is_some() {
                    if let Err(error) = self.git.stage_workspace_all() {
                        self.report_error("Git stage failed", error);
                    }
                    self.activity.git_changes_badge = self.git.total_changes();
                    return Task::none();
                }
                self.git.stage_all();
                Task::none()
            }
            Message::GitUnstageAll => {
                if self.workspace_root.is_some() {
                    if let Err(error) = self.git.unstage_workspace_all() {
                        self.report_error("Git unstage failed", error);
                    }
                    self.activity.git_changes_badge = self.git.total_changes();
                    return Task::none();
                }
                self.git.unstage_all();
                Task::none()
            }
            Message::GitCommitMessageChanged(msg) => {
                self.git.commit_message = msg;
                Task::none()
            }
            Message::GitCommit => {
                if self.workspace_root.is_some() {
                    match self.git.commit_workspace() {
                        Ok(message) => self.terminal.log(LogLevel::Success, message),
                        Err(error) => self.report_error("Git commit failed", error),
                    }
                    self.activity.git_changes_badge = self.git.total_changes();
                    return Task::none();
                }
                match self.git.commit() {
                    Ok(success_msg) => {
                        self.activity.git_changes_badge = self.git.total_changes();
                        self.terminal.log(LogLevel::Success, &success_msg);
                        self.notifications.push(
                            NotificationSeverity::Success,
                            "Git Committed",
                            success_msg,
                            None,
                            None,
                        );
                    }
                    Err(err) => {
                        self.terminal.log(LogLevel::Error, err);
                        self.notifications.push(
                            NotificationSeverity::Error,
                            "Commit Failed",
                            err,
                            None,
                            None,
                        );
                    }
                }
                Task::none()
            }
            Message::GitToggleDiff(path) => {
                if self.workspace_root.is_some() {
                    if let Err(error) = self.git.toggle_workspace_diff(&path) {
                        self.report_error("Git diff failed", error);
                    }
                    return Task::none();
                }
                self.git.toggle_diff_for(&path);
                Task::none()
            }

            // --- Run & Debug ---
            Message::DebugStart => {
                if self.workspace_root.is_some() {
                    self.report_error(
                        "Debug unavailable",
                        "A debug adapter is not configured".to_string(),
                    );
                    return Task::none();
                }
                self.debug.start();
                self.terminal.active_tab = PanelTab::DebugConsole;
                self.terminal
                    .log(LogLevel::Info, "Started debugging session (LLDB attached)");
                self.notifications.push(
                    NotificationSeverity::Info,
                    "Debugger Started",
                    "Hit breakpoint at src/lib.rs:42",
                    None,
                    None,
                );
                Task::none()
            }
            Message::DebugPause => {
                if self.workspace_root.is_some() {
                    return Task::none();
                }
                self.debug.pause();
                Task::none()
            }
            Message::DebugContinue => {
                if self.workspace_root.is_some() {
                    return Task::none();
                }
                self.debug.continue_exec();
                Task::none()
            }
            Message::DebugStepOver => {
                if self.workspace_root.is_some() {
                    return Task::none();
                }
                self.debug.step_over();
                self.terminal
                    .log(LogLevel::Info, "Step over -> src/lib.rs:43");
                Task::none()
            }
            Message::DebugStop => {
                if self.workspace_root.is_some() {
                    return Task::none();
                }
                self.debug.stop();
                self.terminal
                    .log(LogLevel::Info, "Debugging session terminated.");
                Task::none()
            }
            Message::DebugToggleBreakpoint(file, line) => {
                self.debug.toggle_breakpoint(&file, line);
                Task::none()
            }
            Message::DebugWatchInputChanged(expr) => {
                self.debug.new_watch_input = expr;
                Task::none()
            }
            Message::DebugAddWatch => {
                let expr = self.debug.new_watch_input.clone();
                self.debug.add_watch(&expr);
                Task::none()
            }
            Message::DebugRemoveWatch(idx) => {
                self.debug.remove_watch(idx);
                Task::none()
            }

            // --- Extensions Marketplace ---
            Message::ExtensionQueryChanged(q) => {
                self.extensions.query = q;
                Task::none()
            }
            Message::ExtensionFilterSelected(filter) => {
                self.extensions.filter = filter;
                Task::none()
            }
            Message::ExtensionToggleInstall(id) => {
                if self.workspace_root.is_some() {
                    self.report_error(
                        "Extensions unavailable",
                        "Extension installation is not implemented yet".to_string(),
                    );
                    return Task::none();
                }
                self.extensions.toggle_install(id);
                Task::none()
            }
            Message::ExtensionToggleEnabled(id) => {
                if self.workspace_root.is_some() {
                    self.report_error(
                        "Extensions unavailable",
                        "Extension hosting is not implemented yet".to_string(),
                    );
                    return Task::none();
                }
                self.extensions.toggle_enabled(id);
                Task::none()
            }

            // --- Multi-Terminal & Output ---
            Message::NewTerminalTab => {
                let Some(root) = self.workspace_root.as_ref() else {
                    self.terminal.new_terminal("zsh");
                    return Task::none();
                };
                self.terminal.new_terminal("Shell");
                let id = self.terminal.terminals.last().unwrap().id;
                match terminal::TerminalSession::start(root, id) {
                    Ok((mut session, task)) => {
                        if self.terminal_expanded {
                            let _ = session.resize(
                                terminal::TERMINAL_EXPANDED_ROWS,
                                terminal::TERMINAL_COLUMNS,
                            );
                        }
                        if let Some(instance) = self.terminal.terminals.last_mut() {
                            instance.running = true;
                            instance.lines = session.scroll(0).to_vec();
                        }
                        self.terminal_sessions.insert(id, session);
                        self.terminal.active_tab = PanelTab::Terminal;
                        self.show_terminal = true;
                        self.terminal_raw_focus = true;
                        task
                    }
                    Err(error) => {
                        self.terminal_raw_focus = false;
                        self.terminal.terminals.pop();
                        self.terminal.active_terminal_idx =
                            self.terminal.terminals.len().saturating_sub(1);
                        self.report_error("Terminal failed", error);
                        Task::none()
                    }
                }
            }
            Message::PersistRecovery(revision) => {
                if revision == self.recovery_revision {
                    self.flush_recovery();
                }
                Task::none()
            }
            Message::RestoreRecovery(index) => {
                self.restore_recovery(index);
                Task::none()
            }
            Message::DiscardRecovery(index) => {
                if let Some(store) = &mut self.recovery_store {
                    if let Err(error) = store.dismiss(index) {
                        self.report_error("Discard failed", error.to_string());
                    }
                }
                Task::none()
            }
            Message::CloseTerminalTab(idx) => {
                if self.workspace_root.is_some() {
                    if idx < self.terminal.terminals.len() {
                        let instance = self.terminal.terminals.remove(idx);
                        self.terminal_sessions.remove(&instance.id);
                        self.terminal.active_terminal_idx = self
                            .terminal
                            .active_terminal_idx
                            .min(self.terminal.terminals.len().saturating_sub(1));
                        if self.terminal.terminals.is_empty() {
                            self.terminal_raw_focus = false;
                        }
                    }
                } else {
                    self.terminal.close_terminal(idx);
                }
                Task::none()
            }
            Message::SelectTerminalInstance(idx) => {
                if idx < self.terminal.terminals.len() {
                    self.terminal.active_terminal_idx = idx;
                }
                Task::none()
            }
            Message::TerminalInputChanged(input) => {
                if let Some(instance) = self
                    .terminal
                    .terminals
                    .get_mut(self.terminal.active_terminal_idx)
                {
                    instance.input = input;
                }
                Task::none()
            }
            Message::TerminalSubmit => {
                if let Some(instance) = self
                    .terminal
                    .terminals
                    .get_mut(self.terminal.active_terminal_idx)
                {
                    if let Some(session) = self.terminal_sessions.get_mut(&instance.id) {
                        let command = std::mem::take(&mut instance.input);
                        if let Err(error) = session.write(format!("{command}\r").as_bytes()) {
                            self.report_error("Terminal input failed", error.to_string());
                        }
                    }
                }
                Task::none()
            }
            Message::TerminalInterrupt => {
                if let Some(instance) = self
                    .terminal
                    .terminals
                    .get(self.terminal.active_terminal_idx)
                {
                    if let Some(session) = self.terminal_sessions.get_mut(&instance.id) {
                        let _ = session.write(&[3]);
                    }
                }
                Task::none()
            }
            Message::TerminalOutput(id, bytes) => {
                if let Some(session) = self.terminal_sessions.get_mut(&id) {
                    let lines = session.receive(&bytes);
                    if let Some(instance) = self
                        .terminal
                        .terminals
                        .iter_mut()
                        .find(|instance| instance.id == id)
                    {
                        instance.lines = lines.to_vec();
                    }
                }
                Task::none()
            }
            Message::TerminalExited(id) => {
                if let Some(instance) = self
                    .terminal
                    .terminals
                    .iter_mut()
                    .find(|instance| instance.id == id)
                {
                    instance.running = false;
                    let code = self
                        .terminal_sessions
                        .get_mut(&id)
                        .and_then(|session| session.exit_code());
                    instance.lines.push(format!(
                        "[shell exited{}]",
                        code.map(|code| format!(" with code {code}"))
                            .unwrap_or_default()
                    ));
                }
                Task::none()
            }
            Message::TerminalRawInput(bytes) => {
                if self.terminal_raw_focus {
                    if let Some(instance) = self
                        .terminal
                        .terminals
                        .get(self.terminal.active_terminal_idx)
                    {
                        if let Some(session) = self.terminal_sessions.get_mut(&instance.id) {
                            if let Err(error) = session.write(&bytes) {
                                self.report_error("Terminal input failed", error.to_string());
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::TerminalFocusChanged(focused) => {
                self.terminal_raw_focus = focused
                    && self.show_terminal
                    && self.terminal.active_tab == PanelTab::Terminal
                    && self
                        .terminal
                        .terminals
                        .get(self.terminal.active_terminal_idx)
                        .is_some_and(|instance| instance.running);
                Task::none()
            }
            Message::TerminalScroll(delta) => {
                if let Some(instance) = self
                    .terminal
                    .terminals
                    .get_mut(self.terminal.active_terminal_idx)
                {
                    if let Some(session) = self.terminal_sessions.get_mut(&instance.id) {
                        instance.lines = session.scroll(delta).to_vec();
                    }
                }
                Task::none()
            }
            Message::ToggleTerminalMaximize => {
                self.terminal_expanded = !self.terminal_expanded;
                let rows = if self.terminal_expanded {
                    terminal::TERMINAL_EXPANDED_ROWS
                } else {
                    terminal::TERMINAL_COLLAPSED_ROWS
                };
                for instance in &mut self.terminal.terminals {
                    if let Some(session) = self.terminal_sessions.get_mut(&instance.id) {
                        if session.resize(rows, terminal::TERMINAL_COLUMNS).is_ok() {
                            instance.lines = session.scroll(0).to_vec();
                        }
                    }
                }
                Task::none()
            }
            Message::TerminalIgnored => Task::none(),
            Message::SelectOutputChannel(ch) => {
                self.terminal.active_channel = ch;
                Task::none()
            }
            Message::DebugReplInputChanged(inp) => {
                self.terminal.repl_input = inp;
                Task::none()
            }
            Message::DebugReplSubmit => {
                if self.workspace_root.is_some() {
                    self.report_error(
                        "Debug console unavailable",
                        "No debug adapter is connected".to_string(),
                    );
                    return Task::none();
                }
                self.terminal.evaluate_repl();
                Task::none()
            }

            // --- Notifications ---
            Message::DismissNotification(id) => {
                self.notifications.dismiss(id);
                Task::none()
            }
            Message::ToggleNotificationCenter => {
                self.notifications.toggle_center();
                Task::none()
            }

            // --- Settings ---
            Message::SettingsSearchChanged(q) => {
                self.settings_search = q;
                Task::none()
            }

            // --- Autocomplete ---
            Message::TriggerAutocomplete => {
                if self.workspace_root.is_none() {
                    self.autocomplete.open();
                    return Task::none();
                }
                if self.language_server_status != lsp::ServerStatus::Ready {
                    self.report_error(
                        "Completion unavailable",
                        "rust-analyzer is not ready".to_string(),
                    );
                    return Task::none();
                }
                let Some(doc) = self.docs.active() else {
                    return Task::none();
                };
                let (line, column) = doc.cursor_position();
                let path = doc.path.clone();
                match self
                    .language_server
                    .as_mut()
                    .expect("ready language server")
                    .completion(&path, line.saturating_sub(1), column.saturating_sub(1))
                {
                    Ok(id) => self.lsp_completion_request = Some(id),
                    Err(error) => {
                        self.language_server_status = lsp::ServerStatus::Failed;
                        self.report_error("Completion failed", error.to_string());
                    }
                }
                Task::none()
            }
            Message::AutocompleteSelectNext => {
                self.autocomplete.select_next();
                Task::none()
            }
            Message::AutocompleteSelectPrev => {
                self.autocomplete.select_prev();
                Task::none()
            }
            Message::SelectAutocomplete(idx) => {
                let insertion = self
                    .autocomplete
                    .filtered()
                    .get(idx)
                    .map(|item| item.label.clone());
                self.autocomplete.selected_index = idx;
                self.autocomplete.close();
                if let Some(insertion) = insertion {
                    self.update(Message::EditorAction(text_editor::Action::Edit(
                        text_editor::Edit::Paste(Arc::new(insertion)),
                    )))
                } else {
                    Task::none()
                }
            }
            Message::CloseAutocomplete => {
                self.autocomplete.close();
                Task::none()
            }
            Message::GoToDefinition => {
                let Some(doc) = self.docs.active() else {
                    return Task::none();
                };
                let path = doc.path.clone();
                let cursor = doc.content.cursor().position;
                let word = doc.word_under_cursor();

                if self.language_server_status == lsp::ServerStatus::Ready {
                    if let Some(server) = self.language_server.as_mut() {
                        match server.definition(&path, cursor.line, cursor.column) {
                            Ok(id) => {
                                self.lsp_definition_request = Some(id);
                                return Task::none();
                            }
                            Err(error) => {
                                self.terminal.log(LogLevel::Warning, format!("LSP definition error: {error}"));
                            }
                        }
                    }
                }

                if let Some(token) = word {
                    self.fast_simd_definition_jump(&token);
                }
                Task::none()
            }
            Message::FindReferences => {
                let Some(doc) = self.docs.active() else {
                    return Task::none();
                };
                let path = doc.path.clone();
                let cursor = doc.content.cursor().position;
                let word = doc.word_under_cursor();

                if self.language_server_status == lsp::ServerStatus::Ready {
                    if let Some(server) = self.language_server.as_mut() {
                        match server.references(&path, cursor.line, cursor.column) {
                            Ok(id) => {
                                self.lsp_references_request = Some(id);
                                return Task::none();
                            }
                            Err(error) => {
                                self.terminal.log(LogLevel::Warning, format!("LSP references error: {error}"));
                            }
                        }
                    }
                }

                if let Some(token) = word {
                    self.search.query = token;
                    self.show_sidebar = true;
                    self.activity.select(SidebarView::Search);
                    return self.schedule_search(true);
                }
                Task::none()
            }
            Message::ShowHover => {
                let Some(doc) = self.docs.active() else {
                    return Task::none();
                };
                let path = doc.path.clone();
                let cursor = doc.content.cursor().position;
                let word = doc.word_under_cursor();

                if self.language_server_status == lsp::ServerStatus::Ready {
                    if let Some(server) = self.language_server.as_mut() {
                        match server.hover(&path, cursor.line, cursor.column) {
                            Ok(id) => {
                                self.lsp_hover_request = Some(id);
                                return Task::none();
                            }
                            Err(error) => {
                                self.terminal.log(LogLevel::Warning, format!("LSP hover error: {error}"));
                            }
                        }
                    }
                }

                if let Some(token) = word {
                    self.hover.show(&token, &format!("symbol `{token}`"), "Workspace symbol (instant inspection)");
                }
                Task::none()
            }
            Message::NavigateToLocation(path, line, col) => {
                self.jump_to_file_location(&path, line, col);
                Task::none()
            }
            Message::AcceptConflictCurrent(index) => {
                if let Some(doc) = self.docs.active_mut() {
                    doc.resolve_conflict(index, document::ConflictChoice::AcceptCurrent);
                    self.terminal.log(LogLevel::Success, "Resolved merge conflict (kept current change)");
                }
                Task::none()
            }
            Message::AcceptConflictIncoming(index) => {
                if let Some(doc) = self.docs.active_mut() {
                    doc.resolve_conflict(index, document::ConflictChoice::AcceptIncoming);
                    self.terminal.log(LogLevel::Success, "Resolved merge conflict (accepted incoming change)");
                }
                Task::none()
            }
            Message::AcceptConflictBoth(index) => {
                if let Some(doc) = self.docs.active_mut() {
                    doc.resolve_conflict(index, document::ConflictChoice::AcceptBoth);
                    self.terminal.log(LogLevel::Success, "Resolved merge conflict (kept both changes)");
                }
                Task::none()
            }
            Message::AcceptAllConflicts(choice) => {
                if let Some(doc) = self.docs.active_mut() {
                    let count = doc.resolve_all_conflicts(choice);
                    self.terminal.log(LogLevel::Success, format!("Resolved {count} merge conflict(s)"));
                }
                Task::none()
            }
            Message::ToggleFold(doc_idx, line) => {
                if let Some(doc) = self.docs.documents.get_mut(doc_idx) {
                    doc.toggle_fold_at(line);
                }
                Task::none()
            }
            Message::FoldAll => {
                if let Some(doc) = self.docs.active_mut() {
                    doc.fold_all();
                }
                Task::none()
            }
            Message::UnfoldAll => {
                if let Some(doc) = self.docs.active_mut() {
                    doc.unfold_all();
                }
                Task::none()
            }
            Message::LanguageServerEvent(event) => {
                match event {
                    lsp::ServerEvent::Message(value) => {
                        if value.get("id").and_then(serde_json::Value::as_u64) == Some(1) {
                            if value.get("result").is_some() {
                                let initialized = self
                                    .language_server
                                    .as_mut()
                                    .and_then(|server| server.initialized().err());
                                if let Some(error) = initialized {
                                    self.language_server_status = lsp::ServerStatus::Failed;
                                    self.report_error(
                                        "Language server initialization failed",
                                        error.to_string(),
                                    );
                                } else {
                                    self.language_server_status = lsp::ServerStatus::Ready;
                                    self.sync_open_documents_to_language_server();
                                    self.terminal.log(LogLevel::Success, "rust-analyzer ready");
                                }
                            } else {
                                self.language_server_status = lsp::ServerStatus::Failed;
                                self.report_error(
                                    "Language server initialization failed",
                                    value.get("error").map(ToString::to_string).unwrap_or_else(
                                        || "invalid initialize response".to_string(),
                                    ),
                                );
                            }
                        } else if value.get("id").and_then(serde_json::Value::as_u64)
                            == self.lsp_completion_request
                        {
                            self.lsp_completion_request = None;
                            self.autocomplete
                                .replace_items(lsp::parse_completion_items(&value));
                        } else if value.get("id").and_then(serde_json::Value::as_u64)
                            == self.lsp_definition_request
                        {
                            self.lsp_definition_request = None;
                            if let Some(locations) = lsp::parse_definition_result(&value) {
                                if let Some(first) = locations.first() {
                                    let target_path = self.language_server.as_ref()
                                        .and_then(|s| s.relative_path(&first.uri))
                                        .unwrap_or_else(|| first.uri.clone());
                                    self.jump_to_file_location(&target_path, first.line, first.column);
                                }
                            }
                        } else if value.get("id").and_then(serde_json::Value::as_u64)
                            == self.lsp_references_request
                        {
                            self.lsp_references_request = None;
                            if let Some(locations) = lsp::parse_references_result(&value) {
                                if let Some(first) = locations.first() {
                                    let target_path = self.language_server.as_ref()
                                        .and_then(|s| s.relative_path(&first.uri))
                                        .unwrap_or_else(|| first.uri.clone());
                                    self.jump_to_file_location(&target_path, first.line, first.column);
                                }
                            }
                        } else if value.get("id").and_then(serde_json::Value::as_u64)
                            == self.lsp_hover_request
                        {
                            self.lsp_hover_request = None;
                            if let Some((sig, doc)) = lsp::parse_hover_result(&value) {
                                if let Some(active_doc) = self.docs.active() {
                                    let symbol = active_doc.word_under_cursor().unwrap_or_default();
                                    self.hover.show(&symbol, &sig, &doc);
                                }
                            }
                        } else {
                            self.publish_language_diagnostics(&value);
                        }
                    }
                    lsp::ServerEvent::Stderr(line) => {
                        self.terminal.output_logs.push(terminal::LogEntry {
                            time: "now".to_string(),
                            level: LogLevel::Info,
                            message: line,
                        });
                    }
                    lsp::ServerEvent::Exited => {
                        if self.language_server_status != lsp::ServerStatus::Failed {
                            self.language_server_status = lsp::ServerStatus::Stopped;
                        }
                        self.language_server = None;
                        self.lsp_completion_request = None;
                        self.lsp_definition_request = None;
                        self.lsp_references_request = None;
                        self.lsp_hover_request = None;
                    }
                }
                Task::none()
            }
        }
    }

    /// Primary view method rendering workspace, modals, and notifications.
    pub fn view(&self) -> Element<'_, Message> {
        let base_content = self.view_base_ide();

        let mut layers = vec![base_content];

        // Command Palette Modal
        if self.palette.is_open {
            layers.push(self.view_command_palette());
        }

        // Floating Toast Notifications (Bottom Right)
        if !self.notifications.active_toasts.is_empty() {
            layers.push(self.view_toast_notifications());
        }

        if layers.len() == 1 {
            layers.pop().unwrap()
        } else {
            stack(layers).into()
        }
    }

    fn view_base_ide(&self) -> Element<'_, Message> {
        let top_bar = self.view_top_bar();

        let mut main_row = row![].spacing(1).height(Length::Fill);

        // 1. Iconic Activity Bar (Far-Left Rail, 48px)
        main_row = main_row.push(self.view_activity_bar());

        // 2. Dynamic Primary Sidebar (240px wide)
        if self.show_sidebar {
            main_row = main_row.push(self.view_sidebar());
        }

        // 3. Center Column: Breadcrumbs + Editors + Bottom Dock
        let mut center_col = column![]
            .spacing(2)
            .width(Length::Fill)
            .height(Length::Fill);

        // Breadcrumbs navigation bar
        center_col = center_col.push(self.view_breadcrumbs());

        // Merge conflict banner if active document has conflicts
        if let Some(doc) = self.docs.active() {
            if doc.has_merge_conflicts() {
                center_col = center_col.push(self.view_merge_conflict_banner());
            }
        }

        // Editor Area or Git Diff View
        if self.git.is_diff_view_open {
            center_col = center_col.push(self.view_git_diff_editor());
        } else {
            center_col = center_col.push(self.view_editors_area());
        }

        // Bottom Console / Terminal / Debug Dock
        if self.show_terminal {
            center_col = center_col.push(self.view_terminal());
        }

        main_row = main_row.push(center_col);

        // 4. Status Bar (Bottom)
        let status_bar = self.view_status_bar();

        let full_layout = column![top_bar, main_row, status_bar].spacing(2).padding(2);

        container(full_layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_top_bar(&self) -> Element<'_, Message> {
        if self.workspace_root.is_some() {
            return self.view_workspace_top_bar();
        }
        let logo = row![
            text("◈").size(15),
            text("VIRGO").size(13),
            container(text("IDE").size(10))
                .padding(Padding {
                    top: 1.0,
                    bottom: 1.0,
                    left: 4.0,
                    right: 4.0
                })
                .style(container::rounded_box),
            container(text("v0.15.0-dev").size(9))
                .padding(Padding {
                    top: 1.0,
                    bottom: 1.0,
                    left: 5.0,
                    right: 5.0
                })
                .style(container::rounded_box),
            container(text(format!("⑂ {}", self.git.active_branch)).size(9))
                .padding(Padding {
                    top: 1.0,
                    bottom: 1.0,
                    left: 5.0,
                    right: 5.0
                })
                .style(container::bordered_box),
        ]
        .spacing(6)
        .align_y(Center);

        let actions = row![
            button(
                row![text("▷").size(11), text("Run")]
                    .spacing(5)
                    .align_y(Center)
            )
            .style(button::secondary)
            .padding(Padding {
                top: 3.0,
                bottom: 3.0,
                left: 10.0,
                right: 10.0
            })
            .on_press(Message::TriggerRun),
            button(
                row![text("⚡").size(11), text("Build")]
                    .spacing(5)
                    .align_y(Center)
            )
            .style(button::secondary)
            .padding(Padding {
                top: 3.0,
                bottom: 3.0,
                left: 10.0,
                right: 10.0
            })
            .on_press(Message::TriggerBuild),
            button(
                row![text("✓").size(11), text("Check")]
                    .spacing(5)
                    .align_y(Center)
            )
            .style(button::text)
            .padding(Padding {
                top: 3.0,
                bottom: 3.0,
                left: 8.0,
                right: 8.0
            })
            .on_press(Message::TriggerCheck),
        ]
        .spacing(5)
        .align_y(Center);

        // Command Palette Trigger Button
        let palette_btn = button(
            row![
                text("⌕").size(12),
                text("Quick Open / Commands").size(11),
                container(text("⌘P").size(9))
                    .padding(Padding {
                        top: 1.0,
                        bottom: 1.0,
                        left: 4.0,
                        right: 4.0
                    })
                    .style(container::rounded_box),
            ]
            .spacing(8)
            .align_y(Center),
        )
        .style(button::secondary)
        .padding(Padding {
            top: 3.0,
            bottom: 3.0,
            left: 12.0,
            right: 12.0,
        })
        .on_press(Message::OpenCommandPalette(false));

        let theme_controls = row![
            button(text("Dark").size(11))
                .style(if self.theme_mode == ThemeMode::Dark {
                    button::primary
                } else {
                    button::text
                })
                .padding(Padding {
                    top: 2.0,
                    bottom: 2.0,
                    left: 7.0,
                    right: 7.0
                })
                .on_press(Message::ThemeSelected(ThemeMode::Dark)),
            button(text("Light").size(11))
                .style(if self.theme_mode == ThemeMode::Light {
                    button::primary
                } else {
                    button::text
                })
                .padding(Padding {
                    top: 2.0,
                    bottom: 2.0,
                    left: 7.0,
                    right: 7.0
                })
                .on_press(Message::ThemeSelected(ThemeMode::Light)),
            button(text("High Contrast").size(11))
                .style(if self.theme_mode == ThemeMode::HighContrast {
                    button::primary
                } else {
                    button::text
                })
                .padding(Padding {
                    top: 2.0,
                    bottom: 2.0,
                    left: 7.0,
                    right: 7.0
                })
                .on_press(Message::ThemeSelected(ThemeMode::HighContrast)),
        ]
        .spacing(3)
        .align_y(Center);

        let layout_toggles = row![
            button(
                text(if self.git.is_diff_view_open {
                    "⇄ Diff"
                } else {
                    "◻ Diff"
                })
                .size(11)
            )
            .style(if self.git.is_diff_view_open {
                button::primary
            } else {
                button::secondary
            })
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 7.0,
                right: 7.0
            })
            .on_press(Message::GitToggleDiff("src/lib.rs".to_string())),
            button(
                text(if self.docs.split_tab.is_some() {
                    "◫ Split"
                } else {
                    "◻ Split"
                })
                .size(11)
            )
            .style(if self.docs.split_tab.is_some() {
                button::primary
            } else {
                button::secondary
            })
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 7.0,
                right: 7.0
            })
            .on_press(Message::ToggleSplitEditor),
            button(
                text(if self.docs.find_open {
                    "⌕ Find"
                } else {
                    "◻ Find"
                })
                .size(11)
            )
            .style(if self.docs.find_open {
                button::primary
            } else {
                button::secondary
            })
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 7.0,
                right: 7.0
            })
            .on_press(Message::ToggleFind),
            button(
                text(if self.show_sidebar {
                    "◧ Sidebar"
                } else {
                    "◻ Sidebar"
                })
                .size(11)
            )
            .style(button::secondary)
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 7.0,
                right: 7.0
            })
            .on_press(Message::ToggleSidebar),
            button(
                text(if self.show_terminal {
                    "⬒ Panel"
                } else {
                    "◻ Panel"
                })
                .size(11)
            )
            .style(button::secondary)
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 7.0,
                right: 7.0
            })
            .on_press(Message::ToggleTerminal),
            button(text("－").size(11))
                .style(button::text)
                .padding(Padding {
                    top: 1.0,
                    bottom: 1.0,
                    left: 5.0,
                    right: 5.0
                })
                .on_press(Message::ZoomOut),
            button(text(format!("{}%", (self.font_size / 13.0 * 100.0) as u32)).size(10))
                .style(button::text)
                .padding(Padding {
                    top: 1.0,
                    bottom: 1.0,
                    left: 4.0,
                    right: 4.0
                })
                .on_press(Message::ZoomReset),
            button(text("＋").size(11))
                .style(button::text)
                .padding(Padding {
                    top: 1.0,
                    bottom: 1.0,
                    left: 5.0,
                    right: 5.0
                })
                .on_press(Message::ZoomIn),
        ]
        .spacing(4)
        .align_y(Center);

        let bar = row![
            logo,
            space().width(Length::Fill),
            actions,
            palette_btn,
            space().width(Length::Fill),
            theme_controls,
            layout_toggles,
        ]
        .spacing(8)
        .padding(Padding {
            top: 4.0,
            bottom: 4.0,
            left: 10.0,
            right: 10.0,
        })
        .align_y(Center);

        container(bar)
            .style(container::bordered_box)
            .width(Length::Fill)
            .into()
    }

    fn view_workspace_top_bar(&self) -> Element<'_, Message> {
        let workspace_name = self
            .workspace_root
            .as_ref()
            .and_then(|root| root.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("Workspace");
        let brand = row![
            text("V").size(17),
            text("VIRGO").size(13),
            text("/").size(12).style(text::secondary),
            text(workspace_name).size(12).style(text::secondary),
        ]
        .spacing(9)
        .align_y(Center);

        let quick_open = button(
            row![
                svg(icons::activity(SidebarView::Search))
                    .width(Length::Fixed(13.0))
                    .height(Length::Fixed(13.0)),
                text("Search files and commands").size(11),
                space().width(Length::Fill),
                text("⌘P").size(10).style(text::secondary),
            ]
            .spacing(12)
            .align_y(Center),
        )
        .width(Length::Fixed(260.0))
        .padding(Padding {
            top: 6.0,
            bottom: 6.0,
            left: 10.0,
            right: 10.0,
        })
        .style(|_, status| graphite_button_style(false, status))
        .on_press(Message::OpenCommandPalette(false));

        let actions = row![
            button(text("Save").size(11))
                .style(button::text)
                .on_press(Message::SaveDocument),
            button(text("Save As").size(11))
                .style(button::text)
                .on_press(Message::OpenSaveAs),
            button(text("Check").size(11))
                .style(button::text)
                .on_press(Message::TriggerCheck),
            button(text("Build").size(11))
                .style(|_, status| graphite_button_style(false, status))
                .on_press(Message::TriggerBuild),
            button(text("Split").size(11))
                .style(button::text)
                .on_press(Message::ToggleSplitEditor),
            button(text("Panel").size(11))
                .style(button::text)
                .on_press(Message::ToggleTerminal),
        ]
        .spacing(2)
        .align_y(Center);

        container(
            row![
                brand,
                space().width(Length::Fill),
                quick_open,
                space().width(Length::Fill),
                actions
            ]
            .spacing(10)
            .padding(Padding {
                top: 7.0,
                bottom: 7.0,
                left: 15.0,
                right: 12.0,
            })
            .align_y(Center),
        )
        .style(container::bordered_box)
        .width(Length::Fill)
        .into()
    }

    /// Renders the iconic VS Code Activity Bar (Far-Left rail with badges).
    fn view_activity_bar(&self) -> Element<'_, Message> {
        let views = [
            SidebarView::Explorer,
            SidebarView::Search,
            SidebarView::SourceControl,
            SidebarView::Debug,
            SidebarView::Extensions,
        ];

        let mut col = column![].spacing(6).align_x(Center);

        for view in views {
            let is_active = self.activity.active_view == view && self.show_sidebar;
            let icon_text = svg(icons::activity(view))
                .width(Length::Fixed(19.0))
                .height(Length::Fixed(19.0));

            let mut btn_content = row![icon_text].align_y(Center);

            // Optional badge
            if let Some(badge_count) = self.activity.badge_for(view) {
                let badge = container(text(format!("{}", badge_count)).size(8))
                    .padding(Padding {
                        top: 1.0,
                        bottom: 1.0,
                        left: 3.0,
                        right: 3.0,
                    })
                    .style(container::primary);
                btn_content = btn_content.push(badge);
            }

            let btn = button(btn_content)
                .padding(Padding {
                    top: 8.0,
                    bottom: 8.0,
                    left: 8.0,
                    right: 8.0,
                })
                .style(if is_active {
                    button::primary
                } else {
                    button::text
                })
                .on_press(Message::SelectSidebarView(view));

            col = col.push(tooltip(
                btn,
                container(text(format!("{}  ·  {}", view.label(), view.shortcut())).size(11))
                    .padding(6)
                    .style(container::rounded_box),
                tooltip::Position::Right,
            ));
        }

        col = col.push(space().height(Length::Fill));

        // Bottom settings gear button
        let settings_active =
            self.activity.active_view == SidebarView::Settings && self.show_sidebar;
        let settings_btn = button(
            svg(icons::activity(SidebarView::Settings))
                .width(Length::Fixed(19.0))
                .height(Length::Fixed(19.0)),
        )
        .padding(Padding {
            top: 8.0,
            bottom: 8.0,
            left: 8.0,
            right: 8.0,
        })
        .style(if settings_active {
            button::primary
        } else {
            button::text
        })
        .on_press(Message::SelectSidebarView(SidebarView::Settings));

        col = col.push(tooltip(
            settings_btn,
            container(text("Settings  ·  Cmd+,").size(11))
                .padding(6)
                .style(container::rounded_box),
            tooltip::Position::Right,
        ));

        container(col)
            .width(Length::Fixed(46.0))
            .height(Length::Fill)
            .padding(Padding {
                top: 6.0,
                bottom: 6.0,
                left: 2.0,
                right: 2.0,
            })
            .style(container::rounded_box)
            .into()
    }

    /// Renders dynamic primary sidebar based on active activity bar selection.
    fn view_sidebar(&self) -> Element<'_, Message> {
        if self.workspace_root.is_some() {
            let unavailable = match self.activity.active_view {
                SidebarView::SourceControl if self.git.repo_root.is_none() => {
                    Some(("SOURCE CONTROL", "No Git repository in this workspace"))
                }
                SidebarView::Debug => Some(("RUN & DEBUG", "No debug adapter is configured")),
                SidebarView::Extensions => Some(("EXTENSIONS", "Extension hosting is being built")),
                _ => None,
            };
            if let Some((title, description)) = unavailable {
                return container(
                    column![
                        text(title).size(11),
                        rule::horizontal(1),
                        text(description).size(11).style(text::secondary),
                    ]
                    .spacing(12)
                    .padding(14),
                )
                .width(Length::Fixed(250.0))
                .height(Length::Fill)
                .style(container::bordered_box)
                .into();
            }
        }
        let content = match self.activity.active_view {
            SidebarView::Explorer => self.view_sidebar_explorer(),
            SidebarView::Search => self.view_sidebar_search(),
            SidebarView::SourceControl => self.view_sidebar_git(),
            SidebarView::Debug => self.view_sidebar_debug(),
            SidebarView::Extensions => self.view_sidebar_extensions(),
            SidebarView::Settings => self.view_sidebar_settings(),
        };

        container(content)
            .width(Length::Fixed(250.0))
            .height(Length::Fill)
            .style(container::bordered_box)
            .into()
    }

    fn view_sidebar_explorer(&self) -> Element<'_, Message> {
        let header = row![
            text("EXPLORER").size(11),
            space().width(Length::Fill),
            button(text("Refresh").size(10))
                .style(button::text)
                .on_press(Message::RefreshWorkspace),
            button(text("Rename").size(10))
                .style(button::text)
                .on_press(Message::OpenRenameSelected),
            button(text("Delete").size(10))
                .style(button::text)
                .on_press(Message::OpenDeleteSelected),
            button(text("＋").size(11))
                .style(button::text)
                .padding(Padding {
                    top: 1.0,
                    bottom: 1.0,
                    left: 4.0,
                    right: 4.0
                })
                .on_press(if self.workspace_root.is_some() {
                    Message::OpenNewFile
                } else {
                    Message::NewScratchpad
                }),
        ]
        .padding(Padding {
            top: 4.0,
            bottom: 4.0,
            left: 8.0,
            right: 8.0,
        })
        .align_y(Center);

        let tree_content = self.view_tree_node(&self.tree.root, 0);

        let mut sidebar_col = column![header, rule::horizontal(1)].spacing(4);
        if self.creating_file {
            sidebar_col = sidebar_col.push(
                column![
                    text("NEW FILE").size(10).style(text::secondary),
                    text_input("src/new_file.rs", &self.new_file_path)
                        .id("virgo-new-file")
                        .on_input(Message::NewFilePathChanged)
                        .on_submit(Message::CreateFile),
                    button(text("Create file").size(11))
                        .style(button::secondary)
                        .on_press(Message::CreateFile),
                ]
                .spacing(6)
                .padding(8),
            );
        }
        if self.saving_as {
            sidebar_col = sidebar_col.push(
                column![
                    text("SAVE AS").size(10).style(text::secondary),
                    text_input("path/to/new_file.rs", &self.save_as_path)
                        .id("virgo-save-as")
                        .on_input(Message::SaveAsPathChanged)
                        .on_submit(Message::ConfirmSaveAs),
                    row![
                        button(text("Save copy").size(11))
                            .style(button::secondary)
                            .on_press(Message::ConfirmSaveAs),
                        button(text("Cancel").size(11))
                            .style(button::text)
                            .on_press(Message::CancelSaveAs),
                    ]
                    .spacing(6),
                ]
                .spacing(6)
                .padding(8),
            );
        }
        if self.renaming_file {
            sidebar_col = sidebar_col.push(
                column![
                    text("RENAME FILE OR FOLDER")
                        .size(10)
                        .style(text::secondary),
                    text_input("new_name.rs", &self.rename_name)
                        .id("virgo-rename-file")
                        .on_input(Message::RenameNameChanged)
                        .on_submit(Message::ConfirmRename),
                    row![
                        button(text("Rename").size(11))
                            .style(button::secondary)
                            .on_press(Message::ConfirmRename),
                        button(text("Cancel").size(11))
                            .style(button::text)
                            .on_press(Message::CancelRename),
                    ]
                    .spacing(6),
                ]
                .spacing(6)
                .padding(8),
            );
        }
        if let Some(path) = &self.deleting_path {
            sidebar_col = sidebar_col.push(
                column![
                    text("MOVE TO VIRGO TRASH").size(10).style(text::secondary),
                    text(path).size(11),
                    text("The entry stays recoverable in .virgo-trash")
                        .size(10)
                        .style(text::secondary),
                    row![
                        button(text("Move to trash").size(11))
                            .style(button::secondary)
                            .on_press(Message::ConfirmDelete),
                        button(text("Cancel").size(11))
                            .style(button::text)
                            .on_press(Message::CancelDelete),
                    ]
                    .spacing(6),
                ]
                .spacing(6)
                .padding(8),
            );
        }
        if let Some(store) = &self.recovery_store {
            if !store.pending().is_empty() {
                let mut drafts = column![
                    text(format!("UNSAVED DRAFTS ({})", store.pending().len()))
                        .size(10)
                        .style(text::secondary)
                ]
                .spacing(6)
                .padding(8);
                for (index, candidate) in store.pending().iter().enumerate() {
                    drafts = drafts.push(
                        column![
                            text(&candidate.entry.path).size(11),
                            row![
                                button(text("Restore").size(10))
                                    .style(button::secondary)
                                    .on_press(Message::RestoreRecovery(index)),
                                button(text("Discard").size(10))
                                    .style(button::text)
                                    .on_press(Message::DiscardRecovery(index)),
                            ]
                            .spacing(6),
                        ]
                        .spacing(4),
                    );
                }
                let height = (44.0 + store.pending().len() as f32 * 54.0).min(210.0);
                sidebar_col = sidebar_col.push(scrollable(drafts).height(Pixels(height)));
            }
        }
        if !self.trash_entries.is_empty() {
            let mut trash = column![
                text(format!("VIRGO TRASH ({})", self.trash_entries.len()))
                    .size(10)
                    .style(text::secondary)
            ]
            .spacing(6)
            .padding(8);
            for (index, entry) in self.trash_entries.iter().enumerate() {
                trash = trash.push(
                    row![
                        text(&entry.original_path).size(11).width(Length::Fill),
                        button(text("Restore").size(10))
                            .style(button::text)
                            .on_press(Message::RestoreTrash(index)),
                    ]
                    .spacing(6)
                    .align_y(Center),
                );
            }
            let height = (38.0 + self.trash_entries.len() as f32 * 32.0).min(160.0);
            sidebar_col = sidebar_col.push(scrollable(trash).height(Pixels(height)));
        }
        sidebar_col = sidebar_col.push(scrollable(tree_content).height(Length::Fill));

        sidebar_col.into()
    }

    fn view_sidebar_search(&self) -> Element<'_, Message> {
        let header = row![
            text(if self.workspace_root.is_some() {
                "SEARCH"
            } else {
                "SEARCH & REPLACE"
            })
            .size(11),
            space().width(Length::Fill),
            button(
                text(if self.search.show_replace {
                    "▼ Replace"
                } else {
                    "▶ Replace"
                })
                .size(10),
            )
            .style(button::text)
            .on_press(Message::ToggleSearchReplace),
        ]
        .padding(Padding {
            top: 4.0,
            bottom: 4.0,
            left: 8.0,
            right: 8.0,
        })
        .align_y(Center);

        let search_input = text_input("Search workspace...", &self.search.query)
            .id("virgo-workspace-search")
            .size(Pixels(12.0))
            .padding(Padding {
                top: 4.0,
                bottom: 4.0,
                left: 6.0,
                right: 6.0,
            })
            .on_input(Message::GlobalSearchQueryChanged);

        let flags_row = row![
            button(text("Aa").size(10))
                .style(if self.search.match_case {
                    button::primary
                } else {
                    button::secondary
                })
                .padding(Padding {
                    top: 2.0,
                    bottom: 2.0,
                    left: 5.0,
                    right: 5.0
                })
                .on_press(Message::ToggleSearchCase),
            button(text("\"ab\"").size(10))
                .style(if self.search.whole_word {
                    button::primary
                } else {
                    button::secondary
                })
                .padding(Padding {
                    top: 2.0,
                    bottom: 2.0,
                    left: 5.0,
                    right: 5.0
                })
                .on_press(Message::ToggleSearchWord),
            button(text(".*").size(10))
                .style(if self.search.use_regex {
                    button::primary
                } else {
                    button::secondary
                })
                .padding(Padding {
                    top: 2.0,
                    bottom: 2.0,
                    left: 5.0,
                    right: 5.0
                })
                .on_press(Message::ToggleSearchRegex),
            space().width(Length::Fill),
            text(if self.search.is_searching {
                "Searching…".to_string()
            } else if self.search.total_matches > search::MAX_VISIBLE_MATCHES {
                format!(
                    "{} results (first {} shown)",
                    self.search.total_matches,
                    search::MAX_VISIBLE_MATCHES
                )
            } else {
                format!("{} results", self.search.total_matches)
            })
            .size(10),
        ]
        .spacing(4)
        .align_y(Center);

        let mut results_col = column![].spacing(4);
        for group in &self.search.results {
            let group_header = row![
                text(if group.is_expanded { "▼" } else { "▶" }).size(9),
                text(&group.file_name).size(11),
                space().width(Length::Fill),
                container(text(format!("{}", group.hits.len())).size(9))
                    .padding(Padding {
                        top: 1.0,
                        bottom: 1.0,
                        left: 4.0,
                        right: 4.0
                    })
                    .style(container::rounded_box),
            ]
            .spacing(5)
            .align_y(Center);

            let group_btn = button(group_header)
                .style(button::text)
                .width(Length::Fill)
                .on_press(Message::ToggleSearchGroup(group.file_path.clone()));

            results_col = results_col.push(group_btn);

            if group.is_expanded {
                for hit in &group.hits {
                    let hit_row = row![
                        text(format!("{}:", hit.line_number))
                            .size(10)
                            .font(taim::Font::MONOSPACE),
                        text(&hit.line_content).size(10).font(taim::Font::MONOSPACE),
                    ]
                    .spacing(6);

                    let hit_btn = button(hit_row)
                        .style(button::text)
                        .width(Length::Fill)
                        .padding(Padding {
                            top: 2.0,
                            bottom: 2.0,
                            left: 14.0,
                            right: 4.0,
                        })
                        .on_press(Message::SearchHitSelected(
                            hit.file_path.clone(),
                            hit.line_number,
                            hit.column,
                        ));

                    results_col = results_col.push(hit_btn);
                }
            }
        }

        let include_input = text_input(
            "Files to include (e.g. **/*.rs)",
            &self.search.files_to_include,
        )
        .size(Pixels(11.0))
        .padding(5)
        .on_input(Message::SearchIncludeChanged);
        let exclude_input = text_input(
            "Files to exclude (e.g. target/**)",
            &self.search.files_to_exclude,
        )
        .size(Pixels(11.0))
        .padding(5)
        .on_input(Message::SearchExcludeChanged);
        let mut col = column![
            header,
            rule::horizontal(1),
            search_input,
            flags_row,
            include_input,
            exclude_input,
        ]
        .spacing(4);

        if let Some(error) = &self.search.search_error {
            col = col.push(text(error).size(10).style(text::danger));
        }

        if self.search.show_replace {
            let replace_input = text_input("Replace with...", &self.search.replace_query)
                .size(Pixels(12.0))
                .padding(Padding {
                    top: 4.0,
                    bottom: 4.0,
                    left: 6.0,
                    right: 6.0,
                })
                .on_input(Message::GlobalSearchReplaceQueryChanged);
            let mut actions = row![
                button(text("Preview Replace").size(10))
                    .style(button::secondary)
                    .on_press(Message::PreviewWorkspaceReplace),
            ]
            .spacing(6);
            if self.replace_undo.is_some() {
                actions = actions.push(
                    button(text("Undo Last Replace").size(10))
                        .style(button::text)
                        .on_press(Message::UndoWorkspaceReplace),
                );
            }
            col = col.push(replace_input).push(actions);
            if let Some(preview) = &self.replace_preview {
                let mut preview_col = column![
                    text(format!(
                        "REVIEW: {} replacement(s) in {} file(s)",
                        preview.total_replacements,
                        preview.files.len()
                    ))
                    .size(10)
                    .style(text::secondary)
                ]
                .spacing(4);
                for change in preview.files.iter().take(8) {
                    preview_col = preview_col.push(
                        column![
                            text(format!("{}  ({})", change.path, change.replacements)).size(10),
                            text(replacement_preview_line(change))
                                .size(9)
                                .font(taim::Font::MONOSPACE)
                                .style(text::secondary),
                        ]
                        .spacing(2),
                    );
                }
                if preview.files.len() > 8 {
                    preview_col = preview_col.push(
                        text(format!("…and {} more", preview.files.len() - 8))
                            .size(10)
                            .style(text::secondary),
                    );
                }
                preview_col = preview_col.push(
                    row![
                        button(text("Apply reviewed changes").size(10))
                            .style(button::secondary)
                            .on_press(Message::ConfirmWorkspaceReplace),
                        button(text("Cancel").size(10))
                            .style(button::text)
                            .on_press(Message::CancelWorkspaceReplace),
                    ]
                    .spacing(6),
                );
                col = col.push(
                    container(preview_col)
                        .padding(6)
                        .style(container::bordered_box),
                );
            }
        }

        col = col.push(rule::horizontal(1));
        col = col.push(scrollable(results_col).height(Length::Fill));
        col.into()
    }

    fn view_sidebar_git(&self) -> Element<'_, Message> {
        let header = row![
            text("SOURCE CONTROL").size(11),
            space().width(Length::Fill),
            button(text("Refresh").size(10))
                .style(button::text)
                .on_press(Message::GitRefresh),
            button(text("✓ Stage All").size(10))
                .style(button::text)
                .on_press(Message::GitStageAll),
        ]
        .padding(Padding {
            top: 4.0,
            bottom: 4.0,
            left: 8.0,
            right: 8.0,
        })
        .align_y(Center);

        let branch_badge = row![
            text("Branch:").size(11),
            container(text(format!("⎇ {}", self.git.active_branch)).size(11))
                .padding(Padding {
                    top: 2.0,
                    bottom: 2.0,
                    left: 6.0,
                    right: 6.0
                })
                .style(container::rounded_box),
        ]
        .spacing(6)
        .align_y(Center);

        let commit_input = text_input("Commit message (Cmd+Enter)...", &self.git.commit_message)
            .size(Pixels(12.0))
            .padding(Padding {
                top: 4.0,
                bottom: 4.0,
                left: 6.0,
                right: 6.0,
            })
            .on_input(Message::GitCommitMessageChanged);

        let commit_btn = button(text(format!("✓ Commit to {}", self.git.active_branch)).size(11))
            .style(button::primary)
            .width(Length::Fill)
            .padding(Padding {
                top: 4.0,
                bottom: 4.0,
                left: 8.0,
                right: 8.0,
            })
            .on_press(Message::GitCommit);

        // Staged Changes Section
        let staged_title = row![
            text(format!(
                "STAGED CHANGES ({})",
                self.git.staged_changes.len()
            ))
            .size(10),
            space().width(Length::Fill),
            button(text("−").size(10))
                .style(button::text)
                .on_press(Message::GitUnstageAll),
        ]
        .align_y(Center);

        let mut staged_col = column![staged_title].spacing(2);
        for change in &self.git.staged_changes {
            let row_item = row![
                container(text(change.status.badge()).size(9))
                    .padding(Padding {
                        top: 1.0,
                        bottom: 1.0,
                        left: 4.0,
                        right: 4.0
                    })
                    .style(container::success),
                text(&change.file_path).size(11),
                space().width(Length::Fill),
                button(text("⇄").size(10))
                    .style(button::text)
                    .on_press(Message::GitToggleDiff(change.file_path.clone())),
                button(text("−").size(10))
                    .style(button::text)
                    .on_press(Message::GitUnstageFile(change.file_path.clone())),
            ]
            .spacing(4)
            .align_y(Center);
            staged_col = staged_col.push(row_item);
        }

        // Unstaged Changes Section
        let unstaged_title = row![
            text(format!("CHANGES ({})", self.git.unstaged_changes.len())).size(10),
            space().width(Length::Fill),
            button(text("＋").size(10))
                .style(button::text)
                .on_press(Message::GitStageAll),
        ]
        .align_y(Center);

        let mut unstaged_col = column![unstaged_title].spacing(2);
        for change in &self.git.unstaged_changes {
            let row_item = row![
                container(text(change.status.badge()).size(9))
                    .padding(Padding {
                        top: 1.0,
                        bottom: 1.0,
                        left: 4.0,
                        right: 4.0
                    })
                    .style(container::secondary),
                text(&change.file_path).size(11),
                space().width(Length::Fill),
                button(text("⇄").size(10))
                    .style(button::text)
                    .on_press(Message::GitToggleDiff(change.file_path.clone())),
                button(text("＋").size(10))
                    .style(button::text)
                    .on_press(Message::GitStageFile(change.file_path.clone())),
            ]
            .spacing(4)
            .align_y(Center);
            unstaged_col = unstaged_col.push(row_item);
        }

        let full_col = column![
            header,
            rule::horizontal(1),
            branch_badge,
            commit_input,
            commit_btn,
            rule::horizontal(1),
            scrollable(
                column![staged_col, space().height(Length::Fixed(8.0)), unstaged_col].spacing(6)
            )
            .height(Length::Fill),
        ]
        .spacing(6);

        full_col.into()
    }

    fn view_sidebar_debug(&self) -> Element<'_, Message> {
        let header = row![
            text("RUN & DEBUG").size(11),
            space().width(Length::Fill),
            container(text(self.debug.status.badge()).size(9))
                .padding(Padding {
                    top: 1.0,
                    bottom: 1.0,
                    left: 4.0,
                    right: 4.0
                })
                .style(match self.debug.status {
                    DebugStatus::Stopped => container::secondary,
                    DebugStatus::Running => container::success,
                    DebugStatus::Paused => container::primary,
                }),
        ]
        .padding(Padding {
            top: 4.0,
            bottom: 4.0,
            left: 8.0,
            right: 8.0,
        })
        .align_y(Center);

        let controls = row![
            button(text("▶").size(12))
                .style(button::success)
                .padding(Padding {
                    top: 3.0,
                    bottom: 3.0,
                    left: 8.0,
                    right: 8.0
                })
                .on_press(Message::DebugStart),
            button(text("⏸").size(12))
                .style(button::secondary)
                .padding(Padding {
                    top: 3.0,
                    bottom: 3.0,
                    left: 8.0,
                    right: 8.0
                })
                .on_press(Message::DebugPause),
            button(text("↷").size(12))
                .style(button::secondary)
                .padding(Padding {
                    top: 3.0,
                    bottom: 3.0,
                    left: 8.0,
                    right: 8.0
                })
                .on_press(Message::DebugStepOver),
            button(text("⏹").size(12))
                .style(button::danger)
                .padding(Padding {
                    top: 3.0,
                    bottom: 3.0,
                    left: 8.0,
                    right: 8.0
                })
                .on_press(Message::DebugStop),
        ]
        .spacing(4)
        .align_y(Center);

        // Variables Inspector Section
        let mut vars_col = column![text("VARIABLES").size(10)].spacing(2);
        for var in &self.debug.variables {
            let line = row![
                text(&var.name).size(10).font(taim::Font::MONOSPACE),
                text(":").size(10),
                text(&var.var_type).size(9).style(text::secondary),
                text("=").size(10),
                text(&var.value).size(10).font(taim::Font::MONOSPACE),
            ]
            .spacing(4)
            .align_y(Center);
            vars_col = vars_col.push(line);
        }

        // Call Stack Section
        let mut stack_col = column![text("CALL STACK").size(10)].spacing(2);
        for frame in &self.debug.call_stack {
            let line = row![
                text(if frame.is_selected { "▶" } else { " " }).size(9),
                text(&frame.function_name)
                    .size(10)
                    .font(taim::Font::MONOSPACE),
                text(format!("({}:{})", frame.file, frame.line))
                    .size(9)
                    .style(text::secondary),
            ]
            .spacing(4)
            .align_y(Center);
            stack_col = stack_col.push(line);
        }

        // Breakpoints Section
        let mut bp_col = column![text("BREAKPOINTS").size(10)].spacing(2);
        for bp in &self.debug.breakpoints {
            let line = row![
                text(if bp.enabled { "●" } else { "○" }).size(10),
                text(format!("{}:{}", bp.file, bp.line))
                    .size(10)
                    .font(taim::Font::MONOSPACE),
            ]
            .spacing(6)
            .align_y(Center);
            bp_col = bp_col.push(line);
        }

        let full_col = column![
            header,
            rule::horizontal(1),
            controls,
            rule::horizontal(1),
            scrollable(
                column![
                    vars_col,
                    rule::horizontal(1),
                    stack_col,
                    rule::horizontal(1),
                    bp_col
                ]
                .spacing(6)
            )
            .height(Length::Fill),
        ]
        .spacing(6);

        full_col.into()
    }

    fn view_sidebar_extensions(&self) -> Element<'_, Message> {
        let header = row![
            text("EXTENSIONS").size(11),
            space().width(Length::Fill),
            button(text("Refresh").size(10)).style(button::text),
        ]
        .padding(Padding {
            top: 4.0,
            bottom: 4.0,
            left: 8.0,
            right: 8.0,
        })
        .align_y(Center);

        let search_box = text_input(
            "Search extensions in Marketplace...",
            &self.extensions.query,
        )
        .size(Pixels(12.0))
        .padding(Padding {
            top: 4.0,
            bottom: 4.0,
            left: 6.0,
            right: 6.0,
        })
        .on_input(Message::ExtensionQueryChanged);

        let filter_tabs = row![
            button(text("All").size(10))
                .style(if self.extensions.filter == ExtensionFilter::All {
                    button::primary
                } else {
                    button::secondary
                })
                .padding(Padding {
                    top: 2.0,
                    bottom: 2.0,
                    left: 6.0,
                    right: 6.0
                })
                .on_press(Message::ExtensionFilterSelected(ExtensionFilter::All)),
            button(text("Installed").size(10))
                .style(if self.extensions.filter == ExtensionFilter::Installed {
                    button::primary
                } else {
                    button::secondary
                })
                .padding(Padding {
                    top: 2.0,
                    bottom: 2.0,
                    left: 6.0,
                    right: 6.0
                })
                .on_press(Message::ExtensionFilterSelected(ExtensionFilter::Installed)),
            button(text("Recommended").size(10))
                .style(if self.extensions.filter == ExtensionFilter::Recommended {
                    button::primary
                } else {
                    button::secondary
                })
                .padding(Padding {
                    top: 2.0,
                    bottom: 2.0,
                    left: 6.0,
                    right: 6.0
                })
                .on_press(Message::ExtensionFilterSelected(
                    ExtensionFilter::Recommended
                )),
        ]
        .spacing(4)
        .align_y(Center);

        let mut list_col = column![].spacing(6);
        for ext in self.extensions.filtered() {
            let top_line = row![
                text(ext.icon).size(14),
                column![
                    text(&ext.name).size(11),
                    text(format!(
                        "{} • {} DLs • ★ {:.1}",
                        ext.publisher, ext.downloads, ext.rating
                    ))
                    .size(9)
                    .style(text::secondary),
                ]
                .spacing(1),
            ]
            .spacing(6)
            .align_y(Center);

            let desc = text(&ext.description).size(10);

            let action_btn = button(
                text(if ext.is_installed {
                    "Uninstall"
                } else {
                    "Install"
                })
                .size(10),
            )
            .style(if ext.is_installed {
                button::secondary
            } else {
                button::primary
            })
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 6.0,
                right: 6.0,
            })
            .on_press(Message::ExtensionToggleInstall(ext.id));

            let ext_card = container(column![top_line, desc, action_btn].spacing(4))
                .padding(6)
                .style(container::bordered_box)
                .width(Length::Fill);

            list_col = list_col.push(ext_card);
        }

        column![
            header,
            rule::horizontal(1),
            search_box,
            filter_tabs,
            rule::horizontal(1),
            scrollable(list_col).height(Length::Fill),
        ]
        .spacing(4)
        .into()
    }

    fn view_sidebar_settings(&self) -> Element<'_, Message> {
        if self.workspace_root.is_some() {
            return column![
                text("SETTINGS").size(11),
                rule::horizontal(1),
                text("APPEARANCE").size(10).style(text::secondary),
                button(text("Graphite dark").size(11))
                    .style(if self.theme_mode == ThemeMode::Dark {
                        button::primary
                    } else {
                        button::text
                    })
                    .on_press(Message::ThemeSelected(ThemeMode::Dark)),
                button(text("Light").size(11))
                    .style(if self.theme_mode == ThemeMode::Light {
                        button::primary
                    } else {
                        button::text
                    })
                    .on_press(Message::ThemeSelected(ThemeMode::Light)),
                button(text("High contrast").size(11))
                    .style(if self.theme_mode == ThemeMode::HighContrast {
                        button::primary
                    } else {
                        button::text
                    })
                    .on_press(Message::ThemeSelected(ThemeMode::HighContrast)),
                rule::horizontal(1),
                text("EDITOR FONT").size(10).style(text::secondary),
                row![
                    button(text("−").size(12))
                        .style(button::text)
                        .on_press(Message::ZoomOut),
                    text(format!("{} px", self.font_size as u32)).size(11),
                    button(text("+").size(12))
                        .style(button::text)
                        .on_press(Message::ZoomIn),
                ]
                .spacing(7)
                .align_y(Center),
                text("Settings apply to this session.")
                    .size(10)
                    .style(text::secondary),
            ]
            .spacing(9)
            .padding(12)
            .into();
        }
        let header = row![text("SETTINGS").size(11)].padding(Padding {
            top: 4.0,
            bottom: 4.0,
            left: 8.0,
            right: 8.0,
        });

        let search_box = text_input("Search settings...", &self.settings_search)
            .id("virgo-settings-search")
            .size(Pixels(12.0))
            .padding(Padding {
                top: 4.0,
                bottom: 4.0,
                left: 6.0,
                right: 6.0,
            })
            .on_input(Message::SettingsSearchChanged);

        let content = column![
            text("Text Editor").size(11),
            text("• Tab Size: 4 spaces").size(10),
            text("• Word Wrap: On").size(10),
            text("• Minimap: Enabled").size(10),
            text("• Bracket Pair Colorization: Active").size(10),
            rule::horizontal(1),
            text("Graphics & Engine").size(11),
            text("• Backend: WGPU Metal/Vulkan compute").size(10),
            text("• Target FPS: 120 FPS locked").size(10),
            text("• Gamma: Linear-light sRGB").size(10),
            rule::horizontal(1),
            text("Keyboard & Vim").size(11),
            text("• Shortcut Map: VS Code Standard").size(10),
            text("• Quick Open: Cmd+P").size(10),
            text("• Command Palette: Cmd+Shift+P").size(10),
        ]
        .spacing(6);

        column![
            header,
            rule::horizontal(1),
            search_box,
            rule::horizontal(1),
            scrollable(content).height(Length::Fill)
        ]
        .spacing(4)
        .into()
    }

    fn view_tree_node<'a>(&'a self, node: &'a FileNode, depth: usize) -> Element<'a, Message> {
        let indent = space().width(Pixels(depth as f32 * 12.0));
        let expander = text(if node.is_dir {
            if node.is_expanded { "⌄" } else { "›" }
        } else {
            " "
        })
        .size(11)
        .width(Length::Fixed(10.0));
        let icon = svg(icons::file(node.is_dir))
            .width(Length::Fixed(14.0))
            .height(Length::Fixed(14.0));
        let label = text(&node.name).size(12);

        let is_selected = self.tree.selected_id == Some(node.id);
        let node_row = row![indent, expander, icon, label]
            .spacing(5)
            .align_y(Center);

        let btn = button(node_row)
            .width(Length::Fill)
            .padding(Padding {
                top: 3.0,
                bottom: 3.0,
                left: 4.0,
                right: 4.0,
            })
            .style(move |theme, status| {
                if is_selected {
                    graphite_button_style(true, status)
                } else {
                    button::text(theme, status)
                }
            })
            .on_press(if node.is_dir {
                Message::TreeNodeToggled(node.id)
            } else {
                Message::FileSelected(node.id)
            });

        if node.is_dir && node.is_expanded {
            let mut col = column![btn].spacing(1);
            for child in &node.children {
                col = col.push(self.view_tree_node(child, depth + 1));
            }
            col.into()
        } else {
            btn.into()
        }
    }

    /// Breadcrumbs bar showing path hierarchy above active editor.
    fn view_breadcrumbs(&self) -> Element<'_, Message> {
        let path = self
            .docs
            .active()
            .map(|d| d.path.as_str())
            .unwrap_or("no-file");
        let crumbs = if let Some(root) = &self.workspace_root {
            let mut parts = vec![
                root.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Workspace")
                    .to_string(),
            ];
            if self.docs.active().is_some() {
                parts.extend(
                    path.split('/')
                        .filter(|part| !part.is_empty())
                        .map(str::to_string),
                );
            }
            parts
        } else {
            lsp::resolve_breadcrumbs(path)
        };

        let mut crumb_row = row![].spacing(4).align_y(Center);
        for (i, part) in crumbs.into_iter().enumerate() {
            if i > 0 {
                crumb_row = crumb_row.push(text("›").size(10).style(text::secondary));
            }
            crumb_row = crumb_row.push(text(part).size(10));
        }

        container(crumb_row)
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 6.0,
                right: 6.0,
            })
            .width(Length::Fill)
            .style(container::rounded_box)
            .into()
    }

    /// Git Diff Side-by-Side Unified View.
    fn view_git_diff_editor(&self) -> Element<'_, Message> {
        let header = row![
            text(format!(
                "GIT DIFF: {}",
                self.git.diff_file.as_deref().unwrap_or("Working Tree")
            ))
            .size(11),
            space().width(Length::Fill),
            button(text("✕ Close Diff").size(10))
                .style(button::text)
                .on_press(Message::GitToggleDiff("".to_string())),
        ]
        .padding(Padding {
            top: 4.0,
            bottom: 4.0,
            left: 8.0,
            right: 8.0,
        })
        .align_y(Center);

        let mut diff_col = column![].spacing(1);
        for line in &self.git.diff_lines {
            let old_num = line
                .old_line_num
                .map(|n| format!("{:3}", n))
                .unwrap_or_else(|| "   ".to_string());
            let new_num = line
                .new_line_num
                .map(|n| format!("{:3}", n))
                .unwrap_or_else(|| "   ".to_string());

            let badge_style = match line.kind {
                DiffLineKind::Addition => container::success,
                DiffLineKind::Deletion => container::danger,
                DiffLineKind::Context => container::rounded_box,
            };

            let prefix = match line.kind {
                DiffLineKind::Addition => "+",
                DiffLineKind::Deletion => "-",
                DiffLineKind::Context => " ",
            };

            let line_row = row![
                text(old_num)
                    .size(10)
                    .font(taim::Font::MONOSPACE)
                    .style(text::secondary),
                text(new_num)
                    .size(10)
                    .font(taim::Font::MONOSPACE)
                    .style(text::secondary),
                container(text(prefix).size(10).font(taim::Font::MONOSPACE)).style(badge_style),
                text(&line.content).size(10).font(taim::Font::MONOSPACE),
            ]
            .spacing(6)
            .align_y(Center);

            diff_col = diff_col.push(line_row);
        }

        container(
            column![
                header,
                rule::horizontal(1),
                scrollable(diff_col).height(Length::Fill)
            ]
            .spacing(4),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(container::bordered_box)
        .into()
    }

    fn view_editors_area(&self) -> Element<'_, Message> {
        if let Some(split_idx) = self.docs.split_tab {
            let left_editor = self.view_single_pane(self.docs.active_tab, 0);
            let right_editor = self.view_single_pane(split_idx, 1);

            let dual_row = row![
                container(left_editor)
                    .width(Length::FillPortion(1))
                    .height(Length::Fill),
                rule::vertical(1),
                container(right_editor)
                    .width(Length::FillPortion(1))
                    .height(Length::Fill),
            ]
            .spacing(2)
            .height(Length::Fill);

            dual_row.into()
        } else {
            self.view_single_pane(self.docs.active_tab, 0)
        }
    }

    fn view_single_pane(&self, active_doc_idx: usize, pane_id: usize) -> Element<'_, Message> {
        let tabs_bar = self.view_pane_tabs(active_doc_idx, pane_id);
        let editor_view = self.view_pane_editor(active_doc_idx, pane_id);

        let mut pane_col = column![tabs_bar]
            .spacing(2)
            .width(Length::Fill)
            .height(Length::Fill);

        if self.docs.find_open && pane_id == 0 {
            pane_col = pane_col.push(self.view_find_bar());
        }

        pane_col = pane_col.push(editor_view);
        pane_col.into()
    }

    fn view_pane_tabs(&self, active_doc_idx: usize, pane_id: usize) -> Element<'_, Message> {
        let mut tab_row = row![].spacing(2).align_y(Center);

        for (i, doc) in self.docs.documents.iter().enumerate() {
            let is_active = i == active_doc_idx;

            let icon_str = match doc.syntax.as_str() {
                "rust" => "◈",
                "toml" => "▤",
                "c" => "⚡",
                "markdown" => "≡",
                "json" => "{ }",
                _ => "◇",
            };

            let mut title_row = row![text(icon_str).size(11), text(&doc.name).size(12)]
                .spacing(4)
                .align_y(Center);

            if doc.is_dirty {
                title_row = title_row.push(text("•").size(11));
            }

            let close_btn = button(text("×").size(11))
                .style(button::text)
                .padding(Padding {
                    top: 0.0,
                    bottom: 0.0,
                    left: 3.0,
                    right: 3.0,
                })
                .on_press(Message::TabClosed(i));

            let tab_content = row![title_row, close_btn].spacing(5).align_y(Center);

            let tab_btn = button(tab_content)
                .padding(Padding {
                    top: 3.0,
                    bottom: 3.0,
                    left: 8.0,
                    right: 8.0,
                })
                .style(if is_active {
                    button::primary
                } else {
                    button::secondary
                })
                .on_press(if pane_id == 0 {
                    Message::TabSelected(i)
                } else {
                    Message::SplitTabSelected(i)
                });

            tab_row = tab_row.push(tab_btn);
        }

        let add_tab = button(text("＋").size(12))
            .style(button::text)
            .padding(Padding {
                top: 3.0,
                bottom: 3.0,
                left: 6.0,
                right: 6.0,
            })
            .on_press(if self.workspace_root.is_some() {
                Message::OpenNewFile
            } else {
                Message::NewScratchpad
            });

        tab_row = tab_row.push(add_tab);

        container(tab_row)
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 4.0,
                right: 4.0,
            })
            .width(Length::Fill)
            .style(container::rounded_box)
            .into()
    }

    fn view_find_bar(&self) -> Element<'_, Message> {
        let search_box = text_input("Find in document...", &self.docs.find_query)
            .id("virgo-editor-find")
            .size(Pixels(12.0))
            .padding(Padding {
                top: 3.0,
                bottom: 3.0,
                left: 6.0,
                right: 6.0,
            })
            .width(Length::Fixed(220.0))
            .on_input(Message::FindQueryChanged);

        let match_info = if self.docs.find_query.is_empty() {
            text("No search").size(11)
        } else if self.docs.find_matches == 0 {
            text("0 results").size(11)
        } else {
            text(format!(
                "{} of {}",
                self.docs.current_match, self.docs.find_matches
            ))
            .size(11)
        };

        let prev_btn = button(text("▲").size(10))
            .style(button::text)
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 6.0,
                right: 6.0,
            })
            .on_press(Message::FindPrev);

        let next_btn = button(text("▼").size(10))
            .style(button::text)
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 6.0,
                right: 6.0,
            })
            .on_press(Message::FindNext);

        let close_btn = button(text("✕").size(10))
            .style(button::text)
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 6.0,
                right: 6.0,
            })
            .on_press(Message::ToggleFind);

        let find_row = row![search_box, match_info, prev_btn, next_btn, close_btn]
            .spacing(8)
            .align_y(Center);

        container(find_row)
            .padding(Padding {
                top: 3.0,
                bottom: 3.0,
                left: 8.0,
                right: 8.0,
            })
            .style(container::bordered_box)
            .into()
    }

    fn view_pane_editor(&self, doc_idx: usize, pane_id: usize) -> Element<'_, Message> {
        if let Some(doc) = self.docs.documents.get(doc_idx) {
            let highlighter_theme = self.highlighter_theme();
            let base = text_editor(&doc.content)
                .font(taim::Font::MONOSPACE)
                .size(Pixels(self.font_size));
            let highlighted = if self.workspace_root.is_some() && self.theme_mode == ThemeMode::Dark
            {
                base.highlight_with::<taim_highlighter::Highlighter>(
                    taim_highlighter::Settings::new(highlighter_theme, &doc.syntax),
                    graphite_highlight,
                )
            } else {
                base.highlight(&doc.syntax, highlighter_theme)
            };
            let editor = highlighted
                .on_action(move |action| {
                    if self.terminal_raw_focus {
                        if matches!(
                            action,
                            text_editor::Action::Click(_)
                                | text_editor::Action::Drag(_)
                                | text_editor::Action::Scroll { .. }
                        ) {
                            Message::EditorInteraction(pane_id, action)
                        } else {
                            Message::TerminalIgnored
                        }
                    } else if pane_id == 0 {
                        Message::EditorAction(action)
                    } else {
                        Message::SplitEditorAction(action)
                    }
                })
                .height(Length::Fill)
                .wrapping(taim_core::text::Wrapping::None)
                .padding(editor_chrome::EDITOR_CONTENT_INSET);

            if self.workspace_root.is_some() {
                let (first_line, vertical_offset) = doc.content.scroll_position();
                let extra_lines = doc.extra_cursor_lines();
                let mut extra_cursor_lines = [0usize; 4];
                let extra_cursor_count = extra_lines.len().min(4) as u8;
                for (i, &l) in extra_lines.iter().take(4).enumerate() {
                    extra_cursor_lines[i] = l;
                }
                let foldable = doc.foldable_blocks();
                let mut foldable_lines = [0usize; 16];
                let foldable_count = foldable.len().min(16) as u8;
                for (i, &(start, _)) in foldable.iter().take(16).enumerate() {
                    foldable_lines[i] = start;
                }
                let mut folded_lines = [0usize; 16];
                let folded_count = doc.folded_ranges.len().min(16) as u8;
                for (i, &(start, _)) in doc.folded_ranges.iter().take(16).enumerate() {
                    folded_lines[i] = start;
                }
                let viewport = editor_chrome::EditorViewport {
                    doc_index: doc_idx,
                    first_line,
                    vertical_offset,
                    line_count: doc.line_count(),
                    cursor_line: doc.content.cursor().position.line,
                    extra_cursor_lines,
                    extra_cursor_count,
                    font_size: self.font_size,
                    light: self.theme_mode == ThemeMode::Light,
                    foldable_lines,
                    foldable_count,
                    folded_lines,
                    folded_count,
                };
                let minimap_density = doc.minimap_density();
                let content = row![
                    editor_chrome::gutter(viewport),
                    container(editor).width(Length::Fill).height(Length::Fill),
                    editor_chrome::minimap(viewport, minimap_density),
                    editor_chrome::scrollbar(viewport),
                ]
                .spacing(0)
                .height(Length::Fill);
                let editor_surface: Element<'_, Message> = container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(container::bordered_box)
                    .into();
                if self.autocomplete.is_open && pane_id == self.docs.focused_pane {
                    let overlay = container(
                        column![
                            space().height(Length::Fixed(34.0)),
                            row![
                                space().width(Length::Fixed(72.0)),
                                self.view_autocomplete_popup(),
                                space().width(Length::Fill),
                            ],
                            space().height(Length::Fill),
                        ]
                        .height(Length::Fill),
                    )
                    .width(Length::Fill)
                    .height(Length::Fill);
                    return stack(vec![editor_surface, overlay.into()]).into();
                }
                return editor_surface;
            }

            // Minimap strip (simulated document density overview)
            let minimap_strip = container(
                column![
                    container(space())
                        .width(Length::Fixed(30.0))
                        .height(Length::Fixed(8.0))
                        .style(container::rounded_box),
                    container(space())
                        .width(Length::Fixed(24.0))
                        .height(Length::Fixed(6.0))
                        .style(container::rounded_box),
                    container(space())
                        .width(Length::Fixed(36.0))
                        .height(Length::Fixed(10.0))
                        .style(container::rounded_box),
                    container(space())
                        .width(Length::Fixed(18.0))
                        .height(Length::Fixed(6.0))
                        .style(container::rounded_box),
                    container(space())
                        .width(Length::Fixed(28.0))
                        .height(Length::Fixed(8.0))
                        .style(container::rounded_box),
                ]
                .spacing(3),
            )
            .width(Length::Fixed(44.0))
            .height(Length::Fill)
            .padding(Padding {
                top: 10.0,
                bottom: 10.0,
                left: 4.0,
                right: 4.0,
            })
            .style(container::rounded_box);

            let editor_with_minimap = row![
                container(editor).width(Length::Fill).height(Length::Fill),
                minimap_strip,
            ]
            .spacing(1)
            .height(Length::Fill);

            container(editor_with_minimap)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(container::bordered_box)
                .into()
        } else {
            container(text("Empty Editor Pane").size(13))
                .center_x(Fill)
                .center_y(Fill)
                .into()
        }
    }

    fn view_autocomplete_popup(&self) -> Element<'_, Message> {
        let filtered = self.autocomplete.filtered();
        let mut items = column![].spacing(1);
        for (index, item) in filtered.iter().enumerate().take(8) {
            items = items.push(
                button(
                    row![
                        text(item.kind.label()).size(9).width(Length::Fixed(54.0)),
                        column![
                            text(&item.label).size(11),
                            text(&item.detail).size(9).style(text::secondary),
                        ]
                        .spacing(1),
                    ]
                    .spacing(6),
                )
                .style(if index == self.autocomplete.selected_index {
                    button::primary
                } else {
                    button::secondary
                })
                .width(Length::Fixed(420.0))
                .on_press(Message::SelectAutocomplete(index)),
            );
        }
        container(items)
            .padding(4)
            .width(Length::Fixed(430.0))
            .style(container::bordered_box)
            .into()
    }

    fn view_command_palette(&self) -> Element<'_, Message> {
        let input_box = text_input("Type file name or > for commands...", &self.palette.query)
            .id("virgo-command-palette")
            .size(Pixels(14.0))
            .padding(Padding {
                top: 8.0,
                bottom: 8.0,
                left: 12.0,
                right: 12.0,
            })
            .width(Length::Fill)
            .on_input(Message::PaletteQueryChanged);

        let filtered = self.palette.filtered();

        let mut items_col = column![].spacing(2);
        for (idx, item) in filtered.iter().enumerate().take(8) {
            let is_selected = idx == self.palette.selected_index;
            let icon = text(item.icon).size(14);
            let title = text(&item.title).size(12);
            let subtitle = text(&item.subtitle).size(10);

            let text_cluster = column![title, subtitle].spacing(1);

            let mut item_row = row![icon, text_cluster].spacing(10).align_y(Center);

            if let Some(sc) = &item.shortcut {
                let sc_badge = container(text(sc).size(9))
                    .padding(Padding {
                        top: 1.0,
                        bottom: 1.0,
                        left: 4.0,
                        right: 4.0,
                    })
                    .style(container::rounded_box);
                item_row = item_row.push(space().width(Length::Fill)).push(sc_badge);
            }

            let item_btn = button(item_row)
                .width(Length::Fill)
                .padding(Padding {
                    top: 6.0,
                    bottom: 6.0,
                    left: 10.0,
                    right: 10.0,
                })
                .style(if is_selected {
                    button::primary
                } else {
                    button::text
                })
                .on_press(Message::PaletteItemChosen(item.id));

            items_col = items_col.push(item_btn);
        }

        let palette_dialog = container(
            column![
                row![
                    text("⚡ QUICK OPEN & COMMAND PALETTE").size(11),
                    space().width(Length::Fill),
                    button(text("Esc to Close").size(10))
                        .style(button::text)
                        .on_press(Message::CloseCommandPalette),
                ]
                .padding(Padding {
                    top: 8.0,
                    bottom: 4.0,
                    left: 12.0,
                    right: 12.0
                })
                .align_y(Center),
                rule::horizontal(1),
                container(input_box).padding(Padding {
                    top: 6.0,
                    bottom: 6.0,
                    left: 10.0,
                    right: 10.0
                }),
                rule::horizontal(1),
                container(scrollable(items_col).height(Length::Fixed(240.0))).padding(Padding {
                    top: 4.0,
                    bottom: 4.0,
                    left: 6.0,
                    right: 6.0
                }),
            ]
            .spacing(2),
        )
        .width(Length::Fixed(560.0))
        .style(container::bordered_box);

        container(
            column![
                space().height(Length::Fixed(40.0)),
                palette_dialog,
                space().height(Length::Fill),
            ]
            .align_x(Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// Bottom-right floating toast notifications stack.
    fn view_toast_notifications(&self) -> Element<'_, Message> {
        let mut toasts_col = column![].spacing(6);

        for toast in &self.notifications.active_toasts {
            let icon = text(toast.severity.icon()).size(12);
            let title = text(&toast.title).size(11);
            let msg = text(&toast.message).size(10);
            let close_btn = button(text("✕").size(10))
                .style(button::text)
                .on_press(Message::DismissNotification(toast.id));

            let mut header_row = row![icon, title, space().width(Length::Fill), close_btn]
                .spacing(6)
                .align_y(Center);

            if let Some(action_label) = &toast.action_label {
                let action_btn = button(text(action_label).size(9))
                    .style(button::success)
                    .padding(Padding {
                        top: 1.0,
                        bottom: 1.0,
                        left: 4.0,
                        right: 4.0,
                    });
                header_row = header_row.push(action_btn);
            }

            let toast_box = container(column![header_row, msg].spacing(2))
                .padding(8)
                .width(Length::Fixed(320.0))
                .style(container::bordered_box);

            toasts_col = toasts_col.push(toast_box);
        }

        container(
            row![
                space().width(Length::Fill),
                column![
                    space().height(Length::Fill),
                    toasts_col,
                    space().height(Length::Fixed(30.0)),
                ],
                space().width(Length::Fixed(16.0)),
            ]
            .align_y(Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_workspace_panel(&self) -> Element<'_, Message> {
        let header = row![
            button(
                row![
                    svg(icons::terminal())
                        .width(Length::Fixed(13.0))
                        .height(Length::Fixed(13.0)),
                    text("Terminal").size(11)
                ]
                .spacing(5)
                .align_y(Center)
            )
            .style(move |_, status| graphite_button_style(
                self.terminal.active_tab == PanelTab::Terminal,
                status
            ))
            .on_press(Message::TerminalTabSelected(PanelTab::Terminal)),
            button(
                row![
                    svg(icons::build())
                        .width(Length::Fixed(13.0))
                        .height(Length::Fixed(13.0)),
                    text("Build").size(11)
                ]
                .spacing(5)
                .align_y(Center)
            )
            .style(move |_, status| graphite_button_style(
                self.terminal.active_tab == PanelTab::BuildConsole,
                status
            ))
            .on_press(Message::TerminalTabSelected(PanelTab::BuildConsole)),
            button(text(format!("Problems ({})", self.terminal.diagnostics.len())).size(11))
                .style(move |_, status| graphite_button_style(
                    self.terminal.active_tab == PanelTab::Diagnostics,
                    status
                ))
                .on_press(Message::TerminalTabSelected(PanelTab::Diagnostics)),
            space().width(Length::Fill),
            button(
                row![
                    svg(icons::plus())
                        .width(Length::Fixed(12.0))
                        .height(Length::Fixed(12.0)),
                    text("Shell").size(10)
                ]
                .spacing(3)
                .align_y(Center)
            )
            .style(button::text)
            .on_press(Message::NewTerminalTab),
            button(
                text(if self.terminal_raw_focus {
                    "Editor focus"
                } else {
                    "Terminal focus"
                })
                .size(10)
            )
            .style(button::text)
            .on_press(Message::TerminalFocusChanged(!self.terminal_raw_focus)),
            button(
                text(if self.terminal_expanded {
                    "Restore"
                } else {
                    "Maximize"
                })
                .size(10)
            )
            .style(button::text)
            .on_press(Message::ToggleTerminalMaximize),
            button(text("Check").size(10))
                .style(button::text)
                .on_press(Message::TriggerCheck),
            button(text("Build").size(10))
                .style(button::text)
                .on_press(Message::TriggerBuild),
            button(text("⌃C").size(10))
                .style(button::text)
                .on_press(Message::TerminalInterrupt),
            button(text("Clear").size(10))
                .style(button::text)
                .on_press(Message::ClearTerminal),
            button(text("×").size(13))
                .style(button::text)
                .on_press(Message::ToggleTerminal),
        ]
        .spacing(6)
        .align_y(Center);

        let body: Element<'_, Message> = if self.terminal.active_tab == PanelTab::Terminal {
            let mut tabs = row![].spacing(4).align_y(Center);
            for (idx, instance) in self.terminal.terminals.iter().enumerate() {
                tabs = tabs.push(
                    button(
                        text(format!(
                            "{}  {}",
                            if instance.running { "●" } else { "○" },
                            instance.name
                        ))
                        .size(10),
                    )
                    .style(move |_, status| {
                        graphite_button_style(idx == self.terminal.active_terminal_idx, status)
                    })
                    .on_press(Message::SelectTerminalInstance(idx)),
                );
                tabs = tabs.push(
                    button(text("×").size(10))
                        .style(button::text)
                        .on_press(Message::CloseTerminalTab(idx)),
                );
            }
            let mut output = column![].spacing(1);
            if let Some(instance) = self
                .terminal
                .terminals
                .get(self.terminal.active_terminal_idx)
            {
                let cursor = self
                    .terminal_sessions
                    .get(&instance.id)
                    .and_then(|session| session.cursor());
                for (row_index, line) in instance.lines.iter().enumerate() {
                    let displayed = if self.terminal_raw_focus
                        && instance.running
                        && cursor.is_some_and(|(row, _)| row == row_index)
                    {
                        let mut chars: Vec<char> = line.chars().collect();
                        let column = cursor
                            .unwrap()
                            .1
                            .min(terminal::TERMINAL_COLUMNS.saturating_sub(1));
                        while chars.len() < column {
                            chars.push(' ');
                        }
                        if column == chars.len() {
                            chars.push('▌');
                        } else {
                            chars[column] = '▌';
                        }
                        chars.into_iter().collect::<String>()
                    } else {
                        line.clone()
                    };
                    output = output.push(
                        text(if displayed.is_empty() {
                            " ".to_string()
                        } else {
                            displayed
                        })
                        .size(11)
                        .font(taim::Font::MONOSPACE),
                    );
                }
                let hint = if !instance.running {
                    "Shell exited. Open a new terminal to continue."
                } else if self.terminal_raw_focus {
                    "Keyboard → terminal  ·  Click Editor focus to return"
                } else {
                    "Click terminal to type directly"
                };
                let height = if self.terminal_expanded { 355.0 } else { 124.0 };
                column![
                    tabs,
                    mouse_area(
                        container(output)
                            .height(Length::Fixed(height))
                            .width(Length::Fill)
                    )
                    .on_press(Message::TerminalFocusChanged(true))
                    .on_scroll(|delta| {
                        let amount = match delta {
                            taim::mouse::ScrollDelta::Lines { y, .. } => y as i32,
                            taim::mouse::ScrollDelta::Pixels { y, .. } => (y / 16.0) as i32,
                        };
                        Message::TerminalScroll(amount.clamp(-12, 12))
                    }),
                    text(hint).size(10).style(text::secondary)
                ]
                .spacing(4)
                .into()
            } else {
                column![
                    tabs,
                    container(text("Open a shell to run commands in this workspace.").size(11))
                        .height(Length::Fixed(125.0)),
                    button(
                        row![
                            svg(icons::plus())
                                .width(Length::Fixed(13.0))
                                .height(Length::Fixed(13.0)),
                            text("New Terminal").size(11)
                        ]
                        .spacing(5)
                        .align_y(Center)
                    )
                    .style(|_, status| graphite_button_style(true, status))
                    .on_press(Message::NewTerminalTab)
                ]
                .spacing(5)
                .into()
            }
        } else if self.terminal.active_tab == PanelTab::Diagnostics {
            let mut problems = column![].spacing(2);
            if self.terminal.diagnostics.is_empty() {
                problems = problems.push(
                    text("No language-server diagnostics.")
                        .size(11)
                        .style(text::secondary),
                );
            }
            for diagnostic in &self.terminal.diagnostics {
                problems = problems.push(
                    button(
                        row![
                            text(match diagnostic.severity {
                                LogLevel::Error => "Error",
                                LogLevel::Warning => "Warning",
                                _ => "Info",
                            })
                            .size(10),
                            text(format!(
                                "{}:{}:{}  {}",
                                diagnostic.file,
                                diagnostic.line,
                                diagnostic.col,
                                diagnostic.message
                            ))
                            .size(10)
                            .font(taim::Font::MONOSPACE),
                        ]
                        .spacing(8),
                    )
                    .style(button::text)
                    .width(Length::Fill)
                    .on_press(Message::SearchHitSelected(
                        diagnostic.file.clone(),
                        diagnostic.line,
                        diagnostic.col,
                    )),
                );
            }
            scrollable(problems).height(Length::Fixed(155.0)).into()
        } else {
            let mut output = column![].spacing(2);
            if self.terminal.build_logs.is_empty() {
                output = output.push(
                    text("Run Check or Build to see task output.")
                        .size(11)
                        .style(text::secondary),
                );
            }
            for entry in self.terminal.build_logs.iter().rev().take(120).rev() {
                output = output.push(text(&entry.message).size(11).font(taim::Font::MONOSPACE));
            }
            scrollable(output)
                .height(Length::Fixed(155.0))
                .anchor_bottom()
                .into()
        };
        container(
            column![header, rule::horizontal(1), body]
                .spacing(6)
                .padding(Padding {
                    top: 6.0,
                    bottom: 6.0,
                    left: 10.0,
                    right: 10.0,
                }),
        )
        .style(container::bordered_box)
        .width(Length::Fill)
        .into()
    }

    fn view_terminal(&self) -> Element<'_, Message> {
        if self.workspace_root.is_some() {
            return self.view_workspace_panel();
        }
        let tabs_header = row![
            button(
                text(format!(
                    "{} {}",
                    PanelTab::Terminal.glyph(),
                    PanelTab::Terminal.label()
                ))
                .size(11)
            )
            .style(if self.terminal.active_tab == PanelTab::Terminal {
                button::primary
            } else {
                button::text
            })
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 6.0,
                right: 6.0
            })
            .on_press(Message::TerminalTabSelected(PanelTab::Terminal)),
            button(
                text(format!(
                    "{} {}",
                    PanelTab::BuildConsole.glyph(),
                    PanelTab::BuildConsole.label()
                ))
                .size(11)
            )
            .style(if self.terminal.active_tab == PanelTab::BuildConsole {
                button::primary
            } else {
                button::text
            })
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 6.0,
                right: 6.0
            })
            .on_press(Message::TerminalTabSelected(PanelTab::BuildConsole)),
            button(
                text(format!(
                    "{} {}",
                    PanelTab::Output.glyph(),
                    PanelTab::Output.label()
                ))
                .size(11)
            )
            .style(if self.terminal.active_tab == PanelTab::Output {
                button::primary
            } else {
                button::text
            })
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 6.0,
                right: 6.0
            })
            .on_press(Message::TerminalTabSelected(PanelTab::Output)),
            button(
                text(format!(
                    "{} {}",
                    PanelTab::DebugConsole.glyph(),
                    PanelTab::DebugConsole.label()
                ))
                .size(11)
            )
            .style(if self.terminal.active_tab == PanelTab::DebugConsole {
                button::primary
            } else {
                button::text
            })
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 6.0,
                right: 6.0
            })
            .on_press(Message::TerminalTabSelected(PanelTab::DebugConsole)),
            button(
                text(format!(
                    "{} {}",
                    PanelTab::Diagnostics.glyph(),
                    PanelTab::Diagnostics.label()
                ))
                .size(11)
            )
            .style(if self.terminal.active_tab == PanelTab::Diagnostics {
                button::primary
            } else {
                button::text
            })
            .padding(Padding {
                top: 2.0,
                bottom: 2.0,
                left: 6.0,
                right: 6.0
            })
            .on_press(Message::TerminalTabSelected(PanelTab::Diagnostics)),
            if self.workspace_root.is_some() {
                Element::from(container(space()).width(Pixels(0.0)).height(Pixels(0.0)))
            } else {
                button(
                    text(format!(
                        "{} {}",
                        PanelTab::Benchmark.glyph(),
                        PanelTab::Benchmark.label()
                    ))
                    .size(11),
                )
                .style(if self.terminal.active_tab == PanelTab::Benchmark {
                    button::primary
                } else {
                    button::text
                })
                .padding(Padding {
                    top: 2.0,
                    bottom: 2.0,
                    left: 6.0,
                    right: 6.0,
                })
                .on_press(Message::TerminalTabSelected(PanelTab::Benchmark))
                .into()
            },
            space().width(Length::Fill),
            button(text("⎚ Clear").size(10))
                .style(button::text)
                .padding(Padding {
                    top: 1.0,
                    bottom: 1.0,
                    left: 5.0,
                    right: 5.0
                })
                .on_press(Message::ClearTerminal),
            button(text("✕").size(10))
                .style(button::text)
                .padding(Padding {
                    top: 1.0,
                    bottom: 1.0,
                    left: 5.0,
                    right: 5.0
                })
                .on_press(Message::ToggleTerminal),
        ]
        .spacing(3)
        .align_y(Center);

        let content: Element<'_, Message> = match self.terminal.active_tab {
            PanelTab::Terminal => {
                let mut term_tabs = row![].spacing(3).align_y(Center);
                for (idx, inst) in self.terminal.terminals.iter().enumerate() {
                    let is_sel = idx == self.terminal.active_terminal_idx;
                    let tab_btn = button(text(&inst.name).size(10))
                        .style(if is_sel {
                            button::secondary
                        } else {
                            button::text
                        })
                        .padding(Padding {
                            top: 1.0,
                            bottom: 1.0,
                            left: 5.0,
                            right: 5.0,
                        })
                        .on_press(Message::SelectTerminalInstance(idx));
                    term_tabs = term_tabs.push(tab_btn);
                }
                let add_term = button(text("＋").size(10))
                    .style(button::text)
                    .on_press(Message::NewTerminalTab);
                term_tabs = term_tabs.push(add_term);

                let mut log_col = column![term_tabs, rule::horizontal(1)].spacing(2);
                for entry in &self.terminal.logs {
                    let badge = container(text(entry.level.badge()).size(9))
                        .padding(Padding {
                            top: 1.0,
                            bottom: 1.0,
                            left: 4.0,
                            right: 4.0,
                        })
                        .style(match entry.level {
                            LogLevel::Info => container::primary,
                            LogLevel::Success => container::success,
                            LogLevel::Warning => container::secondary,
                            LogLevel::Error => container::dark,
                        });
                    let line = row![
                        text(&entry.time).size(11).font(taim::Font::MONOSPACE),
                        badge,
                        text(&entry.message).size(11).font(taim::Font::MONOSPACE),
                    ]
                    .spacing(6)
                    .align_y(Center);
                    log_col = log_col.push(line);
                }
                scrollable(log_col).into()
            }
            PanelTab::BuildConsole => {
                let mut build_col = column![].spacing(2);
                for entry in &self.terminal.build_logs {
                    let loc = parse_compiler_location(&entry.message);
                    let line = if let Some((file, l, c)) = loc {
                        row![
                            text(&entry.time).size(11).font(taim::Font::MONOSPACE),
                            button(text(&entry.message).size(11).font(taim::Font::MONOSPACE))
                                .style(button::text)
                                .padding(0)
                                .on_press(Message::NavigateToLocation(file, l, c)),
                        ]
                        .spacing(6)
                        .align_y(Center)
                    } else {
                        row![
                            text(&entry.time).size(11).font(taim::Font::MONOSPACE),
                            text(&entry.message).size(11).font(taim::Font::MONOSPACE),
                        ]
                        .spacing(6)
                        .align_y(Center)
                    };
                    build_col = build_col.push(line);
                }
                scrollable(build_col).into()
            }
            PanelTab::Output => {
                let channel_selector = row![
                    text("Channel:").size(10),
                    button(text(self.terminal.active_channel.label()).size(10))
                        .style(button::secondary)
                        .padding(Padding {
                            top: 1.0,
                            bottom: 1.0,
                            left: 6.0,
                            right: 6.0
                        })
                        .on_press(Message::SelectOutputChannel(OutputChannel::RustAnalyzer)),
                ]
                .spacing(4)
                .align_y(Center);

                let mut out_col = column![channel_selector, rule::horizontal(1)].spacing(2);
                for entry in &self.terminal.output_logs {
                    let loc = parse_compiler_location(&entry.message);
                    let line = if let Some((file, l, c)) = loc {
                        row![
                            text(&entry.time).size(10).font(taim::Font::MONOSPACE),
                            button(text(&entry.message).size(10).font(taim::Font::MONOSPACE))
                                .style(button::text)
                                .padding(0)
                                .on_press(Message::NavigateToLocation(file, l, c)),
                        ]
                        .spacing(6)
                    } else {
                        row![
                            text(&entry.time).size(10).font(taim::Font::MONOSPACE),
                            text(&entry.message).size(10).font(taim::Font::MONOSPACE),
                        ]
                        .spacing(6)
                    };
                    out_col = out_col.push(line);
                }
                scrollable(out_col).into()
            }
            PanelTab::DebugConsole => {
                let mut dbg_col = column![].spacing(2);
                for entry in &self.terminal.debug_logs {
                    let line = row![
                        text(&entry.time).size(10).font(taim::Font::MONOSPACE),
                        text(&entry.message).size(10).font(taim::Font::MONOSPACE),
                    ]
                    .spacing(6);
                    dbg_col = dbg_col.push(line);
                }

                let repl_bar = row![
                    text("debug>").size(11).font(taim::Font::MONOSPACE),
                    text_input(
                        "Type expression (e.g. taim::version())...",
                        &self.terminal.repl_input
                    )
                    .size(Pixels(11.0))
                    .padding(Padding {
                        top: 2.0,
                        bottom: 2.0,
                        left: 6.0,
                        right: 6.0
                    })
                    .width(Length::Fill)
                    .on_input(Message::DebugReplInputChanged)
                    .on_submit(Message::DebugReplSubmit),
                    button(text("Eval").size(10))
                        .style(button::primary)
                        .padding(Padding {
                            top: 2.0,
                            bottom: 2.0,
                            left: 6.0,
                            right: 6.0
                        })
                        .on_press(Message::DebugReplSubmit),
                ]
                .spacing(6)
                .align_y(Center);

                column![
                    scrollable(dbg_col).height(Length::Fill),
                    rule::horizontal(1),
                    repl_bar
                ]
                .into()
            }
            PanelTab::Diagnostics => {
                let mut diag_col = column![].spacing(3);
                for (idx, diag) in self.terminal.diagnostics.iter().enumerate() {
                    let badge = container(text(diag.severity.badge()).size(9))
                        .padding(Padding {
                            top: 1.0,
                            bottom: 1.0,
                            left: 4.0,
                            right: 4.0,
                        })
                        .style(container::secondary);
                    let loc_btn = button(
                        text(format!("{}:{}:{}", diag.file, diag.line, diag.col))
                            .size(11)
                            .font(taim::Font::MONOSPACE),
                    )
                    .style(button::text)
                    .padding(Padding {
                        top: 0.0,
                        bottom: 0.0,
                        left: 2.0,
                        right: 2.0,
                    })
                    .on_press(Message::NavigateToLocation(
                        diag.file.clone(),
                        diag.line,
                        diag.col,
                    ));
                    let msg = text(&diag.message).size(11);

                    let mut line_row = row![badge, loc_btn, msg].spacing(6).align_y(Center);

                    if let Some(fix) = &diag.suggested_fix {
                        let fix_btn = button(text(format!("Quick Fix: {}", fix)).size(10))
                            .style(button::success)
                            .padding(Padding {
                                top: 1.0,
                                bottom: 1.0,
                                left: 6.0,
                                right: 6.0,
                            })
                            .on_press(Message::ApplyDiagnosticFix(idx));
                        line_row = line_row.push(space().width(Length::Fill)).push(fix_btn);
                    }

                    diag_col = diag_col.push(line_row);
                }
                scrollable(diag_col).into()
            }
            PanelTab::Benchmark => {
                let mut bench_col = column![
                    row![
                        text("METRIC").size(10).width(Length::Fixed(180.0)),
                        text("TAIM STUDIO (GPU)")
                            .size(10)
                            .width(Length::Fixed(180.0)),
                        text("VS CODE (ELECTRON)")
                            .size(10)
                            .width(Length::Fixed(220.0)),
                        text("TAIM ADVANTAGE").size(10).width(Length::Fill),
                    ]
                    .spacing(10)
                    .padding(Padding {
                        top: 2.0,
                        bottom: 2.0,
                        left: 4.0,
                        right: 4.0
                    }),
                    rule::horizontal(1),
                ]
                .spacing(3);

                for row in &BENCHMARK_DATA {
                    let metric = text(row.metric)
                        .size(11)
                        .font(taim::Font::MONOSPACE)
                        .width(Length::Fixed(180.0));
                    let taim_v = container(text(row.taim_val).size(11))
                        .padding(Padding {
                            top: 1.0,
                            bottom: 1.0,
                            left: 6.0,
                            right: 6.0,
                        })
                        .style(container::rounded_box)
                        .width(Length::Fixed(180.0));
                    let vs_v = text(row.vscode_val).size(11).width(Length::Fixed(220.0));
                    let adv = container(text(row.advantage).size(10))
                        .padding(Padding {
                            top: 1.0,
                            bottom: 1.0,
                            left: 6.0,
                            right: 6.0,
                        })
                        .style(container::success);

                    let bench_row = row![metric, taim_v, vs_v, adv].spacing(10).align_y(Center);
                    bench_col = bench_col.push(bench_row);
                }

                scrollable(bench_col).into()
            }
        };

        container(
            column![
                tabs_header,
                rule::horizontal(1),
                container(content)
                    .height(Length::Fixed(135.0))
                    .padding(Padding {
                        top: 3.0,
                        bottom: 3.0,
                        left: 6.0,
                        right: 6.0
                    }),
            ]
            .spacing(2),
        )
        .style(container::bordered_box)
        .width(Length::Fill)
        .into()
    }

    fn view_workspace_status_bar(&self) -> Element<'_, Message> {
        let doc = self.docs.active();
        let (line, col) = doc.map(|d| d.cursor_position()).unwrap_or((1, 1));
        let name = doc.map(|d| d.path.as_str()).unwrap_or("No file open");
        let language = doc.map(|d| d.syntax.as_str()).unwrap_or("Plain Text");
        let dirty_count = self.docs.documents.iter().filter(|d| d.is_dirty).count();
        let save_state = if dirty_count == 0 {
            "Saved".to_string()
        } else {
            format!("{} unsaved", dirty_count)
        };
        let mut row_items = row![
            button(
                text(format!(
                    "⑂ {}  {}",
                    self.git.active_branch,
                    self.git.total_changes()
                ))
                .size(10)
            )
            .style(button::text)
            .padding(0)
            .on_press(Message::RevealSidebarView(SidebarView::SourceControl)),
            text("·").size(10).style(text::secondary),
            text(name).size(10),
            text("·").size(10).style(text::secondary),
            text(save_state).size(10).style(text::secondary),
        ];

        if let Some(blame) = &self.active_line_blame {
            row_items = row_items
                .push(text("·").size(10).style(text::secondary))
                .push(text(format!("⎇ {}", blame)).size(10).style(text::secondary));
        }

        row_items = row_items
            .push(space().width(Length::Fill))
            .push(text(format!("Ln {}, Col {}", line, col)).size(10))
            .push(text("·").size(10).style(text::secondary))
            .push(text(language.to_uppercase()).size(10))
            .push(text("·").size(10).style(text::secondary))
            .push(text("UTF-8").size(10));

        container(
            row_items
                .spacing(8)
                .padding(Padding {
                    top: 5.0,
                    bottom: 5.0,
                    left: 12.0,
                    right: 12.0,
                })
                .align_y(Center),
        )
        .style(container::bordered_box)
        .width(Length::Fill)
        .into()
    }

    fn view_status_bar(&self) -> Element<'_, Message> {
        if self.workspace_root.is_some() {
            return self.view_workspace_status_bar();
        }
        let (line, col) = self
            .docs
            .active()
            .map(|d| d.cursor_position())
            .unwrap_or((1, 1));
        let line_count = self.docs.active().map(|d| d.line_count()).unwrap_or(0);
        let path = self
            .docs
            .active()
            .map(|d| d.path.as_str())
            .unwrap_or("No file");
        let syntax = self
            .docs
            .active()
            .map(|d| d.syntax.as_str())
            .unwrap_or("plain");
        let dirty = self.docs.active().map(|d| d.is_dirty).unwrap_or(false);

        let left = row![
            button(row![text("⑂").size(10), text(&self.git.active_branch).size(10)].spacing(4))
                .style(button::text)
                .padding(0)
                .on_press(Message::SelectSidebarView(SidebarView::SourceControl)),
            text("⇄ 0↓ 0↑").size(10),
            text("|").size(10),
            text(format!("▲ 0  ⚠ {}", self.terminal.diagnostics.len())).size(10),
            text("|").size(10),
            text(format!("{}{}", path, if dirty { " *" } else { "" })).size(10),
            if self.docs.split_tab.is_some() {
                container(text(format!("Split: Pane {}", self.docs.focused_pane + 1)).size(9))
                    .padding(Padding {
                        top: 1.0,
                        bottom: 1.0,
                        left: 4.0,
                        right: 4.0,
                    })
                    .style(container::secondary)
            } else {
                container(space()).width(Pixels(0.0)).height(Pixels(0.0))
            },
        ]
        .spacing(6)
        .align_y(Center);

        let center = row![
            text(if self.workspace_root.is_some() {
                "Virgo • Taim"
            } else {
                "Taim Studio demo"
            })
            .size(10),
        ]
        .align_y(Center);

        let cursor_label = if let Some(doc) = self.docs.active() {
            let extra = if doc.has_extra_cursors() {
                format!(" ({} cursors)", doc.extra_cursors.len() + 1)
            } else {
                String::new()
            };
            let backing = match doc.backing {
                DocumentBacking::Standard => "",
                DocumentBacking::MemoryMapped { is_partial, .. } => {
                    if is_partial {
                        " [MMap Stream]"
                    } else {
                        " [MMap]"
                    }
                }
            };
            format!("Ln {}, Col {}{} ({} lines){}", line, col, extra, line_count, backing)
        } else {
            format!("Ln {}, Col {} ({} lines)", line, col, line_count)
        };

        let right = row![
            text(cursor_label).size(10),
            text("|").size(10),
            text("UTF-8").size(10),
            text("|").size(10),
            text("Spaces: 4").size(10),
            text("|").size(10),
            container(text(syntax.to_uppercase()).size(9))
                .padding(Padding {
                    top: 1.0,
                    bottom: 1.0,
                    left: 4.0,
                    right: 4.0
                })
                .style(container::rounded_box),
            text("|").size(10),
            text(match self.language_server_status {
                lsp::ServerStatus::Unavailable => "LSP: unavailable",
                lsp::ServerStatus::Starting => "LSP: starting",
                lsp::ServerStatus::Ready => "LSP: ready",
                lsp::ServerStatus::Failed => "LSP: failed",
                lsp::ServerStatus::Stopped => "LSP: stopped",
            })
            .size(10),
            text("|").size(10),
            button(
                row![
                    text("⌁").size(10),
                    text(format!("{}", self.notifications.count())).size(9)
                ]
                .spacing(2)
            )
            .style(button::text)
            .padding(0)
            .on_press(Message::ToggleNotificationCenter),
        ]
        .spacing(6)
        .align_y(Center);

        let bar = row![
            left,
            space().width(Length::Fill),
            center,
            space().width(Length::Fill),
            right,
        ]
        .padding(Padding {
            top: 2.0,
            bottom: 2.0,
            left: 8.0,
            right: 8.0,
        })
        .align_y(Center);

        container(bar)
            .style(container::rounded_box)
            .width(Length::Fill)
            .into()
    }
}

impl Drop for Studio {
    fn drop(&mut self) {
        let _ = self.persist_recovery_snapshot();
    }
}

/// Runs the Taim Studio application.
pub fn run() -> taim::Result {
    taim::application(Studio::new, Studio::update, Studio::view)
        .title("Taim Studio — IDE Engine (120 FPS Native)")
        .theme(Studio::theme)
        .subscription(Studio::subscription)
        .run()
}

/// Launches Virgo on a real folder. The executable uses this entry point.
pub fn run_with_workspace(root: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()).into());
    }
    let backend = std::env::var("TAIM_BACKEND")
        .map(taim::Backend::from)
        .unwrap_or_else(|_| {
            #[cfg(target_os = "macos")]
            {
                taim::Backend::Custom("tiny-skia".to_string())
            }
            #[cfg(not(target_os = "macos"))]
            {
                taim::Backend::Best
            }
        });
    Ok(taim::application(
        move || {
            let (mut studio, startup) =
                Studio::with_workspace(root.clone()).expect("cannot load workspace");
            let language_server = studio.start_language_server();
            (studio, Task::batch([startup, language_server]))
        },
        Studio::update,
        Studio::view,
    )
    .title("Virgo IDE")
    .backend(backend)
    .theme(Studio::theme)
    .subscription(Studio::subscription)
    .run()?)
}

static SAVE_ID: AtomicU64 = AtomicU64::new(1);

fn graphite_button_style(active: bool, status: button::Status) -> button::Style {
    let background = if status == button::Status::Hovered {
        0x3B3B3B
    } else if active {
        0x343434
    } else {
        0x242424
    };
    button::Style {
        background: Some(Color::from_packed_rgba8(background, 1.0).into()),
        text_color: Color::from_packed_rgba8(if active { 0xEFEFEF } else { 0xC8C8C8 }, 1.0),
        ..button::Style::default()
    }
}

fn graphite_highlight(
    highlight: &taim_highlighter::Highlight,
    _theme: &Theme,
) -> taim::advanced::text::highlighter::Format<taim::Font> {
    use taim_highlighter::Scope;
    let shade = match highlight.scope() {
        Scope::Comment => 0x8C,
        Scope::String => 0xC9,
        Scope::Number => 0xC0,
        Scope::Keyword => 0xF0,
        Scope::Type => 0xDC,
        Scope::Function => 0xDE,
        Scope::Attribute => 0xB7,
        Scope::Punctuation => 0xA9,
        Scope::Text => 0xD8,
    };
    taim::advanced::text::highlighter::Format {
        color: Some(Color::from_rgb8(shade, shade, shade)),
        font: highlight.font(),
    }
}

fn write_atomic(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("file has no parent directory"))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("file has no name"))?;
    let temporary = parent.join(format!(
        ".{}.virgo-{}-{}.tmp",
        file_name.to_string_lossy(),
        std::process::id(),
        SAVE_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(content)?;
        file.set_permissions(std::fs::metadata(path)?.permissions())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn path_matches_entry(path: &str, entry: &str) -> bool {
    path == entry
        || path
            .strip_prefix(entry)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn trash_manifest_path(trash: &Path, stored_name: &str) -> PathBuf {
    trash.join(format!("{stored_name}.virgo-trash.json"))
}

fn replacement_preview_line(change: &ReplaceFileChange) -> String {
    let (before, after) = change
        .before
        .lines()
        .zip(change.after.lines())
        .find(|(before, after)| before != after)
        .unwrap_or(("", ""));
    let compact = |line: &str| {
        let mut value: String = line.trim().chars().take(52).collect();
        if line.trim().chars().count() > 52 {
            value.push('…');
        }
        value
    };
    format!("{}  →  {}", compact(before), compact(after))
}

fn workspace_fingerprint(root: &Path, tree: &FileTree) -> u64 {
    fn hash_node(buf: &mut PathBuf, node: &FileNode, state: &mut impl Hasher) {
        node.path.hash(state);
        let pushed = if !node.path.is_empty() {
            buf.push(&node.path);
            true
        } else {
            false
        };
        match std::fs::symlink_metadata(&buf) {
            Ok(metadata) => {
                metadata.len().hash(state);
                metadata.is_dir().hash(state);
                metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_nanos())
                    .hash(state);
            }
            Err(error) => error.kind().hash(state),
        }
        if pushed {
            buf.pop();
        }
        for child in &node.children {
            hash_node(buf, child, state);
        }
    }
    let mut state = std::collections::hash_map::DefaultHasher::new();
    let mut buf = PathBuf::from(root);
    buf.reserve(256);
    hash_node(&mut buf, &tree.root, &mut state);
    buf.push(".virgo-trash");
    if let Ok(metadata) = std::fs::symlink_metadata(&buf) {
        metadata.len().hash(&mut state);
        metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .hash(&mut state);
    }
    state.finish()
}

fn write_new_atomic(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("file has no parent directory"))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("file has no name"))?;
    let temporary = parent.join(format!(
        ".{}.virgo-{}-{}.tmp",
        file_name.to_string_lossy(),
        std::process::id(),
        SAVE_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(content)?;
        file.sync_all()?;
        drop(file);
        std::fs::hard_link(&temporary, path)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&temporary);
    result
}
