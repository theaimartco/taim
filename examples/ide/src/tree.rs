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
    /// Rebuilds the explorer while keeping opened folders and the selected file.
    pub fn rescan(&mut self, root_path: &std::path::Path) {
        fn opened(node: &FileNode, paths: &mut std::collections::HashSet<String>) {
            if node.is_dir {
                if node.is_expanded {
                    paths.insert(node.path.clone());
                }
                for child in &node.children {
                    opened(child, paths);
                }
            }
        }
        fn restore(node: &mut FileNode, paths: &std::collections::HashSet<String>) {
            if node.is_dir {
                node.is_expanded = paths.contains(&node.path);
                for child in &mut node.children {
                    restore(child, paths);
                }
            }
        }
        let selected = self
            .selected_id
            .and_then(|id| self.find(id))
            .map(|node| node.path.clone());
        let mut expanded = std::collections::HashSet::new();
        opened(&self.root, &mut expanded);
        let mut next = Self::from_path(root_path);
        restore(&mut next.root, &expanded);
        next.selected_id =
            selected.and_then(|path| next.root.find_by_path(&path).map(|node| node.id));
        *self = next;
    }

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
            if depth > 15 {
                return Vec::new();
            }
            let read_dir = match std::fs::read_dir(path) {
                Ok(rd) => rd,
                Err(_) => return Vec::new(),
            };

            let mut valid_entries = Vec::new();
            for entry in read_dir.flatten() {
                let file_name = entry.file_name();
                let name_bytes = file_name.as_encoded_bytes();
                if name_bytes == b".git"
                    || name_bytes == b".virgo-trash"
                    || name_bytes == b".DS_Store"
                    || name_bytes == b"target"
                    || name_bytes == b"node_modules"
                {
                    continue;
                }
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                let is_dir = file_type.is_dir();
                let is_file = file_type.is_file();
                if !is_dir && !is_file {
                    continue;
                }
                let name = file_name.to_string_lossy().into_owned();
                valid_entries.push((name, entry.path(), is_dir, is_file));
            }

            valid_entries.sort_unstable_by(|a, b| (!a.2).cmp(&(!b.2)).then_with(|| a.0.cmp(&b.0)));

            let mut entries = Vec::with_capacity(valid_entries.len());
            for (name, path, is_dir, is_file) in valid_entries {
                let rel_path = if rel_prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", rel_prefix, name)
                };

                *next_id += 1;
                let id = *next_id;

                if is_dir {
                    let children = scan_dir(&path, &rel_path, depth + 1, next_id);
                    let is_expanded = depth < 1;
                    entries.push(FileNode::dir(id, name, rel_path, is_expanded, children));
                } else if is_file {
                    let syntax = syntax_for_path(&path);
                    entries.push(FileNode::file(id, name, rel_path, syntax));
                }
            }
            entries
        }

        let children = scan_dir(root_path, "", 0, &mut next_id);
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

/// Returns the editor syntax for a file, including files outside the visible tree depth.
pub fn syntax_for_path(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("rs") => "rust",
        Some("toml") => "toml",
        Some("wgsl" | "c" | "h" | "cpp") => "c",
        Some("md") => "markdown",
        Some("json") => "json",
        _ => "text",
    }
}
