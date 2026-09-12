//! Source Control & Git Integration for Taim Studio.
//! Provides branch switching, staging, commit messages, and side-by-side diffing.

/// Status of a modified or untracked file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitFileStatus {
    Modified,
    Added,
    Deleted,
    Untracked,
}

impl GitFileStatus {
    pub fn badge(&self) -> &'static str {
        match self {
            GitFileStatus::Modified => "M",
            GitFileStatus::Added => "A",
            GitFileStatus::Deleted => "D",
            GitFileStatus::Untracked => "U",
        }
    }
}

/// A git-tracked file change.
#[derive(Debug, Clone)]
pub struct GitChange {
    pub file_path: String,
    pub status: GitFileStatus,
}

/// Line classification for Git Diff viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Addition,
    Deletion,
}

/// A single line in a Git Diff view.
#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line_num: Option<usize>,
    pub new_line_num: Option<usize>,
    pub content: String,
}

/// Git repository state.
#[derive(Debug, Clone)]
pub struct GitState {
    pub active_branch: String,
    pub branches: Vec<String>,
    pub staged_changes: Vec<GitChange>,
    pub unstaged_changes: Vec<GitChange>,
    pub commit_message: String,
    pub commit_history: Vec<String>,
    pub diff_file: Option<String>,
    pub diff_lines: Vec<DiffLine>,
    pub is_diff_view_open: bool,
}

impl Default for GitState {
    fn default() -> Self {
        Self::new()
    }
}

impl GitState {
    pub fn new() -> Self {
        let unstaged = vec![
            GitChange {
                file_path: "src/lib.rs".to_string(),
                status: GitFileStatus::Modified,
            },
            GitChange {
                file_path: "Cargo.toml".to_string(),
                status: GitFileStatus::Modified,
            },
        ];

        let staged = vec![GitChange {
            file_path: "src/document.rs".to_string(),
            status: GitFileStatus::Modified,
        }];

        let mut state = Self {
            active_branch: "main".to_string(),
            branches: vec![
                "main".to_string(),
                "feat/gpu-compute".to_string(),
                "fix/mem-bench".to_string(),
            ],
            staged_changes: staged,
            unstaged_changes: unstaged,
            commit_message: String::new(),
            commit_history: vec![
                "feat(ide): add native GPU-accelerated editor engine".to_string(),
                "perf(raster): replace division with cross-product check".to_string(),
            ],
            diff_file: None,
            diff_lines: Vec::new(),
            is_diff_view_open: false,
        };
        state.load_sample_diff("src/lib.rs");
        state
    }

    /// Number of total pending changes.
    pub fn total_changes(&self) -> usize {
        self.staged_changes.len() + self.unstaged_changes.len()
    }

    /// Stages an unstaged file.
    pub fn stage_file(&mut self, file_path: &str) {
        if let Some(pos) = self
            .unstaged_changes
            .iter()
            .position(|c| c.file_path == file_path)
        {
            let change = self.unstaged_changes.remove(pos);
            self.staged_changes.push(change);
        }
    }

    /// Unstages a staged file.
    pub fn unstage_file(&mut self, file_path: &str) {
        if let Some(pos) = self
            .staged_changes
            .iter()
            .position(|c| c.file_path == file_path)
        {
            let change = self.staged_changes.remove(pos);
            self.unstaged_changes.push(change);
        }
    }

    /// Stages all unstaged changes.
    pub fn stage_all(&mut self) {
        let mut to_move = Vec::new();
        std::mem::swap(&mut self.unstaged_changes, &mut to_move);
        self.staged_changes.extend(to_move);
    }

    /// Unstages all staged changes.
    pub fn unstage_all(&mut self) {
        let mut to_move = Vec::new();
        std::mem::swap(&mut self.staged_changes, &mut to_move);
        self.unstaged_changes.extend(to_move);
    }

    /// Commits staged changes with current commit message.
    pub fn commit(&mut self) -> Result<String, &'static str> {
        if self.commit_message.trim().is_empty() {
            return Err("Commit message cannot be empty");
        }
        if self.staged_changes.is_empty() {
            return Err("No staged changes to commit");
        }

        let msg = self.commit_message.trim().to_string();
        self.commit_history.insert(0, msg.clone());
        self.commit_message.clear();
        let count = self.staged_changes.len();
        self.staged_changes.clear();
        self.is_diff_view_open = false;

        Ok(format!("Committed {} file(s): {}", count, msg))
    }

    /// Opens or closes the diff view for a specific file.
    pub fn toggle_diff_for(&mut self, file_path: &str) {
        if self.is_diff_view_open && self.diff_file.as_deref() == Some(file_path) {
            self.is_diff_view_open = false;
        } else {
            self.diff_file = Some(file_path.to_string());
            self.load_sample_diff(file_path);
            self.is_diff_view_open = true;
        }
    }

    /// Generates realistic diff lines for visual comparison.
    pub fn load_sample_diff(&mut self, file_path: &str) {
        self.diff_lines = vec![
            DiffLine {
                kind: DiffLineKind::Context,
                old_line_num: Some(1),
                new_line_num: Some(1),
                content: "use taim::widget::{button, row, column, text};".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Deletion,
                old_line_num: Some(2),
                new_line_num: None,
                content: "-// Legacy CPU rendering loop (33ms frame)".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Addition,
                old_line_num: None,
                new_line_num: Some(2),
                content: "+// Native GPU Compute Rasterizer (0.28ms frame, 120 FPS locked)"
                    .to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Addition,
                old_line_num: None,
                new_line_num: Some(3),
                content: "+use taim_raster::compute::{GpuScene, HardwareDevice};".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Context,
                old_line_num: Some(3),
                new_line_num: Some(4),
                content: "".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Context,
                old_line_num: Some(4),
                new_line_num: Some(5),
                content: "pub fn initialize_surface() {".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Addition,
                old_line_num: None,
                new_line_num: Some(6),
                content: "+    let _pass = GpuScene::bind_zero_copy();".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Context,
                old_line_num: Some(5),
                new_line_num: Some(7),
                content: "}".to_string(),
            },
        ];
        self.diff_file = Some(file_path.to_string());
    }
}
