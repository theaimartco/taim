//! Language Server Protocol (LSP) & IntelliSense Simulation for Taim Studio.
//! Provides Breadcrumbs hierarchy, symbol hover cards, and autocomplete popup state.

/// Kind of an autocomplete suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Keyword,
    Function,
    Struct,
    Enum,
    Trait,
    Variable,
    Module,
}

impl CompletionKind {
    pub fn icon(&self) -> &'static str {
        match self {
            CompletionKind::Keyword => "🔑",
            CompletionKind::Function => "ƒ",
            CompletionKind::Struct => "📦",
            CompletionKind::Enum => "⌥",
            CompletionKind::Trait => "✦",
            CompletionKind::Variable => "𝓍",
            CompletionKind::Module => "📁",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            CompletionKind::Keyword => "keyword",
            CompletionKind::Function => "function",
            CompletionKind::Struct => "struct",
            CompletionKind::Enum => "enum",
            CompletionKind::Trait => "trait",
            CompletionKind::Variable => "variable",
            CompletionKind::Module => "module",
        }
    }
}

/// A single autocomplete suggestion.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: String,
    pub documentation: String,
}

/// Autocomplete popover state.
#[derive(Debug, Clone)]
pub struct AutocompleteState {
    pub is_open: bool,
    pub query: String,
    pub selected_index: usize,
    pub items: Vec<CompletionItem>,
}

impl Default for AutocompleteState {
    fn default() -> Self {
        Self::new()
    }
}

impl AutocompleteState {
    pub fn new() -> Self {
        Self {
            is_open: false,
            query: String::new(),
            selected_index: 0,
            items: vec![
                CompletionItem {
                    label: "button".to_string(),
                    kind: CompletionKind::Function,
                    detail: "pub fn button<'a, Message>(content: impl Into<Element<'a, Message>>) -> Button<'a, Message>".to_string(),
                    documentation: "Constructs a clickable push-button element with hardware GPU hover effects.".to_string(),
                },
                CompletionItem {
                    label: "row".to_string(),
                    kind: CompletionKind::Function,
                    detail: "pub fn row<'a, Message>() -> Row<'a, Message>".to_string(),
                    documentation: "Constructs a horizontal layout flex container.".to_string(),
                },
                CompletionItem {
                    label: "column".to_string(),
                    kind: CompletionKind::Function,
                    detail: "pub fn column<'a, Message>() -> Column<'a, Message>".to_string(),
                    documentation: "Constructs a vertical layout flex container.".to_string(),
                },
                CompletionItem {
                    label: "container".to_string(),
                    kind: CompletionKind::Function,
                    detail: "pub fn container<'a, Message>(content: impl Into<Element<'a, Message>>) -> Container<'a, Message>".to_string(),
                    documentation: "Wraps an element with styling, padding, and alignment constraints.".to_string(),
                },
                CompletionItem {
                    label: "Theme".to_string(),
                    kind: CompletionKind::Enum,
                    detail: "pub enum Theme { Dark, Light, HighContrast, Custom(...) }".to_string(),
                    documentation: "Global styling theme contract for widgets and canvas elements.".to_string(),
                },
                CompletionItem {
                    label: "Element".to_string(),
                    kind: CompletionKind::Struct,
                    detail: "pub struct Element<'a, Message, Theme, Renderer>".to_string(),
                    documentation: "Generic retained UI node in the Taim scene graph.".to_string(),
                },
                CompletionItem {
                    label: "pub fn".to_string(),
                    kind: CompletionKind::Keyword,
                    detail: "pub fn name(args) -> RetType".to_string(),
                    documentation: "Defines a public function.".to_string(),
                },
            ],
        }
    }

    pub fn filtered(&self) -> Vec<&CompletionItem> {
        if self.query.is_empty() {
            self.items.iter().collect()
        } else {
            let q = self.query.to_lowercase();
            self.items.iter().filter(|i| i.label.to_lowercase().contains(&q)).collect()
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.selected_index = 0;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.query.clear();
    }

    pub fn select_next(&mut self) {
        let count = self.filtered().len();
        if count > 0 {
            self.selected_index = (self.selected_index + 1) % count;
        }
    }

    pub fn select_prev(&mut self) {
        let count = self.filtered().len();
        if count > 0 {
            self.selected_index = if self.selected_index == 0 { count - 1 } else { self.selected_index - 1 };
        }
    }
}

/// Symbol hover inspection info card.
#[derive(Debug, Clone)]
pub struct HoverInfo {
    pub is_open: bool,
    pub symbol: String,
    pub signature: String,
    pub doc: String,
}

impl Default for HoverInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl HoverInfo {
    pub fn new() -> Self {
        Self {
            is_open: false,
            symbol: "DocumentManager".to_string(),
            signature: "pub struct DocumentManager".to_string(),
            doc: "High-performance multi-tab document state manager with split pane and live in-editor find/replace support.".to_string(),
        }
    }

    pub fn show(&mut self, symbol: &str, sig: &str, doc: &str) {
        self.symbol = symbol.to_string();
        self.signature = sig.to_string();
        self.doc = doc.to_string();
        self.is_open = true;
    }

    pub fn hide(&mut self) {
        self.is_open = false;
    }
}

/// Resolves path breadcrumbs for display at top of editor (e.g. `taim-project > src > lib.rs > run()`).
pub fn resolve_breadcrumbs(file_path: &str) -> Vec<String> {
    let mut parts = vec!["taim-project".to_string()];
    for segment in file_path.split('/') {
        if !segment.is_empty() {
            parts.push(segment.to_string());
        }
    }
    // Append current symbol context
    if file_path.ends_with(".rs") {
        parts.push("fn run()".to_string());
    }
    parts
}
