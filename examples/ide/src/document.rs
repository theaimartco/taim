//! Multi-tab document manager and text editor state for Taim Studio.

use taim::widget::text_editor;

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
        }
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

    /// Performs an editing action on the document content.
    pub fn perform(&mut self, action: text_editor::Action) {
        let is_edit = matches!(action, text_editor::Action::Edit(_));
        self.content.perform(action);
        if is_edit {
            self.is_dirty = true;
        }
    }

    /// Returns the current (1-indexed) line and column for the cursor.
    pub fn cursor_position(&self) -> (usize, usize) {
        let cursor = self.content.cursor();
        (cursor.position.line + 1, cursor.position.column + 1)
    }

    /// Returns the line count of the document.
    pub fn line_count(&self) -> usize {
        self.content.line_count()
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
            if self.active_tab >= self.documents.len() {
                self.active_tab = self.documents.len().saturating_sub(1);
            }
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

    /// Returns vector of (file_path, content_text) for all open documents.
    pub fn file_buffers(&self) -> Vec<(String, String)> {
        self.documents
            .iter()
            .map(|d| (d.path.clone(), d.text()))
            .collect()
    }
}
