//! Project Explorer file tree model for Taim Studio.

/// A file or directory in the explorer tree.
#[derive(Debug, Clone)]
pub struct FileNode {
    /// Unique identifier for this node.
    pub id: usize,
    /// Display name (e.g. `main.rs` or `src`).
    pub name: String,
    /// Virtual relative path.
    pub path: String,
    /// Whether this node is a directory.
    pub is_dir: bool,
    /// Whether this directory is expanded (ignored for files).
    pub is_expanded: bool,
    /// Syntax highlighting language identifier for taim-highlighter.
    pub syntax: &'static str,
    /// Sub-nodes if this is a directory.
    pub children: Vec<FileNode>,
}

impl FileNode {
    /// Creates a leaf file node.
    pub fn file(
        id: usize,
        name: impl Into<String>,
        path: impl Into<String>,
        syntax: &'static str,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            path: path.into(),
            is_dir: false,
            is_expanded: false,
            syntax,
            children: Vec::new(),
        }
    }

    /// Creates a directory node.
    pub fn dir(
        id: usize,
        name: impl Into<String>,
        path: impl Into<String>,
        expanded: bool,
        children: Vec<FileNode>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            path: path.into(),
            is_dir: true,
            is_expanded: expanded,
            syntax: "",
            children,
        }
    }

    /// Returns the decorative icon or glyph for this node.
    pub fn icon(&self) -> &'static str {
        if self.is_dir {
            if self.is_expanded { "▾" } else { "▸" }
        } else {
            match self.syntax {
                "rust" => "◈",
                "toml" => "▤",
                "markdown" => "≡",
                "json" => "{ }",
                "c" => "⚡",
                _ => "◇",
            }
        }
    }

    /// Recursively toggles directory expansion matching `target_id`.
    pub fn toggle_recursive(&mut self, target_id: usize) -> bool {
        if self.id == target_id {
            if self.is_dir {
                self.is_expanded = !self.is_expanded;
                return true;
            }
            return false;
        }

        for child in &mut self.children {
            if child.toggle_recursive(target_id) {
                return true;
            }
        }

        false
    }

    /// Recursively searches for a node by id.
    pub fn find(&self, target_id: usize) -> Option<&FileNode> {
        if self.id == target_id {
            return Some(self);
        }

        for child in &self.children {
            if let Some(found) = child.find(target_id) {
                return Some(found);
            }
        }

        None
    }

    /// Recursively searches for a node by relative path.
    pub fn find_by_path(&self, target_path: &str) -> Option<&FileNode> {
        if self.path == target_path {
            return Some(self);
        }

        for child in &self.children {
            if let Some(found) = child.find_by_path(target_path) {
                return Some(found);
            }
        }

        None
    }
}

/// The project tree container.
#[derive(Debug, Clone)]
pub struct FileTree {
    /// Root directory node.
    pub root: FileNode,
    /// Currently selected node id in explorer.
    pub selected_id: Option<usize>,
}

impl FileTree {
    /// Creates a rich, realistic default project tree representing a Taim workspace.
    pub fn default_project() -> Self {
        let src = FileNode::dir(
            100,
            "src",
            "src",
            true,
            vec![
                FileNode::file(10, "main.rs", "src/main.rs", "rust"),
                FileNode::file(11, "raster.rs", "src/raster.rs", "rust"),
                FileNode::file(12, "scene.rs", "src/scene.rs", "rust"),
            ],
        );

        let shaders = FileNode::dir(
            200,
            "shaders",
            "shaders",
            true,
            vec![FileNode::file(
                20,
                "compute_raster.wgsl",
                "shaders/compute_raster.wgsl",
                "c",
            )],
        );

        let tests = FileNode::dir(
            300,
            "tests",
            "tests",
            false,
            vec![FileNode::file(
                30,
                "pipeline_test.rs",
                "tests/pipeline_test.rs",
                "rust",
            )],
        );

        let root = FileNode::dir(
            0,
            "taim-project",
            "",
            true,
            vec![
                src,
                shaders,
                tests,
                FileNode::file(1, "Cargo.toml", "Cargo.toml", "toml"),
                FileNode::file(2, "README.md", "README.md", "markdown"),
                FileNode::file(3, "config.json", "config.json", "json"),
            ],
        );

        Self {
            root,
            selected_id: Some(10), // main.rs selected initially
        }
    }

    /// Scans a directory from disk and builds a live FileTree.
    pub fn from_path(root_path: &std::path::Path) -> Self {
        let mut next_id = 1000;
        let root_name = root_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_string();

        fn scan_dir(
            path: &std::path::Path,
            rel_prefix: &str,
            depth: usize,
            next_id: &mut usize,
        ) -> Vec<FileNode> {
            if depth > 3 {
                return Vec::new();
            }
            let read_dir = match std::fs::read_dir(path) {
                Ok(rd) => rd,
                Err(_) => return Vec::new(),
            };

            let mut sorted_entries: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
            sorted_entries.sort_by_key(|e| {
                (
                    e.file_type().map(|t| !t.is_dir()).unwrap_or(true),
                    e.file_name(),
                )
            });

            let mut entries = Vec::new();
            for entry in sorted_entries {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name == "target" || name == "node_modules" {
                    continue;
                }
                let file_type = match entry.file_type() {
                    Ok(ft) => ft,
                    Err(_) => continue,
                };
                let rel_path = if rel_prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", rel_prefix, name)
                };

                *next_id += 1;
                let id = *next_id;

                if file_type.is_dir() {
                    let children = scan_dir(&entry.path(), &rel_path, depth + 1, next_id);
                    let is_expanded = depth < 1;
                    entries.push(FileNode::dir(id, name, rel_path, is_expanded, children));
                } else if file_type.is_file() {
                    let path_buf = entry.path();
                    let ext = path_buf.extension().and_then(|e| e.to_str()).unwrap_or("");
                    let syntax = match ext {
                        "rs" => "rust",
                        "toml" => "toml",
                        "wgsl" | "c" | "h" | "cpp" => "c",
                        "md" => "markdown",
                        "json" => "json",
                        _ => "text",
                    };
                    entries.push(FileNode::file(id, name, rel_path, syntax));
                }
            }
            entries
        }

        let children = scan_dir(root_path, "", 0, &mut next_id);
        if children.is_empty() {
            return Self::default_project();
        }

        let first_file_id = children
            .iter()
            .find_map(|c| {
                if !c.is_dir {
                    Some(c.id)
                } else {
                    c.children.iter().find(|gc| !gc.is_dir).map(|gc| gc.id)
                }
            })
            .unwrap_or(1001);

        let root = FileNode::dir(0, root_name, "", true, children);
        Self {
            root,
            selected_id: Some(first_file_id),
        }
    }

    /// Toggles folder expansion by id.
    pub fn toggle(&mut self, node_id: usize) -> bool {
        self.root.toggle_recursive(node_id)
    }

    /// Finds a node by id.
    pub fn find(&self, node_id: usize) -> Option<&FileNode> {
        self.root.find(node_id)
    }
}
