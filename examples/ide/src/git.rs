//! Source Control & Git Integration for Taim Studio.
//! Provides branch switching, staging, commit messages, and side-by-side diffing.

use std::path::{Path, PathBuf};
use std::process::Command;

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
    pub repo_root: Option<PathBuf>,
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
    pub fn empty() -> Self {
        let mut state = Self::new();
        state.active_branch = "No Git repository".to_string();
        state.branches.clear();
        state.staged_changes.clear();
        state.unstaged_changes.clear();
        state.commit_history.clear();
        state.diff_lines.clear();
        state.diff_file = None;
        state
    }

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
            repo_root: None,
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
                content: "use taim_widget::{button, row, column, text};".to_string(),
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

fn git_output(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| format!("Could not run Git: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Parses the output of `git blame --porcelain` into a human-readable summary.
pub fn parse_porcelain_blame(bytes: &[u8]) -> Option<String> {
    let content = String::from_utf8_lossy(bytes);
    let mut lines = content.lines();
    let first_line = lines.next()?;
    let commit_hash = first_line.split_whitespace().next().unwrap_or("");

    let mut author = None;
    let mut summary = None;

    for line in lines {
        if let Some(rest) = line.strip_prefix("author ") {
            author = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("summary ") {
            summary = Some(rest.trim().to_string());
        }
        if author.is_some() && summary.is_some() {
            break;
        }
    }

    let author_str = author.unwrap_or_else(|| "Unknown".to_string());
    let summary_str = summary.unwrap_or_else(|| "No commit summary".to_string());

    if author_str == "Not Committed Yet" || commit_hash.starts_with("0000000") {
        Some("You • Uncommitted changes".to_string())
    } else {
        let short_hash = &commit_hash[..7.min(commit_hash.len())];
        if short_hash.is_empty() {
            Some(format!("{author_str} • {summary_str}"))
        } else {
            Some(format!("{author_str} ({short_hash}) • {summary_str}"))
        }
    }
}

/// Reads git blame information for a specific 1-based line number of a file.
pub fn read_blame_for_line(repo_root: &Path, file_path: &str, line_num: usize) -> Option<String> {
    let line_arg = format!("{},{}", line_num.max(1), line_num.max(1));
    let output = git_output(
        repo_root,
        &["blame", "-L", &line_arg, "--porcelain", file_path],
    )
    .ok()?;
    parse_porcelain_blame(&output)
}

fn status_kind(code: u8) -> GitFileStatus {
    match code {
        b'A' => GitFileStatus::Added,
        b'D' => GitFileStatus::Deleted,
        b'?' => GitFileStatus::Untracked,
        _ => GitFileStatus::Modified,
    }
}

fn parse_status_and_branch(bytes: &[u8]) -> (Option<String>, Vec<GitChange>, Vec<GitChange>) {
    let mut branch = None;
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut entries = bytes.split(|byte| *byte == 0);
    while let Some(entry) = entries.next() {
        if entry.starts_with(b"## ") {
            let header = String::from_utf8_lossy(&entry[3..]);
            let name = header.split("...").next().unwrap_or("").trim();
            let clean = if let Some(stripped) = name.strip_prefix("No commits yet on ") {
                stripped
            } else if let Some(stripped) = name.strip_prefix("Initial commit on ") {
                stripped
            } else {
                name
            };
            if !clean.is_empty() {
                branch = Some(clean.to_string());
            }
            continue;
        }
        if entry.len() < 4 || entry[2] != b' ' {
            continue;
        }
        let path = String::from_utf8_lossy(&entry[3..]).into_owned();
        let first = entry[0];
        let second = entry[1];
        if first == b'R' || first == b'C' || second == b'R' || second == b'C' {
            let _ = entries.next();
        }
        if first == b'?' && second == b'?' {
            unstaged.push(GitChange {
                file_path: path,
                status: GitFileStatus::Untracked,
            });
            continue;
        }
        if first != b' ' && first != b'?' {
            staged.push(GitChange {
                file_path: path.clone(),
                status: status_kind(first),
            });
        }
        if second != b' ' && second != b'?' {
            unstaged.push(GitChange {
                file_path: path,
                status: status_kind(second),
            });
        }
    }
    (branch, staged, unstaged)
}

fn parse_hunk_start(token: &str) -> Option<usize> {
    token.get(1..)?.split(',').next()?.parse::<usize>().ok()
}

fn parse_diff(bytes: &[u8]) -> Vec<DiffLine> {
    let content = String::from_utf8_lossy(bytes);
    let mut lines = Vec::new();
    let (mut old, mut new) = (0, 0);
    let mut in_hunk = false;
    for line in content.lines() {
        if let Some(header) = line.strip_prefix("@@ ") {
            let mut parts = header.split_whitespace();
            let Some(old_start) = parts.next().and_then(parse_hunk_start) else {
                continue;
            };
            let Some(new_start) = parts.next().and_then(parse_hunk_start) else {
                continue;
            };
            old = old_start;
            new = new_start;
            in_hunk = true;
            continue;
        }
        if !in_hunk {
            continue;
        }
        let Some((prefix, body)) = line.as_bytes().split_first() else {
            continue;
        };
        let body = String::from_utf8_lossy(body).into_owned();
        match prefix {
            b' ' => {
                lines.push(DiffLine {
                    kind: DiffLineKind::Context,
                    old_line_num: Some(old),
                    new_line_num: Some(new),
                    content: body,
                });
                old += 1;
                new += 1;
            }
            b'-' => {
                lines.push(DiffLine {
                    kind: DiffLineKind::Deletion,
                    old_line_num: Some(old),
                    new_line_num: None,
                    content: body,
                });
                old += 1;
            }
            b'+' => {
                lines.push(DiffLine {
                    kind: DiffLineKind::Addition,
                    old_line_num: None,
                    new_line_num: Some(new),
                    content: body,
                });
                new += 1;
            }
            _ => {}
        }
    }
    lines
}

fn read_branch_from_head(root: &Path) -> Option<String> {
    let head = root.join(".git/HEAD");
    if let Ok(content) = std::fs::read_to_string(head) {
        let trimmed = content.trim();
        if let Some(branch) = trimmed.strip_prefix("ref: refs/heads/") {
            return Some(branch.to_string());
        }
        if !trimmed.is_empty() && trimmed.len() >= 7 && !trimmed.contains(' ') {
            return Some(trimmed[..7].to_string());
        }
    }
    None
}

impl GitState {
    /// Loads real repository state for a workspace. A non-repository stays empty.
    pub fn for_workspace(root: &Path) -> Self {
        let repo_root = if root.join(".git").exists() {
            Some(root.to_path_buf())
        } else {
            let Ok(top) = git_output(root, &["rev-parse", "--show-toplevel"]) else {
                return Self::empty();
            };
            let repo_root = PathBuf::from(String::from_utf8_lossy(&top).trim());
            repo_root.canonicalize().ok()
        };
        let Some(repo_root) = repo_root else {
            return Self::empty();
        };
        let mut state = Self::empty();
        if let Some(branch) = read_branch_from_head(&repo_root) {
            state.active_branch = branch;
        }
        state.repo_root = Some(repo_root);
        let _ = state.refresh_workspace();
        state
    }

    pub fn refresh_workspace(&mut self) -> Result<(), String> {
        let root = self.repo_root.as_ref().ok_or("No Git repository")?;
        let status = git_output(
            root,
            &[
                "--no-optional-locks",
                "-c",
                "core.preloadindex=true",
                "-c",
                "status.relativePaths=false",
                "status",
                "-b",
                "--no-ahead-behind",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
            ],
        )?;
        let (branch, staged, unstaged) = parse_status_and_branch(&status);
        if let Some(branch) = branch {
            self.active_branch = branch;
        } else if let Some(head_branch) = read_branch_from_head(root) {
            self.active_branch = head_branch;
        } else if self.active_branch.is_empty() || self.active_branch == "No Git repository" {
            let fallback_branch = git_output(root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
                .or_else(|_| git_output(root, &["rev-parse", "--short", "HEAD"]))
                .map(|b| String::from_utf8_lossy(&b).trim().to_string())
                .unwrap_or_else(|_| "HEAD".to_string());
            self.active_branch = fallback_branch;
        }
        self.staged_changes = staged;
        self.unstaged_changes = unstaged;
        Ok(())
    }

    pub fn refresh_commit_history(&mut self) {
        if let Some(root) = self.repo_root.as_ref()
            && let Ok(output) = git_output(root, &["log", "-5", "--pretty=format:%s"])
        {
            self.commit_history = String::from_utf8_lossy(&output)
                .lines()
                .map(str::to_string)
                .collect();
        }
    }

    pub fn stage_workspace_file(&mut self, path: &str) -> Result<(), String> {
        if !self
            .unstaged_changes
            .iter()
            .any(|change| change.file_path == path)
        {
            return Err("File is not an unstaged Git change".to_string());
        }
        let root = self.repo_root.as_ref().ok_or("No Git repository")?;
        git_output(root, &["add", "--", path])?;
        self.refresh_workspace()
    }

    pub fn unstage_workspace_file(&mut self, path: &str) -> Result<(), String> {
        if !self
            .staged_changes
            .iter()
            .any(|change| change.file_path == path)
        {
            return Err("File is not a staged Git change".to_string());
        }
        let root = self.repo_root.as_ref().ok_or("No Git repository")?;
        if git_output(root, &["rev-parse", "--verify", "HEAD"]).is_ok() {
            git_output(root, &["restore", "--staged", "--", path])?;
        } else {
            git_output(root, &["rm", "--cached", "--", path])?;
        }
        self.refresh_workspace()
    }

    pub fn stage_workspace_all(&mut self) -> Result<(), String> {
        let root = self.repo_root.as_ref().ok_or("No Git repository")?;
        git_output(root, &["add", "--all"])?;
        self.refresh_workspace()
    }

    pub fn unstage_workspace_all(&mut self) -> Result<(), String> {
        let root = self.repo_root.as_ref().ok_or("No Git repository")?;
        if git_output(root, &["rev-parse", "--verify", "HEAD"]).is_ok() {
            git_output(root, &["restore", "--staged", "--", "."])?;
        } else {
            git_output(root, &["rm", "--cached", "-r", "--", "."])?;
        }
        self.refresh_workspace()
    }

    pub fn commit_workspace(&mut self) -> Result<String, String> {
        let message = self.commit_message.trim();
        if message.is_empty() {
            return Err("Commit message cannot be empty".to_string());
        }
        if self.staged_changes.is_empty() {
            return Err("No staged changes to commit".to_string());
        }
        let root = self.repo_root.as_ref().ok_or("No Git repository")?;
        let output = git_output(root, &["commit", "-m", message])?;
        self.commit_message.clear();
        self.is_diff_view_open = false;
        self.refresh_workspace()?;
        self.refresh_commit_history();
        Ok(String::from_utf8_lossy(&output).trim().to_string())
    }

    pub fn toggle_workspace_diff(&mut self, path: &str) -> Result<(), String> {
        if path.is_empty() || (self.is_diff_view_open && self.diff_file.as_deref() == Some(path)) {
            self.is_diff_view_open = false;
            return Ok(());
        }
        let root = self.repo_root.as_ref().ok_or("No Git repository")?;
        let unstaged = self
            .unstaged_changes
            .iter()
            .find(|change| change.file_path == path);
        let staged = self
            .staged_changes
            .iter()
            .find(|change| change.file_path == path);
        let diff = if let Some(change) = unstaged {
            if change.status == GitFileStatus::Untracked {
                let candidate = root.join(path);
                let resolved = candidate
                    .canonicalize()
                    .map_err(|error| format!("Cannot preview {path}: {error}"))?;
                if !resolved.starts_with(root) || !resolved.is_file() {
                    return Err("Cannot preview a file outside the repository".to_string());
                }
                let size = resolved
                    .metadata()
                    .map_err(|error| format!("Cannot preview {path}: {error}"))?
                    .len();
                if size > 1_000_000 {
                    return Err("File is too large for diff preview".to_string());
                }
                let text = std::fs::read_to_string(&resolved)
                    .map_err(|error| format!("Cannot preview {path}: {error}"))?;
                text.lines()
                    .enumerate()
                    .map(|(index, content)| DiffLine {
                        kind: DiffLineKind::Addition,
                        old_line_num: None,
                        new_line_num: Some(index + 1),
                        content: content.to_string(),
                    })
                    .collect()
            } else {
                parse_diff(&git_output(root, &["diff", "--no-ext-diff", "--", path])?)
            }
        } else if staged.is_some() {
            parse_diff(&git_output(
                root,
                &["diff", "--cached", "--no-ext-diff", "--", path],
            )?)
        } else {
            return Err("File is not a Git change".to_string());
        };
        self.diff_file = Some(path.to_string());
        self.diff_lines = diff;
        self.is_diff_view_open = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_porcelain_blame_committed() {
        let sample = b"e82f48c08da6238b15e2193b2a095cd5dfcf6b95 1 1 1\nauthor Satpal Singh\nauthor-mail <satpal@example.com>\nauthor-time 1711200000\nauthor-tz +0530\ncommitter Satpal Singh\ncommitter-mail <satpal@example.com>\ncommitter-time 1711200000\ncommitter-tz +0530\nsummary feat(ide): add native GPU editor\nfilename src/lib.rs\n\tlet x = 42;\n";
        let parsed = parse_porcelain_blame(sample).unwrap();
        assert_eq!(
            parsed,
            "Satpal Singh (e82f48c) • feat(ide): add native GPU editor"
        );
    }

    #[test]
    fn test_parse_porcelain_blame_uncommitted() {
        let sample = b"0000000000000000000000000000000000000000 1 1 1\nauthor Not Committed Yet\nauthor-mail <not.committed.yet>\nsummary Not Committed Yet\nfilename src/lib.rs\n\tlet x = 42;\n";
        let parsed = parse_porcelain_blame(sample).unwrap();
        assert_eq!(parsed, "You • Uncommitted changes");
    }
}
