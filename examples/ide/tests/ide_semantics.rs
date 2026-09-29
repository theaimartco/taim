//! Headless semantic and state verification tests for Taim Studio.

use taim::widget::text_editor;
use taim_ide::terminal::PanelTab;
use taim_ide::{Message, Studio, ThemeMode};

#[test]
fn test_studio_initialization() {
    let (studio, _task) = Studio::new();

    // Verify initial layout flags
    assert!(studio.show_sidebar);
    assert!(studio.show_terminal);
    assert_eq!(studio.theme_mode, ThemeMode::Dark);
    assert_eq!(studio.font_size, 13.0);

    // Verify default open documents
    assert_eq!(studio.docs.documents.len(), 4);
    assert_eq!(studio.docs.active_tab, 0);
    assert_eq!(studio.docs.active().unwrap().name, "main.rs");
    assert_eq!(studio.docs.active().unwrap().syntax, "rust");

    // Verify initial file tree
    assert_eq!(studio.tree.root.name, "taim-project");
    assert!(studio.tree.root.is_expanded);
    assert_eq!(studio.tree.selected_id, Some(10)); // main.rs
}

#[test]
fn test_theme_switching() {
    let (mut studio, _task) = Studio::new();

    // Switch to Light
    let _ = studio.update(Message::ThemeSelected(ThemeMode::Light));
    assert_eq!(studio.theme_mode, ThemeMode::Light);
    assert_eq!(studio.theme(), taim::Theme::Light);
    assert_eq!(studio.highlighter_theme(), taim_highlighter::Theme::Light);

    // Switch to High Contrast
    let _ = studio.update(Message::ThemeSelected(ThemeMode::HighContrast));
    assert_eq!(studio.theme_mode, ThemeMode::HighContrast);
    assert_eq!(studio.theme(), taim::Theme::HighContrast);
    assert_eq!(
        studio.highlighter_theme(),
        taim_highlighter::Theme::HighContrast
    );

    // Switch back to Dark
    let _ = studio.update(Message::ThemeSelected(ThemeMode::Dark));
    assert_eq!(studio.theme_mode, ThemeMode::Dark);
    assert_eq!(studio.theme(), taim::Theme::Dark);
    assert_eq!(studio.highlighter_theme(), taim_highlighter::Theme::Dark);
}

#[test]
fn test_file_tree_toggle_and_selection() {
    let (mut studio, _task) = Studio::new();

    // Check tests folder is initially collapsed (id: 300)
    let tests_node = studio.tree.find(300).unwrap();
    assert!(!tests_node.is_expanded);

    // Toggle expansion
    let _ = studio.update(Message::TreeNodeToggled(300));
    let tests_node = studio.tree.find(300).unwrap();
    assert!(tests_node.is_expanded);

    // Select pipeline_test.rs (id: 30)
    let _ = studio.update(Message::FileSelected(30));
    assert_eq!(studio.docs.active().unwrap().path, "tests/pipeline_test.rs");
    assert_eq!(studio.docs.active().unwrap().syntax, "rust");
    assert_eq!(studio.tree.selected_id, Some(30));
}

#[test]
fn test_tab_management_and_scratchpad() {
    let (mut studio, _task) = Studio::new();
    let initial_count = studio.docs.documents.len();

    // Create scratchpad
    let _ = studio.update(Message::NewScratchpad);
    assert_eq!(studio.docs.documents.len(), initial_count + 1);
    assert_eq!(studio.docs.active_tab, initial_count);
    assert!(studio.docs.active().unwrap().name.starts_with("scratch_"));

    // Switch tab back to 0
    let _ = studio.update(Message::TabSelected(0));
    assert_eq!(studio.docs.active_tab, 0);
    assert_eq!(studio.docs.active().unwrap().name, "main.rs");

    // Close tab 0
    let _ = studio.update(Message::TabClosed(0));
    assert_eq!(studio.docs.documents.len(), initial_count);
    assert_eq!(studio.docs.active().unwrap().name, "Cargo.toml");
}

#[test]
fn test_editor_actions_and_dirty_flag() {
    let (mut studio, _task) = Studio::new();

    // Active document is main.rs, initially not dirty
    assert!(!studio.docs.active().unwrap().is_dirty);
    let (initial_line, initial_col) = studio.docs.active().unwrap().cursor_position();
    assert_eq!(initial_line, 1);
    assert_eq!(initial_col, 1);

    // Perform an edit action
    let _ = studio.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('X'),
    )));

    // Verify document marked dirty
    assert!(studio.docs.active().unwrap().is_dirty);
}

#[test]
fn test_zoom_controls() {
    let (mut studio, _task) = Studio::new();
    assert_eq!(studio.font_size, 13.0);

    let _ = studio.update(Message::ZoomIn);
    assert_eq!(studio.font_size, 14.0);

    let _ = studio.update(Message::ZoomOut);
    assert_eq!(studio.font_size, 13.0);

    let _ = studio.update(Message::ZoomReset);
    assert_eq!(studio.font_size, 13.0);
}

#[test]
fn test_terminal_and_build_pipeline() {
    let (mut studio, _task) = Studio::new();

    // Trigger check
    let _ = studio.update(Message::TriggerCheck);
    assert_eq!(studio.terminal.active_tab, PanelTab::BuildConsole);
    assert!(!studio.terminal.build_logs.is_empty());

    // Trigger run
    let _ = studio.update(Message::TriggerRun);
    assert_eq!(studio.terminal.active_tab, PanelTab::Terminal);
    assert!(!studio.terminal.logs.is_empty());

    // Switch to diagnostics
    let _ = studio.update(Message::TerminalTabSelected(PanelTab::Diagnostics));
    assert_eq!(studio.terminal.active_tab, PanelTab::Diagnostics);
    assert_eq!(studio.terminal.diagnostics.len(), 1);

    // Clear terminal
    let _ = studio.update(Message::ClearTerminal);
    assert!(studio.terminal.diagnostics.is_empty());
}

#[test]
fn test_view_tree_generation() {
    let (mut studio, _task) = Studio::new();

    // Ensure base view constructs cleanly
    {
        let _view = studio.view();
    }

    // Open split editor and command palette, ensure stack view constructs without panic
    let _ = studio.update(Message::ToggleSplitEditor);
    let _ = studio.update(Message::OpenCommandPalette(true));
    {
        let _view_modal = studio.view();
    }
}

#[test]
fn test_command_palette_and_quick_open() {
    let (mut studio, _task) = Studio::new();

    // Open file search
    let _ = studio.update(Message::OpenCommandPalette(false));
    assert!(studio.palette.is_open);
    assert!(studio.palette.query.is_empty());

    // Search for raster
    let _ = studio.update(Message::PaletteQueryChanged("raster".to_string()));
    let matches = studio.palette.filtered();
    assert!(!matches.is_empty());
    assert_eq!(matches[0].title, "raster.rs");

    // Select and choose raster.rs (id: 2)
    let _ = studio.update(Message::PaletteItemChosen(2));
    assert!(!studio.palette.is_open);
    assert_eq!(studio.docs.active().unwrap().path, "src/raster.rs");

    // Open action command mode
    let _ = studio.update(Message::OpenCommandPalette(true));
    assert!(studio.palette.is_open);
    assert_eq!(studio.palette.query, "> ");

    // Close palette
    let _ = studio.update(Message::CloseCommandPalette);
    assert!(!studio.palette.is_open);
}

#[test]
fn test_dual_split_editor() {
    let (mut studio, _task) = Studio::new();

    // Initially split is disabled
    assert!(studio.docs.split_tab.is_none());

    // Toggle split editor on
    let _ = studio.update(Message::ToggleSplitEditor);
    assert!(studio.docs.split_tab.is_some());
    assert_eq!(studio.docs.focused_pane, 0);

    // Switch focus to secondary split pane
    let _ = studio.update(Message::FocusPane(1));
    assert_eq!(studio.docs.focused_pane, 1);

    // Change split pane active tab
    let _ = studio.update(Message::SplitTabSelected(2));
    assert_eq!(studio.docs.split_tab, Some(2));
    assert_eq!(studio.docs.active().unwrap().name, "compute_raster.wgsl");

    // Toggle split off
    let _ = studio.update(Message::ToggleSplitEditor);
    assert!(studio.docs.split_tab.is_none());
    assert_eq!(studio.docs.focused_pane, 0);
}

#[test]
fn test_in_editor_find_and_replace() {
    let (mut studio, _task) = Studio::new();

    // Open find
    let _ = studio.update(Message::ToggleFind);
    assert!(studio.docs.find_open);

    // Search for "fn" in main.rs
    let _ = studio.update(Message::FindQueryChanged("fn".to_string()));
    assert!(studio.docs.find_matches > 0);
    assert_eq!(studio.docs.current_match, 1);

    // Navigate matches
    let _ = studio.update(Message::FindNext);
    assert!(studio.docs.current_match >= 1);

    let _ = studio.update(Message::FindPrev);
    assert!(studio.docs.current_match >= 1);

    // Close find
    let _ = studio.update(Message::ToggleFind);
    assert!(!studio.docs.find_open);
}

#[test]
fn test_diagnostic_quick_fix() {
    let (mut studio, _task) = Studio::new();
    assert_eq!(studio.terminal.diagnostics.len(), 1);

    // Active document is main.rs where the diagnostic is located
    assert_eq!(studio.docs.active().unwrap().path, "src/main.rs");

    // Apply quick fix
    let _ = studio.update(Message::ApplyDiagnosticFix(0));
    assert_eq!(studio.terminal.diagnostics.len(), 0);
}

#[test]
fn test_benchmark_dashboard() {
    let (mut studio, _task) = Studio::new();

    // Switch to benchmark tab
    let _ = studio.update(Message::TerminalTabSelected(PanelTab::Benchmark));
    assert_eq!(studio.terminal.active_tab, PanelTab::Benchmark);
}

#[test]
fn test_save_document() {
    let (mut studio, _task) = Studio::new();

    // Edit active document
    let _ = studio.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('Z'),
    )));
    assert!(studio.docs.active().unwrap().is_dirty);

    // Save document
    let _ = studio.update(Message::SaveDocument);
    assert!(!studio.docs.active().unwrap().is_dirty);
}

#[test]
fn test_activity_bar_and_sidebar_switching() {
    use taim_ide::SidebarView;

    let (mut studio, _task) = Studio::new();
    assert_eq!(studio.activity.active_view, SidebarView::Explorer);
    assert!(studio.show_sidebar);

    // Switch to Search
    let _ = studio.update(Message::SelectSidebarView(SidebarView::Search));
    assert_eq!(studio.activity.active_view, SidebarView::Search);
    assert!(studio.show_sidebar);

    // Switch to Source Control
    let _ = studio.update(Message::SelectSidebarView(SidebarView::SourceControl));
    assert_eq!(studio.activity.active_view, SidebarView::SourceControl);
    assert!(studio.show_sidebar);

    // Clicking active view toggles sidebar visibility
    let _ = studio.update(Message::SelectSidebarView(SidebarView::SourceControl));
    assert!(!studio.show_sidebar);

    // Re-clicking active view restores sidebar visibility
    let _ = studio.update(Message::SelectSidebarView(SidebarView::SourceControl));
    assert!(studio.show_sidebar);

    // Switch to Debug, Extensions, Settings
    let _ = studio.update(Message::SelectSidebarView(SidebarView::Debug));
    assert_eq!(studio.activity.active_view, SidebarView::Debug);

    let _ = studio.update(Message::SelectSidebarView(SidebarView::Extensions));
    assert_eq!(studio.activity.active_view, SidebarView::Extensions);

    let _ = studio.update(Message::SelectSidebarView(SidebarView::Settings));
    assert_eq!(studio.activity.active_view, SidebarView::Settings);
}

#[test]
fn test_global_workspace_search() {
    let (mut studio, _task) = Studio::new();

    // Default search has hits
    assert!(studio.search.total_matches > 0);

    // Search query update
    let _ = studio.update(Message::GlobalSearchQueryChanged("println".to_string()));
    let _ = studio.update(Message::ToggleSearchCase);
    assert!(studio.search.match_case);

    let _ = studio.update(Message::ToggleSearchWord);
    assert!(studio.search.whole_word);

    let _ = studio.update(Message::ToggleSearchReplace);
    assert!(studio.search.show_replace);
}

#[test]
fn test_source_control_git() {
    let (mut studio, _task) = Studio::new();

    assert_eq!(studio.git.active_branch, "main");
    assert!(!studio.git.unstaged_changes.is_empty());

    // Stage a file
    let file = studio.git.unstaged_changes[0].file_path.clone();
    let _ = studio.update(Message::GitStageFile(file.clone()));
    assert!(
        studio
            .git
            .staged_changes
            .iter()
            .any(|c| c.file_path == file)
    );

    // Unstage the file
    let _ = studio.update(Message::GitUnstageFile(file.clone()));
    assert!(
        studio
            .git
            .unstaged_changes
            .iter()
            .any(|c| c.file_path == file)
    );

    // Stage all
    let _ = studio.update(Message::GitStageAll);
    assert!(studio.git.unstaged_changes.is_empty());
    assert!(!studio.git.staged_changes.is_empty());

    // Toggle Diff View
    let _ = studio.update(Message::GitToggleDiff(file.clone()));
    assert!(studio.git.is_diff_view_open);
    assert_eq!(studio.git.diff_file.as_deref(), Some(file.as_str()));

    // Close Diff View
    let _ = studio.update(Message::GitToggleDiff(file));
    assert!(!studio.git.is_diff_view_open);

    // Commit changes
    let _ = studio.update(Message::GitCommitMessageChanged(
        "feat: add GPU acceleration".to_string(),
    ));
    let _ = studio.update(Message::GitCommit);
    assert!(studio.git.staged_changes.is_empty());
    assert_eq!(studio.git.commit_history[0], "feat: add GPU acceleration");
}

#[test]
fn test_run_and_debug_lifecycle() {
    use taim_ide::DebugStatus;

    let (mut studio, _task) = Studio::new();
    assert_eq!(studio.debug.status, DebugStatus::Stopped);

    // Start Debugging
    let _ = studio.update(Message::DebugStart);
    assert_eq!(studio.debug.status, DebugStatus::Paused);

    // Step Over
    let line_before = studio.debug.call_stack[0].line;
    let _ = studio.update(Message::DebugStepOver);
    assert_eq!(studio.debug.call_stack[0].line, line_before + 1);

    // Continue
    let _ = studio.update(Message::DebugContinue);
    assert_eq!(studio.debug.status, DebugStatus::Running);

    // Breakpoints
    let bp_count = studio.debug.breakpoints.len();
    let _ = studio.update(Message::DebugToggleBreakpoint(
        "src/main.rs".to_string(),
        100,
    ));
    assert_eq!(studio.debug.breakpoints.len(), bp_count + 1);

    // Watch expressions
    let _ = studio.update(Message::DebugWatchInputChanged(
        "studio.font_size".to_string(),
    ));
    let _ = studio.update(Message::DebugAddWatch);
    assert!(
        studio
            .debug
            .watch_expressions
            .iter()
            .any(|w| w.expression == "studio.font_size")
    );

    // Stop Debugging
    let _ = studio.update(Message::DebugStop);
    assert_eq!(studio.debug.status, DebugStatus::Stopped);
}

#[test]
fn test_extensions_marketplace() {
    use taim_ide::ExtensionFilter;

    let (mut studio, _task) = Studio::new();

    // Query filter
    let _ = studio.update(Message::ExtensionQueryChanged("rust".to_string()));
    assert!(!studio.extensions.filtered().is_empty());

    // Category filter
    let _ = studio.update(Message::ExtensionFilterSelected(ExtensionFilter::Installed));
    assert!(studio.extensions.filtered().iter().all(|e| e.is_installed));

    // Toggle install on uninstalled item
    let target_id = studio
        .extensions
        .extensions
        .iter()
        .find(|e| !e.is_installed)
        .map(|e| e.id)
        .unwrap();
    let _ = studio.update(Message::ExtensionToggleInstall(target_id));
    assert!(
        studio
            .extensions
            .extensions
            .iter()
            .find(|e| e.id == target_id)
            .unwrap()
            .is_installed
    );
}

#[test]
fn test_multi_terminal_and_debug_repl() {
    use taim_ide::OutputChannel;

    let (mut studio, _task) = Studio::new();

    // Add new terminal instance
    let initial_count = studio.terminal.terminals.len();
    let _ = studio.update(Message::NewTerminalTab);
    assert_eq!(studio.terminal.terminals.len(), initial_count + 1);
    assert_eq!(studio.terminal.active_terminal_idx, initial_count);

    // Switch output channel
    let _ = studio.update(Message::SelectOutputChannel(OutputChannel::CargoBuild));
    assert_eq!(studio.terminal.active_channel, OutputChannel::CargoBuild);

    // Debug REPL evaluation
    let _ = studio.update(Message::DebugReplInputChanged(
        "taim::version()".to_string(),
    ));
    let _ = studio.update(Message::DebugReplSubmit);
    assert!(
        studio
            .terminal
            .debug_logs
            .iter()
            .any(|l| l.message.contains("0.15.0-dev"))
    );
}

#[test]
fn test_toast_notifications() {
    let (mut studio, _task) = Studio::new();
    assert!(studio.notifications.count() > 0);

    let first_id = studio.notifications.active_toasts[0].id;
    let _ = studio.update(Message::DismissNotification(first_id));
    assert!(
        !studio
            .notifications
            .active_toasts
            .iter()
            .any(|t| t.id == first_id)
    );
}

#[test]
fn test_breadcrumbs_generation() {
    let (studio, _task) = Studio::new();
    let active_doc = studio.docs.active().unwrap();
    let crumbs = active_doc.breadcrumbs();

    assert_eq!(crumbs[0], "taim-project");
    assert_eq!(crumbs[1], "src");
    assert_eq!(crumbs[2], "main.rs");
    assert_eq!(crumbs[3], "fn run()");
}

#[test]
fn test_lsp_go_to_definition_simd_fallback() {
    let (mut studio, _task) = Studio::new();
    // Default open document has "fn main()" or similar
    assert!(studio.fast_simd_definition_jump("main"));
    let active = studio.docs.active().unwrap();
    assert!(active.text().contains("fn main()"));
}

#[test]
fn test_compiler_and_diagnostic_navigation() {
    let (mut studio, _task) = Studio::new();
    
    // Test parser for compiler error format
    let raw_error = "--> src/main.rs:5:10";
    let loc = taim_ide::parse_compiler_location(raw_error).expect("should parse compiler error location");
    assert_eq!(loc.0, "src/main.rs");
    assert_eq!(loc.1, 5);
    assert_eq!(loc.2, 10);

    // Test NavigateToLocation
    let _ = studio.update(Message::NavigateToLocation("src/main.rs".to_string(), 5, 2));
    let active = studio.docs.active().unwrap();
    assert_eq!(active.path, "src/main.rs");
    assert_eq!(active.content.cursor().position.line, 4); // 0-based
}

#[test]
fn test_interactive_merge_conflict_resolution() {
    let (mut studio, _task) = Studio::new();
    let conflicted = "\
fn feature() {
<<<<<<< HEAD (Current Change)
    let engine = \"Metal GPU\";
=======
    let engine = \"Vulkan GPU\";
>>>>>>> feature/vulkan (Incoming Change)
    println!(\"{engine}\");
}
";
    // Replace active doc content with conflicted code
    if let Some(doc) = studio.docs.active_mut() {
        doc.content = taim::widget::text_editor::Content::with_text(conflicted);
    }
    let doc = studio.docs.active().unwrap();
    assert!(doc.has_merge_conflicts());
    assert_eq!(doc.find_merge_conflicts().len(), 1);

    // Accept Current
    let _ = studio.update(Message::AcceptConflictCurrent(0));
    let doc = studio.docs.active().unwrap();
    assert!(!doc.has_merge_conflicts());
    assert!(doc.text().contains("Metal GPU"));
    assert!(!doc.text().contains("Vulkan GPU"));
}

#[test]
fn test_code_folding_workbench_integration() {
    let (mut studio, _task) = Studio::new();
    let sample = "\
fn compute() {
    let a = 10;
    let b = 20;
}
";
    if let Some(doc) = studio.docs.active_mut() {
        doc.content = taim::widget::text_editor::Content::with_text(sample);
    }
    assert!(studio.docs.active().unwrap().is_line_foldable(0));

    // Toggle fold line 0
    let _ = studio.update(Message::ToggleFold(studio.docs.active_tab, 0));
    assert!(studio.docs.active().unwrap().is_line_folded(1));
    assert!(studio.docs.active().unwrap().is_line_folded(2));

    // Unfold all
    let _ = studio.update(Message::UnfoldAll);
    assert!(!studio.docs.active().unwrap().is_line_folded(1));

    // Fold all
    let _ = studio.update(Message::FoldAll);
    assert!(studio.docs.active().unwrap().is_line_folded(1));
}

#[test]
fn test_palette_and_shortcut_routes_for_new_features() {
    let (mut studio, _task) = Studio::new();
    // Test Go to Definition palette execution
    let item_def = studio.palette.all_items.iter().find(|i| i.payload == "action:goto_definition").unwrap();
    assert_eq!(item_def.shortcut, Some("F12".to_string()));

    // Test Fold All palette execution
    let item_fold = studio.palette.all_items.iter().find(|i| i.payload == "action:fold_all").unwrap();
    assert!(item_fold.title.contains("Fold All Blocks"));
    let _ = studio.update(Message::PaletteItemChosen(item_fold.id));
}
