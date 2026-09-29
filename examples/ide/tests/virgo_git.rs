use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use taim_ide::{DiffLineKind, Message, Studio};

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn repository() -> std::path::PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "virgo-git-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.name", "Virgo Test"]);
    git(&root, &["config", "user.email", "virgo@example.invalid"]);
    root
}

#[test]
fn workspace_git_status_stage_diff_and_commit_match_cli() {
    let root = repository();
    std::fs::write(root.join("main.rs"), "fn old() {}\n").unwrap();
    git(&root, &["add", "main.rs"]);
    git(&root, &["commit", "-qm", "initial"]);
    std::fs::write(root.join("main.rs"), "fn new() {}\n").unwrap();
    std::fs::write(root.join("notes.txt"), "new file\n").unwrap();

    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    assert!(ide.git.repo_root.is_some());
    assert_eq!(ide.git.unstaged_changes.len(), 2);
    assert!(ide.git.staged_changes.is_empty());

    let _ = ide.update(Message::GitToggleDiff("main.rs".into()));
    assert!(ide.git.is_diff_view_open);
    assert!(
        ide.git
            .diff_lines
            .iter()
            .any(|line| line.kind == DiffLineKind::Deletion)
    );
    assert!(
        ide.git
            .diff_lines
            .iter()
            .any(|line| line.kind == DiffLineKind::Addition)
    );

    let _ = ide.update(Message::GitStageFile("main.rs".into()));
    assert_eq!(
        git(&root, &["diff", "--cached", "--name-only"]).trim(),
        "main.rs"
    );
    assert!(
        ide.git
            .staged_changes
            .iter()
            .any(|change| change.file_path == "main.rs")
    );

    let _ = ide.update(Message::GitUnstageFile("main.rs".into()));
    assert!(git(&root, &["diff", "--cached", "--name-only"]).is_empty());
    let _ = ide.update(Message::GitStageAll);
    assert_eq!(ide.git.staged_changes.len(), 2);
    let _ = ide.update(Message::GitCommitMessageChanged("real commit".into()));
    let _ = ide.update(Message::GitCommit);
    assert_eq!(
        git(&root, &["log", "-1", "--pretty=%s"]).trim(),
        "real commit"
    );
    assert!(git(&root, &["status", "--porcelain"]).is_empty());
    assert_eq!(ide.git.total_changes(), 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unborn_repository_can_unstage_without_deleting_worktree_file() {
    let root = repository();
    std::fs::write(root.join("draft.rs"), "fn draft() {}\n").unwrap();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let _ = ide.update(Message::GitStageFile("draft.rs".into()));
    assert_eq!(ide.git.staged_changes.len(), 1);
    let _ = ide.update(Message::GitUnstageAll);
    assert!(ide.git.staged_changes.is_empty());
    assert!(root.join("draft.rs").exists());
    assert!(
        ide.git
            .unstaged_changes
            .iter()
            .any(|change| change.file_path == "draft.rs")
    );
    std::fs::remove_dir_all(root).unwrap();
}
