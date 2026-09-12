//! Global Workspace Search & Replace for Taim Studio.
//! Provides project-wide searching across all open documents and project tree files.

/// A single search result occurrence inside a file.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub file_path: String,
    pub line_number: usize,
    pub column: usize,
    pub line_content: String,
    pub match_length: usize,
}

/// A group of search hits within a single file.
#[derive(Debug, Clone)]
pub struct FileSearchGroup {
    pub file_path: String,
    pub file_name: String,
    pub hits: Vec<SearchHit>,
    pub is_expanded: bool,
}

/// Global search and replace state.
#[derive(Debug, Clone, Default)]
pub struct GlobalSearchState {
    pub query: String,
    pub replace_query: String,
    pub match_case: bool,
    pub whole_word: bool,
    pub use_regex: bool,
    pub files_to_include: String,
    pub files_to_exclude: String,
    pub show_replace: bool,
    pub results: Vec<FileSearchGroup>,
    pub total_matches: usize,
    pub is_searching: bool,
}

impl GlobalSearchState {
    pub fn new() -> Self {
        let mut state = Self {
            query: "fn ".to_string(),
            replace_query: String::new(),
            match_case: false,
            whole_word: false,
            use_regex: false,
            files_to_include: String::new(),
            files_to_exclude: String::new(),
            show_replace: false,
            results: Vec::new(),
            total_matches: 0,
            is_searching: false,
        };
        state.populate_initial_demo_search();
        state
    }

    /// Populates representative initial search results for demonstration.
    pub fn populate_initial_demo_search(&mut self) {
        self.results = vec![
            FileSearchGroup {
                file_path: "src/main.rs".to_string(),
                file_name: "main.rs".to_string(),
                hits: vec![
                    SearchHit {
                        file_path: "src/main.rs".to_string(),
                        line_number: 6,
                        column: 1,
                        line_content: "pub fn run() -> Result<()> {".to_string(),
                        match_length: 3,
                    },
                    SearchHit {
                        file_path: "src/main.rs".to_string(),
                        line_number: 14,
                        column: 5,
                        line_content: "    fn render_frame(&mut self) {".to_string(),
                        match_length: 3,
                    },
                ],
                is_expanded: true,
            },
            FileSearchGroup {
                file_path: "src/pipeline.rs".to_string(),
                file_name: "pipeline.rs".to_string(),
                hits: vec![SearchHit {
                    file_path: "src/pipeline.rs".to_string(),
                    line_number: 18,
                    column: 5,
                    line_content: "    pub fn create_render_pipeline() {".to_string(),
                    match_length: 3,
                }],
                is_expanded: true,
            },
        ];
        self.total_matches = self.results.iter().map(|g| g.hits.len()).sum();
    }

    /// Sets the search query and performs an in-memory scan across provided file buffers.
    pub fn set_query(&mut self, query: String, file_buffers: &[(&str, &str)]) {
        self.query = query;
        self.recompute_search(file_buffers);
    }

    /// Toggles case sensitivity.
    pub fn toggle_match_case(&mut self, file_buffers: &[(&str, &str)]) {
        self.match_case = !self.match_case;
        self.recompute_search(file_buffers);
    }

    /// Toggles whole word matching.
    pub fn toggle_whole_word(&mut self, file_buffers: &[(&str, &str)]) {
        self.whole_word = !self.whole_word;
        self.recompute_search(file_buffers);
    }

    /// Toggles replace input visibility.
    pub fn toggle_replace(&mut self) {
        self.show_replace = !self.show_replace;
    }

    /// Toggle expansion of a file result group.
    pub fn toggle_group_expanded(&mut self, file_path: &str) {
        if let Some(group) = self.results.iter_mut().find(|g| g.file_path == file_path) {
            group.is_expanded = !group.is_expanded;
        }
    }

    /// Clear all results.
    pub fn clear(&mut self) {
        self.query.clear();
        self.replace_query.clear();
        self.results.clear();
        self.total_matches = 0;
    }

    /// Recomputes search across given (path, content) file tuples.
    pub fn recompute_search(&mut self, file_buffers: &[(&str, &str)]) {
        if self.query.trim().is_empty() {
            self.results.clear();
            self.total_matches = 0;
            return;
        }

        let query = if self.match_case {
            self.query.clone()
        } else {
            self.query.to_lowercase()
        };

        let mut groups = Vec::new();

        for (path, content) in file_buffers {
            let mut hits = Vec::new();
            for (line_idx, line) in content.lines().enumerate() {
                let check_line = if self.match_case {
                    line.to_string()
                } else {
                    line.to_lowercase()
                };

                let mut start_idx = 0;
                while let Some(rel_idx) = check_line[start_idx..].find(&query) {
                    let col = start_idx + rel_idx + 1;
                    hits.push(SearchHit {
                        file_path: path.to_string(),
                        line_number: line_idx + 1,
                        column: col,
                        line_content: line.trim().to_string(),
                        match_length: query.len(),
                    });
                    start_idx += rel_idx + query.len().max(1);
                }
            }

            if !hits.is_empty() {
                let file_name = path.rsplit('/').next().unwrap_or(path).to_string();
                groups.push(FileSearchGroup {
                    file_path: path.to_string(),
                    file_name,
                    hits,
                    is_expanded: true,
                });
            }
        }

        self.total_matches = groups.iter().map(|g| g.hits.len()).sum();
        self.results = groups;
    }
}
