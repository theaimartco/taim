use regex::{Regex, RegexBuilder};
use std::sync::Arc;

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
    pub search_error: Option<String>,
    pub cached_buffers: Arc<Vec<(String, Arc<str>)>>,
    pub cached_fingerprint: u64,
    matcher: Option<Regex>,
    includes: Vec<Regex>,
    excludes: Vec<Regex>,
}

/// Keep the sidebar responsive when a short query matches thousands of lines.
pub const MAX_VISIBLE_MATCHES: usize = 200;

#[derive(Debug, Clone)]
pub struct ReplaceFileChange {
    pub path: String,
    pub before: String,
    pub after: String,
    pub replacements: usize,
    pub was_open: bool,
    pub was_dirty: bool,
    pub disk_snapshot: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ReplaceBatch {
    pub files: Vec<ReplaceFileChange>,
    pub total_replacements: usize,
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
            search_error: None,
            cached_buffers: Arc::new(Vec::new()),
            cached_fingerprint: 0,
            matcher: None,
            includes: Vec::new(),
            excludes: Vec::new(),
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
        self.matcher = None;
        self.search_error = None;
    }

    /// Recomputes search across given (path, content) file tuples.
    pub fn recompute_search(&mut self, file_buffers: &[(&str, &str)]) {
        self.results.clear();
        self.total_matches = 0;
        if !self.prepare_matcher() {
            return;
        }

        for (path, content) in file_buffers {
            self.append_file(path, content);
        }
    }

    /// Recomputes search using cached buffers and overlaying dirty/extra documents.
    pub fn recompute_search_cached(
        &mut self,
        dirty_docs: &[(&str, &str)],
        extra_docs: &[(&str, &str)],
    ) {
        self.results.clear();
        self.total_matches = 0;
        if !self.prepare_matcher() {
            return;
        }

        let cached = Arc::clone(&self.cached_buffers);
        for (path, content) in cached.iter() {
            if let Some((_, dirty_text)) = dirty_docs.iter().find(|(p, _)| *p == path.as_str()) {
                self.append_file(path, dirty_text);
            } else {
                self.append_file(path, content);
            }
        }
        for (path, content) in extra_docs {
            self.append_file(path, content);
        }
    }

    /// Adds results from one file without retaining the entire workspace in memory.
    pub fn append_file(&mut self, path: &str, content: &str) {
        let Some(matcher) = self.matcher.as_ref() else {
            return;
        };
        if (!self.includes.is_empty() && !self.includes.iter().any(|glob| glob.is_match(path)))
            || self.excludes.iter().any(|glob| glob.is_match(path))
        {
            return;
        }
        if !matcher.is_match(content) {
            return;
        }

        let mut visible_matches: usize = self.results.iter().map(|group| group.hits.len()).sum();
        let mut hits = Vec::new();
        let bytes = content.as_bytes();
        let mut line_number = 1;
        let mut line_start = 0;

        for found in matcher.find_iter(content) {
            let start = found.start();
            let end = found.end();
            if start == end {
                continue;
            }
            if self.whole_word {
                let word_char = |c: char| c.is_alphanumeric() || c == '_';
                let left_word = content[..start].chars().next_back().is_some_and(word_char);
                let right_word = content[end..].chars().next().is_some_and(word_char);
                if left_word || right_word {
                    continue;
                }
            }
            self.total_matches += 1;
            if visible_matches < MAX_VISIBLE_MATCHES {
                visible_matches += 1;
                while line_start < start {
                    if let Some(pos) = bytes[line_start..start].iter().position(|&b| b == b'\n') {
                        line_number += 1;
                        line_start += pos + 1;
                    } else {
                        break;
                    }
                }
                let line_end = bytes[start..]
                    .iter()
                    .position(|&b| b == b'\n')
                    .map(|pos| start + pos)
                    .unwrap_or(content.len());
                let line_content = &content[line_start..line_end];
                let column = content[line_start..start].chars().count() + 1;
                let match_length = content[start..end].chars().count();
                hits.push(SearchHit {
                    file_path: path.to_string(),
                    line_number,
                    column,
                    line_content: line_content.trim().to_string(),
                    match_length,
                });
            }
        }

        if !hits.is_empty() {
            let file_name = path.rsplit('/').next().unwrap_or(path).to_string();
            self.results.push(FileSearchGroup {
                file_path: path.to_string(),
                file_name,
                hits,
                is_expanded: true,
            });
        }
    }

    /// Compiles the active query and path globs once before scanning files.
    pub fn prepare_matcher(&mut self) -> bool {
        self.matcher = None;
        self.includes.clear();
        self.excludes.clear();
        self.search_error = None;
        if self.query.trim().is_empty() {
            return false;
        }
        let pattern = if self.use_regex {
            self.query.clone()
        } else {
            regex::escape(&self.query)
        };
        match RegexBuilder::new(&pattern)
            .multi_line(true)
            .case_insensitive(!self.match_case)
            .build()
        {
            Ok(matcher) => self.matcher = Some(matcher),
            Err(error) => {
                self.search_error = Some(format!("Invalid search pattern: {error}"));
                return false;
            }
        }
        for (input, include) in [
            (&self.files_to_include, true),
            (&self.files_to_exclude, false),
        ] {
            for glob in input
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
            {
                match compile_glob(glob) {
                    Ok(pattern) if include => self.includes.push(pattern),
                    Ok(pattern) => self.excludes.push(pattern),
                    Err(error) => {
                        self.search_error = Some(format!("Invalid file pattern: {error}"));
                        self.matcher = None;
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Produces replacement text using the same matcher and path filters as search.
    pub fn replace_text(&self, path: &str, content: &str) -> Result<(String, usize), String> {
        let matcher = self
            .matcher
            .as_ref()
            .ok_or_else(|| "Search pattern is not ready".to_string())?;
        if (!self.includes.is_empty() && !self.includes.iter().any(|glob| glob.is_match(path)))
            || self.excludes.iter().any(|glob| glob.is_match(path))
        {
            return Ok((content.to_string(), 0));
        }
        let mut output = String::with_capacity(content.len());
        let mut last = 0;
        let mut replacements = 0;
        for captures in matcher.captures_iter(content) {
            let Some(found) = captures.get(0) else {
                continue;
            };
            if found.start() == found.end() {
                continue;
            }
            let word_char = |character: char| character.is_alphanumeric() || character == '_';
            let left_word = content[..found.start()]
                .chars()
                .next_back()
                .is_some_and(word_char);
            let right_word = content[found.end()..].chars().next().is_some_and(word_char);
            if self.whole_word && (left_word || right_word) {
                continue;
            }
            output.push_str(&content[last..found.start()]);
            if self.use_regex {
                captures.expand(&self.replace_query, &mut output);
            } else {
                output.push_str(&self.replace_query);
            }
            last = found.end();
            replacements += 1;
        }
        if replacements == 0 {
            return Ok((content.to_string(), 0));
        }
        output.push_str(&content[last..]);
        Ok((output, replacements))
    }
}

fn compile_glob(glob: &str) -> Result<Regex, regex::Error> {
    let mut pattern = String::new();
    if glob.contains('/') {
        pattern.push('^');
    } else {
        pattern.push_str("(?:^|/)");
    }
    let mut chars = glob.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                if chars.peek() == Some(&'/') {
                    chars.next();
                    pattern.push_str("(?:.*/)?");
                } else {
                    pattern.push_str(".*");
                }
            }
            '*' => pattern.push_str("[^/]*"),
            '?' => pattern.push_str("[^/]"),
            _ => pattern.push_str(&regex::escape(&ch.to_string())),
        }
    }
    if glob.contains('/') {
        pattern.push('$');
    } else {
        pattern.push_str("(?:$|/)");
    }
    Regex::new(&pattern)
}
