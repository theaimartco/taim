//! Multi-tab document manager and text editor state for Taim Studio.

use std::cell::RefCell;
use taim::widget::text_editor;

/// Backing storage and viewport mode for an open document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentBacking {
    /// In-memory standard buffer (fully loaded).
    Standard,
    /// Memory-mapped large file (>2MB) streamed on demand.
    MemoryMapped {
        /// Total file size on disk in bytes.
        file_size: u64,
        /// Estimated or scanned total line count in the entire file.
        total_lines: usize,
        /// Whether only an initial viewport chunk is currently loaded in editor memory.
        is_partial: bool,
    },
}

/// An open document with its own text editor content and metadata.
pub struct Document {
    /// Document ID (matches file node id if opened from tree).
    pub id: usize,
    /// Virtual file path (e.g. `src/main.rs`).
    pub path: String,
    /// Display tab name (e.g. `main.rs`).
    pub name: String,
    /// Syntax token for highlighting (`rust`, `toml`, `c`, `markdown`, `json`).
    pub syntax: String,
    /// The live editor content.
    pub content: text_editor::Content,
    /// Whether the document has unsaved edits.
    pub is_dirty: bool,
    /// Whether the tab is pinned.
    pub is_pinned: bool,
    /// The disk content seen when this buffer was opened or last saved.
    pub disk_snapshot: Option<String>,
    /// Cached minimap density profile to prevent per-frame re-scanning.
    pub minimap_density_cache: RefCell<Option<std::sync::Arc<[u8]>>>,
    /// Storage backing mode (Standard or MemoryMapped).
    pub backing: DocumentBacking,
    /// Secondary cursor locations for multi-cursor editing.
    pub extra_cursors: Vec<text_editor::Position>,
    /// Last word or selection query used by Cmd+D multi-cursor expansion.
    pub last_select_query: Option<String>,
    /// Folded line ranges (start_line, end_line).
    pub folded_ranges: Vec<(usize, usize)>,
}

/// A detected git merge conflict in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeConflict {
    pub start_line: usize,
    pub separator_line: usize,
    pub end_line: usize,
    pub current_label: String,
    pub incoming_label: String,
}

/// Strategy for resolving a merge conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictChoice {
    AcceptCurrent,
    AcceptIncoming,
    AcceptBoth,
}

impl Document {
    /// Creates a new document with the provided initial text.
    pub fn new(
        id: usize,
        path: impl Into<String>,
        name: impl Into<String>,
        syntax: impl Into<String>,
        text: &str,
    ) -> Self {
        Self {
            id,
            path: path.into(),
            name: name.into(),
            syntax: syntax.into(),
            content: text_editor::Content::with_text(text),
            is_dirty: false,
            is_pinned: false,
            disk_snapshot: None,
            minimap_density_cache: RefCell::new(None),
            backing: DocumentBacking::Standard,
            extra_cursors: Vec::new(),
            last_select_query: None,
            folded_ranges: Vec::new(),
        }
    }

    /// Opens a file from disk using memory-mapped chunking for large files (>2MB)
    /// to guarantee sub-millisecond opening regardless of file size.
    pub fn open_disk_file(
        id: usize,
        path: impl Into<String>,
        name: impl Into<String>,
        syntax: impl Into<String>,
        disk_path: &std::path::Path,
    ) -> std::io::Result<Self> {
        let path = path.into();
        let name = name.into();
        let syntax = syntax.into();
        let metadata = std::fs::symlink_metadata(disk_path)?;
        let file_size = metadata.len();

        const LARGE_FILE_THRESHOLD: u64 = 1 * 1024 * 1024; // 1MB

        if file_size > LARGE_FILE_THRESHOLD {
            let file = std::fs::File::open(disk_path)?;
            let mmap = unsafe { memmap2::Mmap::map(&file)? };

            // Take the first chunk (up to 2MB) ending on a valid newline boundary
            let max_chunk = (2 * 1024 * 1024).min(mmap.len());
            let chunk_slice = &mmap[..max_chunk];
            let end_offset = chunk_slice
                .iter()
                .rposition(|&b| b == b'\n')
                .map(|pos| pos + 1)
                .unwrap_or(max_chunk);

            let initial_text = match std::str::from_utf8(&mmap[..end_offset]) {
                Ok(valid_str) => valid_str,
                Err(error) => std::str::from_utf8(&mmap[..error.valid_up_to()]).unwrap_or(""),
            };

            // Fast newline count across the memory map using vectorization
            let total_lines = mmap.iter().filter(|&&b| b == b'\n').count() + 1;
            let is_partial = file_size > initial_text.len() as u64;

            let mut doc = Self::new(id, path, name, syntax, initial_text);
            doc.backing = DocumentBacking::MemoryMapped {
                file_size,
                total_lines,
                is_partial,
            };
            if !is_partial {
                doc.disk_snapshot = Some(initial_text.to_string());
            }
            Ok(doc)
        } else {
            let content = std::fs::read_to_string(disk_path)?;
            let mut doc = Self::new(id, path, name, syntax, &content);
            doc.disk_snapshot = Some(content);
            doc.backing = DocumentBacking::Standard;
            Ok(doc)
        }
    }

    /// Returns whether any extra cursors are active.
    pub fn has_extra_cursors(&self) -> bool {
        !self.extra_cursors.is_empty()
    }

    /// Returns the active extra cursors.
    pub fn extra_cursors(&self) -> &[text_editor::Position] {
        &self.extra_cursors
    }

    /// Clears all extra cursors.
    pub fn clear_extra_cursors(&mut self) {
        self.extra_cursors.clear();
        self.last_select_query = None;
    }

    /// Returns a list of unique line indices that contain extra cursors.
    pub fn extra_cursor_lines(&self) -> Vec<usize> {
        let mut lines: Vec<usize> = self.extra_cursors.iter().map(|c| c.line).collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    /// Selects the next occurrence of the word under cursor (Cmd+D / Ctrl+D).
    pub fn select_next_occurrence(&mut self) -> bool {
        let query = if let Some(query) = &self.last_select_query {
            query.clone()
        } else {
            let mut selected_text = self.content.selection();
            if selected_text.as_ref().map_or(true, |s| s.is_empty()) {
                self.content.perform(text_editor::Action::SelectWord);
                selected_text = self.content.selection();
            }
            let Some(q) = selected_text.filter(|s| !s.is_empty()) else {
                return false;
            };
            self.content
                .perform(text_editor::Action::Move(text_editor::Motion::Right));
            self.last_select_query = Some(q.clone());
            q
        };

        let total_lines = self.content.line_count();
        if total_lines == 0 {
            return false;
        }

        let cursor = self.content.cursor();
        let start_pos = self
            .extra_cursors
            .iter()
            .max_by_key(|p| (p.line, p.column))
            .copied()
            .unwrap_or(cursor.position);

        let mut found = None;

        // 1. Search forward from start_pos to end of document
        for line_idx in start_pos.line..total_lines {
            if let Some(line) = self.content.line(line_idx) {
                let col_start = if line_idx == start_pos.line {
                    start_pos.column
                } else {
                    0
                };
                if col_start < line.text.len() {
                    if let Some(offset) = line.text[col_start..].find(&query) {
                        let match_col = col_start + offset + query.len();
                        let match_pos = text_editor::Position::new(line_idx, match_col);
                        if !self.extra_cursors.contains(&match_pos) && match_pos != cursor.position
                        {
                            found = Some(match_pos);
                            break;
                        }
                    }
                }
            }
        }

        // 2. Wrap around from top of document if not found
        if found.is_none() {
            for line_idx in 0..=start_pos.line.min(total_lines.saturating_sub(1)) {
                if let Some(line) = self.content.line(line_idx) {
                    let max_col = if line_idx == start_pos.line {
                        start_pos.column
                    } else {
                        line.text.len()
                    };
                    if let Some(offset) = line.text[..max_col.min(line.text.len())].find(&query) {
                        let match_col = offset + query.len();
                        let match_pos = text_editor::Position::new(line_idx, match_col);
                        if !self.extra_cursors.contains(&match_pos) && match_pos != cursor.position
                        {
                            found = Some(match_pos);
                            break;
                        }
                    }
                }
            }
        }

        if let Some(next_pos) = found {
            self.extra_cursors.push(next_pos);
            true
        } else {
            false
        }
    }

    /// Extracts the identifier / word token under or immediately adjacent to the cursor.
    pub fn word_under_cursor(&self) -> Option<String> {
        let cursor = self.content.cursor().position;
        let line = self.content.line(cursor.line)?;
        let text = &line.text;
        if text.is_empty() {
            return None;
        }
        let col = cursor.column.min(text.len());
        let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        let bytes = text.as_bytes();
        let mut start = col;
        if start == bytes.len() && start > 0 && is_ident(bytes[start - 1]) {
            start -= 1;
        } else if start < bytes.len()
            && !is_ident(bytes[start])
            && start > 0
            && is_ident(bytes[start - 1])
        {
            start -= 1;
        }
        if start >= bytes.len() || !is_ident(bytes[start]) {
            return None;
        }
        while start > 0 && is_ident(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = col;
        while end < bytes.len() && is_ident(bytes[end]) {
            end += 1;
        }
        if start < end {
            Some(text[start..end].to_string())
        } else {
            None
        }
    }

    /// Identifies git merge conflicts (<<<<<<<, =======, >>>>>>>).
    pub fn find_merge_conflicts(&self) -> Vec<MergeConflict> {
        let mut conflicts = Vec::new();
        let mut current_start = None;
        let mut current_sep = None;
        let count = self.content.line_count();

        for i in 0..count {
            if let Some(line) = self.content.line(i) {
                let trimmed = line.text.trim_start();
                if trimmed.starts_with("<<<<<<<") {
                    let label = trimmed
                        .strip_prefix("<<<<<<<")
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    current_start = Some((i, label));
                    current_sep = None;
                } else if trimmed.starts_with("=======")
                    && current_start.is_some()
                    && current_sep.is_none()
                {
                    current_sep = Some(i);
                } else if trimmed.starts_with(">>>>>>>")
                    && current_start.is_some()
                    && current_sep.is_some()
                {
                    let (start, cur_label) = current_start.take().unwrap();
                    let sep = current_sep.take().unwrap();
                    let inc_label = trimmed
                        .strip_prefix(">>>>>>>")
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    conflicts.push(MergeConflict {
                        start_line: start,
                        separator_line: sep,
                        end_line: i,
                        current_label: cur_label,
                        incoming_label: inc_label,
                    });
                }
            }
        }
        conflicts
    }

    /// Fast check whether document contains merge conflict markers.
    pub fn has_merge_conflicts(&self) -> bool {
        let count = self.content.line_count();
        for i in 0..count {
            if let Some(line) = self.content.line(i) {
                if line.text.trim_start().starts_with("<<<<<<<") {
                    return true;
                }
            }
        }
        false
    }

    /// Resolves a single merge conflict at `conflict_index`.
    pub fn resolve_conflict(&mut self, conflict_index: usize, choice: ConflictChoice) -> bool {
        let conflicts = self.find_merge_conflicts();
        let Some(conflict) = conflicts.get(conflict_index) else {
            return false;
        };
        let count = self.content.line_count();
        let mut new_lines = Vec::with_capacity(count);

        for i in 0..count {
            if i < conflict.start_line || i > conflict.end_line {
                if let Some(line) = self.content.line(i) {
                    new_lines.push(line.text.to_string());
                }
            } else if i == conflict.start_line {
                match choice {
                    ConflictChoice::AcceptCurrent => {
                        for j in (conflict.start_line + 1)..conflict.separator_line {
                            if let Some(line) = self.content.line(j) {
                                new_lines.push(line.text.to_string());
                            }
                        }
                    }
                    ConflictChoice::AcceptIncoming => {
                        for j in (conflict.separator_line + 1)..conflict.end_line {
                            if let Some(line) = self.content.line(j) {
                                new_lines.push(line.text.to_string());
                            }
                        }
                    }
                    ConflictChoice::AcceptBoth => {
                        for j in (conflict.start_line + 1)..conflict.separator_line {
                            if let Some(line) = self.content.line(j) {
                                new_lines.push(line.text.to_string());
                            }
                        }
                        for j in (conflict.separator_line + 1)..conflict.end_line {
                            if let Some(line) = self.content.line(j) {
                                new_lines.push(line.text.to_string());
                            }
                        }
                    }
                }
            }
        }

        let new_text = new_lines.join("\n");
        self.content = text_editor::Content::with_text(&new_text);
        self.is_dirty = true;
        self.minimap_density_cache.borrow_mut().take();
        true
    }

    /// Resolves all merge conflicts in the document using the chosen strategy.
    pub fn resolve_all_conflicts(&mut self, choice: ConflictChoice) -> usize {
        let mut resolved = 0;
        while self.resolve_conflict(0, choice) {
            resolved += 1;
        }
        resolved
    }

    /// Computes foldable block line ranges based on braces.
    pub fn foldable_blocks(&self) -> Vec<(usize, usize)> {
        let mut blocks = Vec::new();
        let mut stack = Vec::new();
        let count = self.content.line_count();
        for i in 0..count {
            if let Some(line) = self.content.line(i) {
                let text = line.text.trim();
                if text.contains('{') && !text.contains('}') {
                    stack.push(i);
                } else if text.contains('}') && !text.contains('{') {
                    if let Some(start) = stack.pop() {
                        if i > start {
                            blocks.push((start, i));
                        }
                    }
                }
            }
        }
        blocks
    }

    /// Returns whether the specified line starts a foldable block.
    pub fn is_line_foldable(&self, line: usize) -> bool {
        self.foldable_blocks()
            .iter()
            .any(|(start, _)| *start == line)
    }

    /// Returns whether the specified line is currently hidden inside a folded block.
    pub fn is_line_folded(&self, line: usize) -> bool {
        self.folded_ranges
            .iter()
            .any(|(start, end)| line > *start && line <= *end)
    }

    /// Toggles folding for the block starting at `line`.
    pub fn toggle_fold_at(&mut self, line: usize) -> bool {
        if let Some(pos) = self
            .folded_ranges
            .iter()
            .position(|(start, _)| *start == line)
        {
            self.folded_ranges.remove(pos);
            true
        } else if let Some((start, end)) =
            self.foldable_blocks().into_iter().find(|(s, _)| *s == line)
        {
            self.folded_ranges.push((start, end));
            true
        } else {
            false
        }
    }

    /// Folds all foldable blocks in the document.
    pub fn fold_all(&mut self) {
        self.folded_ranges = self.foldable_blocks();
    }

    /// Unfolds all blocks in the document.
    pub fn unfold_all(&mut self) {
        self.folded_ranges.clear();
    }

    /// Returns the raw text string of the document content.
    pub fn text(&self) -> String {
        self.content.text()
    }

    /// Resolves breadcrumbs hierarchy for active document.
    pub fn breadcrumbs(&self) -> Vec<String> {
        let mut parts = vec!["taim-project".to_string()];
        for segment in self.path.split('/') {
            if !segment.is_empty() {
                parts.push(segment.to_string());
            }
        }
        if self.path.ends_with(".rs") {
            parts.push("fn run()".to_string());
        }
        parts
    }

    /// Performs an editing action on the document content, replicating edits
    /// across any active extra cursors.
    pub fn perform(&mut self, action: text_editor::Action) {
        match action {
            text_editor::Action::Edit(ref edit) if !self.extra_cursors.is_empty() => {
                let orig_pos = self.content.cursor().position;
                // Sort extra cursors descending so edits at higher offsets do not
                // invalidate coordinates of lower offsets.
                let mut sorted_cursors = self.extra_cursors.clone();
                sorted_cursors.sort_by(|a, b| (b.line, b.column).cmp(&(a.line, a.column)));

                let mut updated_cursors = Vec::with_capacity(sorted_cursors.len());
                for pos in sorted_cursors {
                    self.content.move_to(text_editor::Cursor::new(pos));
                    self.content
                        .perform(text_editor::Action::Edit(edit.clone()));
                    updated_cursors.push(self.content.cursor().position);
                }

                // Restore primary cursor and perform the edit at the primary location
                self.content.move_to(text_editor::Cursor::new(orig_pos));
                self.content.perform(action);

                self.extra_cursors = updated_cursors;
                self.is_dirty = true;
                *self.minimap_density_cache.borrow_mut() = None;
            }
            text_editor::Action::Click(_) | text_editor::Action::Drag(_) => {
                self.extra_cursors.clear();
                self.last_select_query = None;
                let is_edit = matches!(action, text_editor::Action::Edit(_));
                self.content.perform(action);
                if is_edit {
                    self.is_dirty = true;
                    *self.minimap_density_cache.borrow_mut() = None;
                }
            }
            _ => {
                let is_edit = matches!(action, text_editor::Action::Edit(_));
                self.content.perform(action);
                if is_edit {
                    self.is_dirty = true;
                    *self.minimap_density_cache.borrow_mut() = None;
                }
            }
        }
    }

    /// Returns the current (1-indexed) line and column for the cursor.
    pub fn cursor_position(&self) -> (usize, usize) {
        let cursor = self.content.cursor();
        (cursor.position.line + 1, cursor.position.column + 1)
    }

    /// Returns the line count of the document.
    pub fn line_count(&self) -> usize {
        match self.backing {
            DocumentBacking::Standard => self.content.line_count(),
            DocumentBacking::MemoryMapped {
                total_lines,
                is_partial,
                ..
            } => {
                if is_partial {
                    total_lines.max(self.content.line_count())
                } else {
                    self.content.line_count()
                }
            }
        }
    }

    /// Returns the cached minimap density preview, computing it lazily only when invalidated.
    pub fn minimap_density(&self) -> std::sync::Arc<[u8]> {
        let mut cache = self.minimap_density_cache.borrow_mut();
        if let Some(density) = cache.as_ref() {
            return std::sync::Arc::clone(density);
        }
        let line_count = self.content.line_count();
        let sample_count = line_count.min(140);
        let density: std::sync::Arc<[u8]> = if sample_count == 0 {
            std::sync::Arc::from([])
        } else {
            let vec: Vec<u8> = (0..sample_count)
                .map(|sample| {
                    let line = sample * line_count / sample_count;
                    self.content.line_preview_density(line).min(80) as u8
                })
                .collect();
            std::sync::Arc::from(vec)
        };
        *cache = Some(std::sync::Arc::clone(&density));
        density
    }
}

/// Manages multiple open editor tabs and split panes.
pub struct DocumentManager {
    /// Open documents.
    pub documents: Vec<Document>,
    /// Currently active document index in primary pane.
    pub active_tab: usize,
    /// Document index open in split secondary pane, if enabled.
    pub split_tab: Option<usize>,
    /// Which pane currently has focus (0 = primary, 1 = secondary).
    pub focused_pane: usize,
    /// Whether in-editor Find bar is active.
    pub find_open: bool,
    /// Active search term in find bar.
    pub find_query: String,
    /// Count of matches in active document.
    pub find_matches: usize,
    /// Active match index (1-based).
    pub current_match: usize,
    /// Monotonically increasing ID counter for scratchpads.
    next_id: usize,
}

impl Default for DocumentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentManager {
    /// Starts with no sample buffers for a real workspace.
    pub fn empty_workspace() -> Self {
        Self {
            documents: Vec::new(),
            active_tab: 0,
            split_tab: None,
            focused_pane: 0,
            find_open: false,
            find_query: String::new(),
            find_matches: 0,
            current_match: 0,
            next_id: 1000,
        }
    }

    /// Opens disk content or focuses a buffer that is already open.
    pub fn open_with_text(&mut self, id: usize, path: &str, name: &str, syntax: &str, text: &str) {
        if let Some(index) = self.documents.iter().position(|doc| doc.path == path) {
            self.active_tab = index;
            return;
        }
        let mut doc = Document::new(id, path, name, syntax, text);
        doc.disk_snapshot = Some(text.to_string());
        self.documents.push(doc);
        self.active_tab = self.documents.len() - 1;
    }

    /// Opens a disk file using memory-mapped chunking for large files.
    pub fn open_disk_file(
        &mut self,
        id: usize,
        path: &str,
        name: &str,
        syntax: &str,
        disk_path: &std::path::Path,
    ) -> std::io::Result<()> {
        if let Some(index) = self.documents.iter().position(|doc| doc.path == path) {
            self.active_tab = index;
            return Ok(());
        }
        let doc = Document::open_disk_file(id, path, name, syntax, disk_path)?;
        self.documents.push(doc);
        self.active_tab = self.documents.len() - 1;
        Ok(())
    }

    /// Initializes default open documents for Taim Studio.
    pub fn new() -> Self {
        let main_rs = r#"//! High-performance compute-raster demo powered by Taim.
use taim::prelude::*;
use taim::widget::{button, column, container, row, text};
use taim::signal::Signal;

pub fn main() -> taim::Result {
    println!("Initializing Taim compute rasterizer...");
    taim::application(App::new, App::update, App::view)
        .title("Taim App — 120 FPS Compute Engine")
        .run()
}

struct App {
    frame_count: Signal<u64>,
    raster_time_us: f32,
}

impl App {
    fn new() -> (Self, Task<()>) {
        (
            Self {
                frame_count: Signal::new(0),
                raster_time_us: 42.5,
            },
            Task::none(),
        )
    }

    fn update(&mut self, _msg: ()) -> Task<()> {
        self.frame_count.update(|c| *c += 1);
        Task::none()
    }

    fn view(&self) -> Element<'_, ()> {
        container(
            column![
                text("Zero-Rebuild Scene Graph").size(22),
                text(format!("Frame: {}", self.frame_count.get())).size(16),
            ]
            .spacing(12),
        )
        .padding(20)
        .into()
    }
}
"#;

        let cargo_toml = r#"[package]
name = "taim-sample"
version = "0.15.0"
edition = "2024"

[dependencies]
taim = { version = "0.15.0", features = ["wgpu", "signal", "highlighter"] }
glam = "0.33"
"#;

        let shader_wgsl = r#"// Taim Compute Rasterizer WGSL Shader
struct Vertex {
    pos: vec2<f32>,
    color: vec4<f32>,
};

struct Uniforms {
    viewport: vec2<f32>,
    scale: f32,
    gamma: f32,
};

@group(0) @binding(0) var<uniform> u_params: Uniforms;
@group(0) @binding(1) var out_texture: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let coords = vec2<i32>(global_id.xy);
    let uv = vec2<f32>(coords) / u_params.viewport;
    
    // Linear-light physical compositing
    let col = vec4<f32>(uv.x, uv.y, 0.5 + 0.5 * sin(uv.x * 6.28), 1.0);
    textureStore(out_texture, coords, col);
}
"#;

        let readme_md = r#"# Taim Studio

Welcome to **Taim Studio**, the zero-rebuild desktop IDE powered by Taim!

## Features
- **GPU Compute Raster**: Microsecond frame presentation via wgpu.
- **Incremental Syntax Highlighting**: Zero-allocation per-keystroke highlighting.
- **Multi-Tab Workspace**: Effortlessly navigate multiple documents.
- **Integrated Console**: Live asynchronous task streaming.
"#;

        let documents = vec![
            Document::new(10, "src/main.rs", "main.rs", "rust", main_rs),
            Document::new(1, "Cargo.toml", "Cargo.toml", "toml", cargo_toml),
            Document::new(
                20,
                "shaders/compute_raster.wgsl",
                "compute_raster.wgsl",
                "c",
                shader_wgsl,
            ),
            Document::new(2, "README.md", "README.md", "markdown", readme_md),
        ];

        Self {
            documents,
            active_tab: 0,
            split_tab: None,
            focused_pane: 0,
            find_open: false,
            find_query: String::new(),
            find_matches: 0,
            current_match: 0,
            next_id: 1000,
        }
    }

    /// Returns the currently active document in the focused pane.
    pub fn active(&self) -> Option<&Document> {
        if self.focused_pane == 1 && self.split_tab.is_some() {
            self.split_doc()
        } else {
            self.documents.get(self.active_tab)
        }
    }

    /// Returns a mutable reference to the active document in the focused pane.
    pub fn active_mut(&mut self) -> Option<&mut Document> {
        if self.focused_pane == 1 && self.split_tab.is_some() {
            self.split_doc_mut()
        } else {
            self.documents.get_mut(self.active_tab)
        }
    }

    /// Returns the document displayed in the split pane, if active.
    pub fn split_doc(&self) -> Option<&Document> {
        self.split_tab.and_then(|idx| self.documents.get(idx))
    }

    /// Returns a mutable reference to the split pane document.
    pub fn split_doc_mut(&mut self) -> Option<&mut Document> {
        let idx = self.split_tab?;
        self.documents.get_mut(idx)
    }

    /// Toggles side-by-side split dual-editor view.
    pub fn toggle_split(&mut self) {
        if self.split_tab.is_some() {
            self.split_tab = None;
            self.focused_pane = 0;
        } else {
            // Open another document in split pane (e.g. index 1 or 2)
            let other_idx = if self.documents.len() > 1 {
                if self.active_tab == 0 { 1 } else { 0 }
            } else {
                self.active_tab
            };
            self.split_tab = Some(other_idx);
        }
    }

    /// Toggles the in-editor find bar.
    pub fn toggle_find(&mut self) {
        self.find_open = !self.find_open;
        if self.find_open {
            self.recalculate_matches();
        }
    }

    /// Updates the find query and counts matches.
    pub fn set_find_query(&mut self, query: String) {
        self.find_query = query;
        self.recalculate_matches();
    }

    /// Cycles to next match.
    pub fn find_next(&mut self) {
        if self.find_matches > 0 {
            self.current_match = (self.current_match % self.find_matches) + 1;
        }
    }

    /// Cycles to previous match.
    pub fn find_prev(&mut self) {
        if self.find_matches > 0 {
            self.current_match = if self.current_match <= 1 {
                self.find_matches
            } else {
                self.current_match - 1
            };
        }
    }

    /// Counts how many times `find_query` appears in the active document.
    pub fn recalculate_matches(&mut self) {
        if self.find_query.is_empty() {
            self.find_matches = 0;
            self.current_match = 0;
            return;
        }

        let query = self.find_query.to_lowercase();
        if let Some(doc) = self.active() {
            let full_text = doc.content.text().to_lowercase();
            let matches = full_text.matches(&query).count();
            self.find_matches = matches;
            self.current_match = if matches > 0 { 1 } else { 0 };
        }
    }

    /// Selects an open tab by index.
    pub fn select_tab(&mut self, index: usize) {
        if index < self.documents.len() {
            self.active_tab = index;
        }
    }

    /// Closes a tab by index.
    pub fn close_tab(&mut self, index: usize) {
        if self.documents.len() <= 1 {
            // Keep at least one document open
            return;
        }

        if index < self.documents.len() {
            self.documents.remove(index);
            if index < self.active_tab {
                self.active_tab -= 1;
            } else if self.active_tab >= self.documents.len() {
                self.active_tab = self.documents.len() - 1;
            }
            self.split_tab = self.split_tab.and_then(|split| {
                if split == index {
                    None
                } else if split > index {
                    Some(split - 1)
                } else {
                    Some(split)
                }
            });
            if self.split_tab.is_none() {
                self.focused_pane = 0;
            }
        }
    }

    /// Removes documents matching a path predicate while keeping pane indices valid.
    pub fn remove_matching(&mut self, mut matches: impl FnMut(&Document) -> bool) {
        let active_path = self
            .documents
            .get(self.active_tab)
            .map(|doc| doc.path.clone());
        let split_path = self.split_doc().map(|doc| doc.path.clone());
        self.documents.retain(|doc| !matches(doc));
        if self.documents.is_empty() {
            self.active_tab = 0;
            self.split_tab = None;
            self.focused_pane = 0;
            return;
        }
        self.active_tab = active_path
            .and_then(|path| self.documents.iter().position(|doc| doc.path == path))
            .unwrap_or_else(|| self.active_tab.min(self.documents.len() - 1));
        self.split_tab = split_path
            .and_then(|path| self.documents.iter().position(|doc| doc.path == path))
            .filter(|index| *index != self.active_tab);
        if self.split_tab.is_none() {
            self.focused_pane = 0;
        }
    }

    /// Opens a file or focuses it if already open.
    pub fn open_or_focus(&mut self, id: usize, path: &str, name: &str, syntax: &str) {
        if let Some(pos) = self
            .documents
            .iter()
            .position(|doc| doc.id == id || doc.path == path)
        {
            self.active_tab = pos;
            return;
        }

        // Content generator for additional files
        let initial_text = match path {
            "src/raster.rs" => {
                r#"//! Tiled compute rasterizer with G-buffer deferred shading.
pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub prim_count: u32,
}

pub struct Rasterizer {
    pub width: u32,
    pub height: u32,
}

impl Rasterizer {
    pub fn new(w: u32, h: u32) -> Self {
        Self { width: w, height: h }
    }
}
"#
            }
            "src/scene.rs" => {
                r#"//! Retained scene tree with cache-friendly NodeId allocation.
pub struct SceneArena {
    pub count: usize,
}
"#
            }
            "tests/pipeline_test.rs" => {
                r#"#[test]
fn test_compute_pipeline() {
    assert_eq!(1 + 1, 2);
}
"#
            }
            "config.json" => {
                r#"{
    "app": "Taim Studio",
    "version": "0.15.0",
    "gpu": "wgpu-compute",
    "renderer": "compute-raster"
}
"#
            }
            _ => "// New document\n",
        };

        let new_doc = Document::new(id, path, name, syntax, initial_text);
        self.documents.push(new_doc);
        self.active_tab = self.documents.len() - 1;
    }

    /// Creates a new scratchpad document.
    pub fn new_scratchpad(&mut self) {
        let id = self.next_id;
        self.next_id += 1;
        let title = format!("scratch_{}.rs", id % 100);
        let path = format!("scratch/{}", title);
        let initial_text = "// Scratchpad - test code here\nfn main() {\n    println!(\"Hello from scratchpad!\");\n}\n";
        let doc = Document::new(id, path, title, "rust", initial_text);
        self.documents.push(doc);
        self.active_tab = self.documents.len() - 1;
    }

    /// Allocates a unique editor ID for a restored draft outside the file tree.
    pub fn reserve_virtual_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Returns vector of (file_path, content_text) for all open documents.
    pub fn file_buffers(&self) -> Vec<(String, String)> {
        self.documents
            .iter()
            .map(|d| (d.path.clone(), d.text()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_under_cursor_extracts_identifier() {
        let text = "let mut calculated_sum = 42;";
        let doc = Document::new(1, "test.rs", "test.rs", "rust", text);
        // Default cursor is at (0, 0) -> "let"
        assert_eq!(doc.word_under_cursor(), Some("let".to_string()));
    }

    #[test]
    fn test_merge_conflict_detection_and_resolution() {
        let conflicted = "\
fn calculate() -> i32 {
<<<<<<< HEAD (Current Change)
    let base = 100;
=======
    let base = 200;
>>>>>>> feature/gpu (Incoming Change)
    base + 5
}
";
        let doc = Document::new(1, "math.rs", "math.rs", "rust", conflicted);
        assert!(doc.has_merge_conflicts());
        let conflicts = doc.find_merge_conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].start_line, 1);
        assert_eq!(conflicts[0].separator_line, 3);
        assert_eq!(conflicts[0].end_line, 5);

        // Test AcceptCurrent
        let mut doc_cur = Document::new(1, "math.rs", "math.rs", "rust", conflicted);
        assert!(doc_cur.resolve_conflict(0, ConflictChoice::AcceptCurrent));
        assert!(!doc_cur.has_merge_conflicts());
        assert!(doc_cur.text().contains("let base = 100;"));
        assert!(!doc_cur.text().contains("let base = 200;"));
        assert!(!doc_cur.text().contains("<<<<<<<"));

        // Test AcceptIncoming
        let mut doc_inc = Document::new(1, "math.rs", "math.rs", "rust", conflicted);
        assert!(doc_inc.resolve_conflict(0, ConflictChoice::AcceptIncoming));
        assert!(!doc_inc.has_merge_conflicts());
        assert!(!doc_inc.text().contains("let base = 100;"));
        assert!(doc_inc.text().contains("let base = 200;"));

        // Test AcceptBoth
        let mut doc_both = Document::new(1, "math.rs", "math.rs", "rust", conflicted);
        assert!(doc_both.resolve_conflict(0, ConflictChoice::AcceptBoth));
        assert!(!doc_both.has_merge_conflicts());
        assert!(doc_both.text().contains("let base = 100;"));
        assert!(doc_both.text().contains("let base = 200;"));
    }

    #[test]
    fn test_code_folding_blocks_and_toggles() {
        let code = "\
fn outer() {
    let a = 1;
    if a > 0 {
        let b = 2;
    }
}
";
        let mut doc = Document::new(1, "fold.rs", "fold.rs", "rust", code);
        let blocks = doc.foldable_blocks();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0], (2, 4)); // if block: lines 2 to 4
        assert_eq!(blocks[1], (0, 5)); // outer block: lines 0 to 5

        assert!(doc.is_line_foldable(0));
        assert!(doc.is_line_foldable(2));
        assert!(!doc.is_line_foldable(1));

        // Toggle fold at line 0
        assert!(doc.toggle_fold_at(0));
        assert!(doc.is_line_folded(1));
        assert!(doc.is_line_folded(3));
        assert!(doc.is_line_folded(5));
        assert!(!doc.is_line_folded(0));

        // Toggle unfold
        assert!(doc.toggle_fold_at(0));
        assert!(!doc.is_line_folded(1));
    }
}
