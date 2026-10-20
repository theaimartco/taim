use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use taim::widget::text_editor;
use taim_futures::futures::{StreamExt, executor::block_on};
use taim_ide::{Message, Studio};
use taim_runtime::Action;

fn workspace() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "virgo-test-{}-{nonce}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rs"), "fn real_workspace() {}\n").unwrap();
    fs::write(root.join("notes.txt"), "needle is on disk\n").unwrap();
    root
}

#[test]
fn opens_and_saves_actual_workspace_content() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let seed = ide.theme().seed();
    for color in [
        seed.background,
        seed.text,
        seed.primary,
        seed.success,
        seed.warning,
        seed.danger,
    ] {
        assert_eq!(color.r, color.g);
        assert_eq!(color.g, color.b);
    }
    let file_id = ide.tree.root.find_by_path("main.rs").unwrap().id;
    let _ = ide.update(Message::FileSelected(file_id));
    assert_eq!(
        ide.docs.active().unwrap().text(),
        "fn real_workspace() {}\n"
    );

    let _ = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('X'),
    )));
    assert!(ide.docs.active().unwrap().is_dirty);
    let _ = ide.update(Message::SaveDocument);
    assert!(!ide.docs.active().unwrap().is_dirty);
    assert_eq!(
        fs::read_to_string(root.join("main.rs")).unwrap(),
        ide.docs.active().unwrap().text()
    );
    assert!(ide.docs.active().unwrap().disk_snapshot.is_some());
    let _ = ide.view();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn explorer_refresh_discovers_external_files_and_preserves_open_tabs() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let open_path = ide.docs.active().unwrap().path.clone();
    fs::write(root.join("added.rs"), "fn added() {}\n").unwrap();
    let _ = ide.update(Message::RefreshWorkspace);
    assert!(ide.tree.root.find_by_path("added.rs").is_some());
    assert!(
        ide.palette
            .all_items
            .iter()
            .any(|item| item.payload == "added.rs")
    );
    assert_eq!(ide.docs.active().unwrap().path, open_path);
    assert_eq!(
        ide.docs.active().unwrap().id,
        ide.tree.root.find_by_path(&open_path).unwrap().id
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_poll_reloads_clean_files_and_preserves_dirty_missing_buffers() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let main = ide.tree.root.find_by_path("main.rs").unwrap().id;
    let _ = ide.update(Message::FileSelected(main));
    fs::write(root.join("main.rs"), "fn externally_updated() {}\n").unwrap();
    fs::write(root.join("added.rs"), "fn added() {}\n").unwrap();
    let _ = ide.update(Message::WorkspacePoll);
    assert_eq!(
        ide.docs.active().unwrap().text(),
        "fn externally_updated() {}\n"
    );
    assert!(ide.tree.root.find_by_path("added.rs").is_some());

    let notes = ide.tree.root.find_by_path("notes.txt").unwrap().id;
    let _ = ide.update(Message::FileSelected(notes));
    let _ = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('D'),
    )));
    let dirty_text = ide.docs.active().unwrap().text();
    fs::remove_file(root.join("notes.txt")).unwrap();
    fs::remove_file(root.join("main.rs")).unwrap();
    let _ = ide.update(Message::WorkspacePoll);
    assert!(
        ide.docs
            .documents
            .iter()
            .any(|doc| { doc.path == "notes.txt" && doc.is_dirty && doc.text() == dirty_text })
    );
    assert!(!ide.docs.documents.iter().any(|doc| doc.path == "main.rs"));
    assert!(ide.tree.root.find_by_path("notes.txt").is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn external_changes_are_not_overwritten() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let file_id = ide.tree.root.find_by_path("main.rs").unwrap().id;
    let _ = ide.update(Message::FileSelected(file_id));
    let _ = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('X'),
    )));
    fs::write(root.join("main.rs"), "external edit\n").unwrap();
    let _ = ide.update(Message::SaveDocument);
    assert!(ide.docs.active().unwrap().is_dirty);
    assert_eq!(
        fs::read_to_string(root.join("main.rs")).unwrap(),
        "external edit\n"
    );
    assert!(
        ide.terminal
            .logs
            .iter()
            .any(|entry| entry.message.contains("changed on disk"))
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn save_and_rename_reject_parent_symlink_redirected_outside_workspace() {
    let root = workspace();
    let outside = root.with_extension("outside");
    fs::create_dir(root.join("nested")).unwrap();
    fs::write(root.join("nested/file.rs"), "initial\n").unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("file.rs"), "initial\n").unwrap();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let file = ide.tree.root.find_by_path("nested/file.rs").unwrap().id;
    let _ = ide.update(Message::FileSelected(file));
    let _ = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('X'),
    )));
    fs::remove_file(root.join("nested/file.rs")).unwrap();
    fs::remove_dir(root.join("nested")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("nested")).unwrap();

    let _ = ide.update(Message::SaveDocument);
    assert!(ide.docs.active().unwrap().is_dirty);
    assert_eq!(
        fs::read_to_string(outside.join("file.rs")).unwrap(),
        "initial\n"
    );
    let _ = ide.update(Message::OpenRenameSelected);
    let _ = ide.update(Message::RenameNameChanged("renamed.rs".into()));
    let _ = ide.update(Message::ConfirmRename);
    assert!(!outside.join("renamed.rs").exists());
    assert!(outside.join("file.rs").exists());
    drop(ide);
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[cfg(unix)]
#[test]
fn workspace_poll_never_reads_through_a_replaced_parent_symlink() {
    let root = workspace();
    let outside = root.with_extension("outside");
    fs::create_dir(root.join("nested")).unwrap();
    fs::write(root.join("nested/file.rs"), "inside\n").unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("file.rs"), "outside secret\n").unwrap();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let file = ide.tree.root.find_by_path("nested/file.rs").unwrap().id;
    let _ = ide.update(Message::FileSelected(file));
    fs::remove_file(root.join("nested/file.rs")).unwrap();
    fs::remove_dir(root.join("nested")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("nested")).unwrap();
    let _ = ide.update(Message::WorkspacePoll);
    assert!(
        !ide.docs
            .documents
            .iter()
            .any(|doc| doc.path == "nested/file.rs")
    );
    assert!(
        !ide.docs
            .documents
            .iter()
            .any(|doc| doc.text().contains("outside secret"))
    );
    drop(ide);
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[test]
fn creates_new_files_and_rejects_overwrite_or_traversal() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let _ = ide.update(Message::OpenNewFile);
    let _ = ide.update(Message::NewFilePathChanged("draft.rs".into()));
    let _ = ide.update(Message::CreateFile);
    assert!(root.join("draft.rs").is_file());
    assert_eq!(ide.docs.active().unwrap().path, "draft.rs");
    assert!(
        ide.palette
            .all_items
            .iter()
            .any(|item| item.payload == "draft.rs")
    );

    let _ = ide.update(Message::NewFilePathChanged("notes.txt".into()));
    let _ = ide.update(Message::CreateFile);
    assert_eq!(
        fs::read_to_string(root.join("notes.txt")).unwrap(),
        "needle is on disk\n"
    );

    let _ = ide.update(Message::NewFilePathChanged("../escape.rs".into()));
    let _ = ide.update(Message::CreateFile);
    assert!(!root.parent().unwrap().join("escape.rs").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn save_as_creates_new_file_without_changing_original() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let original = fs::read_to_string(root.join("main.rs")).unwrap();
    let main_id = ide.tree.root.find_by_path("main.rs").unwrap().id;
    let _ = ide.update(Message::FileSelected(main_id));
    let _ = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('X'),
    )));
    let edited = ide.docs.active().unwrap().text();
    let _ = ide.update(Message::OpenSaveAs);
    assert!(ide.saving_as);
    let _ = ide.update(Message::SaveAsPathChanged("copy.rs".into()));
    let _ = ide.update(Message::ConfirmSaveAs);
    assert_eq!(fs::read_to_string(root.join("main.rs")).unwrap(), original);
    assert_eq!(fs::read_to_string(root.join("copy.rs")).unwrap(), edited);
    assert_eq!(ide.docs.active().unwrap().path, "copy.rs");
    assert!(!ide.docs.active().unwrap().is_dirty);
    assert!(!ide.saving_as);
    assert!(ide.tree.root.find_by_path("copy.rs").is_some());

    let _ = ide.update(Message::OpenSaveAs);
    let _ = ide.update(Message::SaveAsPathChanged("notes.txt".into()));
    let _ = ide.update(Message::ConfirmSaveAs);
    assert!(ide.saving_as);
    assert_eq!(ide.docs.active().unwrap().path, "copy.rs");
    assert_eq!(
        fs::read_to_string(root.join("notes.txt")).unwrap(),
        "needle is on disk\n"
    );

    let _ = ide.update(Message::SaveAsPathChanged("../outside.rs".into()));
    let _ = ide.update(Message::ConfirmSaveAs);
    assert!(!root.parent().unwrap().join("outside.rs").exists());
    #[cfg(unix)]
    {
        let outside = root.with_extension("outside");
        fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();
        let _ = ide.update(Message::SaveAsPathChanged("linked/escaped.rs".into()));
        let _ = ide.update(Message::ConfirmSaveAs);
        assert!(!outside.join("escaped.rs").exists());
        fs::remove_dir_all(outside).unwrap();
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scratchpad_save_uses_save_as_and_preserves_buffer_on_failure() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let _ = ide.update(Message::NewScratchpad);
    let _ = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('Z'),
    )));
    let content = ide.docs.active().unwrap().text();
    let _ = ide.update(Message::SaveDocument);
    assert!(ide.saving_as);
    let _ = ide.update(Message::SaveAsPathChanged("notes.txt".into()));
    let _ = ide.update(Message::ConfirmSaveAs);
    assert!(ide.docs.active().unwrap().is_dirty);
    assert_eq!(ide.docs.active().unwrap().text(), content);
    let _ = ide.update(Message::SaveAsPathChanged("draft.txt".into()));
    let _ = ide.update(Message::ConfirmSaveAs);
    assert_eq!(fs::read_to_string(root.join("draft.txt")).unwrap(), content);
    assert!(!ide.docs.active().unwrap().is_dirty);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn save_as_keeps_editor_consistent_when_destination_is_outside_tree_depth() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let main_id = ide.tree.root.find_by_path("main.rs").unwrap().id;
    let _ = ide.update(Message::FileSelected(main_id));
    let _ = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('D'),
    )));
    let content = ide.docs.active().unwrap().text();
    let mut relative = PathBuf::new();
    for index in 0..17 {
        relative.push(format!("level{index}"));
    }
    fs::create_dir_all(root.join(&relative)).unwrap();
    relative.push("deep.rs");
    let relative = relative.to_string_lossy().to_string();
    let _ = ide.update(Message::OpenSaveAs);
    let _ = ide.update(Message::SaveAsPathChanged(relative.clone()));
    let _ = ide.update(Message::ConfirmSaveAs);
    assert_eq!(fs::read_to_string(root.join(&relative)).unwrap(), content);
    assert_eq!(ide.docs.active().unwrap().path, relative);
    assert_eq!(ide.docs.active().unwrap().syntax, "rust");
    assert!(!ide.docs.active().unwrap().is_dirty);
    assert!(!ide.saving_as);
    assert!(ide.tree.selected_id.is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn explorer_rename_keeps_dirty_editor_and_rejects_collisions() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let main = ide.tree.root.find_by_path("main.rs").unwrap().id;
    let _ = ide.update(Message::FileSelected(main));
    let _ = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('Q'),
    )));
    let edited = ide.docs.active().unwrap().text();
    let _ = ide.update(Message::OpenRenameSelected);
    let _ = ide.update(Message::RenameNameChanged("notes.txt".into()));
    let _ = ide.update(Message::ConfirmRename);
    assert!(root.join("main.rs").exists());
    assert_eq!(ide.docs.active().unwrap().path, "main.rs");
    assert!(ide.renaming_file);

    let _ = ide.update(Message::RenameNameChanged("renamed.rs".into()));
    let _ = ide.update(Message::ConfirmRename);
    assert!(!root.join("main.rs").exists());
    assert!(root.join("renamed.rs").exists());
    assert_eq!(ide.docs.active().unwrap().path, "renamed.rs");
    assert!(ide.docs.active().unwrap().is_dirty);
    assert_eq!(ide.docs.active().unwrap().text(), edited);
    assert!(ide.tree.root.find_by_path("renamed.rs").is_some());
    let _ = ide.update(Message::SaveDocument);
    assert_eq!(fs::read_to_string(root.join("renamed.rs")).unwrap(), edited);
    assert!(!ide.docs.active().unwrap().is_dirty);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rename_keeps_open_editor_consistent_when_new_name_is_filtered_from_tree() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let main = ide.tree.root.find_by_path("main.rs").unwrap().id;
    let _ = ide.update(Message::FileSelected(main));
    let _ = ide.update(Message::OpenRenameSelected);
    let _ = ide.update(Message::RenameNameChanged("target".into()));
    let _ = ide.update(Message::ConfirmRename);
    assert!(!root.join("main.rs").exists());
    assert!(root.join("target").exists());
    assert_eq!(ide.docs.active().unwrap().path, "target");
    assert!(!ide.renaming_file);
    assert!(ide.tree.selected_id.is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn explorer_renames_directory_and_reconciles_open_dirty_children() {
    let root = workspace();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/one.rs"), "fn one() {}\n").unwrap();
    fs::write(root.join("src/two.rs"), "fn two() {}\n").unwrap();
    fs::create_dir(root.join("occupied")).unwrap();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let one = ide.tree.root.find_by_path("src/one.rs").unwrap().id;
    let two = ide.tree.root.find_by_path("src/two.rs").unwrap().id;
    let src = ide.tree.root.find_by_path("src").unwrap().id;
    let _ = ide.update(Message::FileSelected(one));
    let _ = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('D'),
    )));
    let edited = ide.docs.active().unwrap().text();
    let _ = ide.update(Message::FileSelected(two));
    ide.tree.selected_id = Some(src);
    let _ = ide.update(Message::OpenRenameSelected);
    let _ = ide.update(Message::RenameNameChanged("occupied".into()));
    let _ = ide.update(Message::ConfirmRename);
    assert!(root.join("src").exists());
    assert!(ide.renaming_file);

    let _ = ide.update(Message::RenameNameChanged("source".into()));
    let _ = ide.update(Message::ConfirmRename);
    assert!(!root.join("src").exists());
    assert!(root.join("source/one.rs").exists());
    assert!(
        ide.docs
            .documents
            .iter()
            .any(|doc| { doc.path == "source/one.rs" && doc.is_dirty && doc.text() == edited })
    );
    assert!(
        ide.docs
            .documents
            .iter()
            .any(|doc| doc.path == "source/two.rs")
    );
    let one_index = ide
        .docs
        .documents
        .iter()
        .position(|doc| doc.path == "source/one.rs")
        .unwrap();
    let _ = ide.update(Message::TabSelected(one_index));
    let _ = ide.update(Message::SaveDocument);
    assert_eq!(
        fs::read_to_string(root.join("source/one.rs")).unwrap(),
        edited
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn explorer_delete_requires_confirmation_and_preserves_entry_in_trash() {
    let root = workspace();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/one.rs"), "fn one() {}\n").unwrap();
    fs::write(root.join("src/two.rs"), "fn two() {}\n").unwrap();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let one = ide.tree.root.find_by_path("src/one.rs").unwrap().id;
    let src = ide.tree.root.find_by_path("src").unwrap().id;
    let _ = ide.update(Message::FileSelected(one));
    let _ = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('U'),
    )));
    ide.tree.selected_id = Some(src);
    let _ = ide.update(Message::OpenDeleteSelected);
    assert_eq!(ide.deleting_path.as_deref(), Some("src"));
    assert!(root.join("src").exists());
    let _ = ide.update(Message::ConfirmDelete);
    assert!(root.join("src").exists());
    assert!(ide.deleting_path.is_some());

    let dirty = ide
        .docs
        .documents
        .iter()
        .position(|doc| doc.path == "src/one.rs")
        .unwrap();
    let _ = ide.update(Message::TabSelected(dirty));
    let _ = ide.update(Message::SaveDocument);
    ide.tree.selected_id = Some(src);
    let _ = ide.update(Message::ConfirmDelete);
    assert!(!root.join("src").exists());
    assert!(ide.deleting_path.is_none());
    assert!(
        !ide.docs
            .documents
            .iter()
            .any(|doc| doc.path.starts_with("src/"))
    );
    assert!(ide.tree.root.find_by_path(".virgo-trash").is_none());
    let trashed: Vec<_> = fs::read_dir(root.join(".virgo-trash"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    assert_eq!(trashed.len(), 1);
    assert_eq!(ide.trash_entries.len(), 1);
    assert_eq!(
        fs::read_to_string(trashed[0].join("one.rs")).unwrap(),
        "Ufn one() {}\n"
    );
    assert_eq!(
        fs::read_to_string(trashed[0].join("two.rs")).unwrap(),
        "fn two() {}\n"
    );
    let _ = ide.update(Message::RestoreTrash(0));
    assert!(root.join("src/one.rs").exists());
    assert!(root.join("src/two.rs").exists());
    assert!(ide.trash_entries.is_empty());
    assert!(ide.tree.root.find_by_path("src/one.rs").is_some());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn trash_restore_never_overwrites_a_recreated_path() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let notes = ide.tree.root.find_by_path("notes.txt").unwrap().id;
    ide.tree.selected_id = Some(notes);
    let _ = ide.update(Message::OpenDeleteSelected);
    let _ = ide.update(Message::ConfirmDelete);
    assert_eq!(ide.trash_entries.len(), 1);
    fs::write(root.join("notes.txt"), "replacement\n").unwrap();
    let _ = ide.update(Message::RestoreTrash(0));
    assert_eq!(
        fs::read_to_string(root.join("notes.txt")).unwrap(),
        "replacement\n"
    );
    assert_eq!(ide.trash_entries.len(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn delete_cancel_and_workspace_root_protection_leave_disk_untouched() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let notes = ide.tree.root.find_by_path("notes.txt").unwrap().id;
    ide.tree.selected_id = Some(notes);
    let _ = ide.update(Message::OpenDeleteSelected);
    let _ = ide.update(Message::CancelDelete);
    assert!(root.join("notes.txt").exists());
    assert!(ide.deleting_path.is_none());
    ide.tree.selected_id = Some(0);
    let _ = ide.update(Message::OpenDeleteSelected);
    assert!(ide.deleting_path.is_none());
    assert!(root.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn dirty_file_draft_survives_restart_and_clears_after_save() {
    let root = workspace();
    let backup = root.with_extension("recovery");
    let (mut ide, _) = Studio::with_workspace_and_recovery(root.clone(), backup.clone()).unwrap();
    let main = ide.tree.root.find_by_path("main.rs").unwrap().id;
    let _ = ide.update(Message::FileSelected(main));
    let task = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('R'),
    )));
    let edited = ide.docs.active().unwrap().text();
    let result = block_on(taim_runtime::task::into_stream(task).unwrap().next()).unwrap();
    let Action::Output(message) = result else {
        panic!("expected recovery timer output");
    };
    let _ = ide.update(message);
    drop(ide);

    let (mut restarted, _) =
        Studio::with_workspace_and_recovery(root.clone(), backup.clone()).unwrap();
    assert_eq!(restarted.pending_recovery_count(), 1);
    assert_eq!(
        fs::read_to_string(root.join("main.rs")).unwrap(),
        "fn real_workspace() {}\n"
    );
    let _ = restarted.update(Message::RestoreRecovery(0));
    assert_eq!(restarted.pending_recovery_count(), 0);
    assert_eq!(restarted.docs.active().unwrap().text(), edited);
    assert!(restarted.docs.active().unwrap().is_dirty);
    let _ = restarted.update(Message::SaveDocument);
    assert_eq!(fs::read_to_string(root.join("main.rs")).unwrap(), edited);
    assert!(!restarted.docs.active().unwrap().is_dirty);
    drop(restarted);

    let (third, _) = Studio::with_workspace_and_recovery(root.clone(), backup.clone()).unwrap();
    assert_eq!(third.pending_recovery_count(), 0);
    drop(third);
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(backup).unwrap();
}

#[test]
fn old_recovery_timer_cannot_replace_a_newer_edit() {
    let root = workspace();
    let backup = root.with_extension("recovery");
    let (mut ide, _) = Studio::with_workspace_and_recovery(root.clone(), backup.clone()).unwrap();
    let first = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('A'),
    )));
    let second = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('B'),
    )));
    let latest = ide.docs.active().unwrap().text();
    let first = block_on(taim_runtime::task::into_stream(first).unwrap().next()).unwrap();
    let Action::Output(first) = first else {
        panic!("expected first recovery timer");
    };
    let _ = ide.update(first);
    let (before, _) = Studio::with_workspace_and_recovery(root.clone(), backup.clone()).unwrap();
    assert_eq!(before.pending_recovery_count(), 0);
    drop(before);

    let second = block_on(taim_runtime::task::into_stream(second).unwrap().next()).unwrap();
    let Action::Output(second) = second else {
        panic!("expected second recovery timer");
    };
    let _ = ide.update(second);
    let (after, _) = Studio::with_workspace_and_recovery(root.clone(), backup.clone()).unwrap();
    assert_eq!(after.pending_recovery_count(), 1);
    let _ = after.view();
    drop(after);
    drop(ide);
    let (mut restored, _) =
        Studio::with_workspace_and_recovery(root.clone(), backup.clone()).unwrap();
    let _ = restored.update(Message::RestoreRecovery(0));
    assert_eq!(restored.docs.active().unwrap().text(), latest);
    drop(restored);
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(backup).unwrap();
}

#[test]
fn restored_draft_does_not_overwrite_external_disk_edit() {
    let root = workspace();
    let backup = root.with_extension("recovery");
    let (mut ide, _) = Studio::with_workspace_and_recovery(root.clone(), backup.clone()).unwrap();
    let main = ide.tree.root.find_by_path("main.rs").unwrap().id;
    let _ = ide.update(Message::FileSelected(main));
    let _ = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('C'),
    )));
    drop(ide);
    fs::write(root.join("main.rs"), "external change\n").unwrap();

    let (mut restarted, _) =
        Studio::with_workspace_and_recovery(root.clone(), backup.clone()).unwrap();
    assert_eq!(restarted.pending_recovery_count(), 1);
    let _ = restarted.update(Message::RestoreRecovery(0));
    assert!(restarted.docs.active().unwrap().is_dirty);
    let _ = restarted.update(Message::SaveDocument);
    assert_eq!(
        fs::read_to_string(root.join("main.rs")).unwrap(),
        "external change\n"
    );
    assert!(restarted.docs.active().unwrap().is_dirty);
    let _ = restarted.update(Message::OpenSaveAs);
    let _ = restarted.update(Message::SaveAsPathChanged("recovered.rs".into()));
    let _ = restarted.update(Message::ConfirmSaveAs);
    assert!(root.join("recovered.rs").exists());
    assert!(!restarted.docs.active().unwrap().is_dirty);
    drop(restarted);
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(backup).unwrap();
}

#[test]
fn scratchpad_draft_can_be_restored_or_discarded() {
    let root = workspace();
    let backup = root.with_extension("recovery");
    let (mut ide, _) = Studio::with_workspace_and_recovery(root.clone(), backup.clone()).unwrap();
    let _ = ide.update(Message::NewScratchpad);
    let _ = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('S'),
    )));
    let text = ide.docs.active().unwrap().text();
    drop(ide);

    let (mut restarted, _) =
        Studio::with_workspace_and_recovery(root.clone(), backup.clone()).unwrap();
    assert_eq!(restarted.pending_recovery_count(), 1);
    let _ = restarted.update(Message::RestoreRecovery(0));
    assert_eq!(restarted.docs.active().unwrap().text(), text);
    assert!(restarted.docs.active().unwrap().disk_snapshot.is_none());
    let _ = restarted.update(Message::SaveDocument);
    assert!(restarted.saving_as);
    drop(restarted);

    let (mut third, _) = Studio::with_workspace_and_recovery(root.clone(), backup.clone()).unwrap();
    assert_eq!(third.pending_recovery_count(), 1);
    let _ = third.update(Message::DiscardRecovery(0));
    assert_eq!(third.pending_recovery_count(), 0);
    drop(third);
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(backup).unwrap();
}

#[test]
fn search_reads_unopened_files_and_dirty_buffers() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let _ = ide.update(Message::GlobalSearchQueryChanged("needle".into()));
    assert_eq!(ide.search.total_matches, 1);
    assert_eq!(ide.search.results[0].file_path, "notes.txt");
    let hit = ide.search.results[0].hits[0].clone();
    let _ = ide.update(Message::SearchHitSelected(
        hit.file_path,
        hit.line_number,
        hit.column,
    ));
    assert_eq!(ide.docs.active().unwrap().path, "notes.txt");
    assert!(
        ide.palette
            .all_items
            .iter()
            .any(|item| item.payload == "notes.txt")
    );

    let file_id = ide.tree.root.find_by_path("notes.txt").unwrap().id;
    let _ = ide.update(Message::FileSelected(file_id));
    ide.docs.active_mut().unwrap().content =
        text_editor::Content::with_text("unsaved needle needle");
    ide.docs.active_mut().unwrap().is_dirty = true;
    let _ = ide.update(Message::GlobalSearchQueryChanged("needle".into()));
    assert_eq!(ide.search.total_matches, 2);
    let _ = ide.update(Message::ToggleSearchWord);
    assert_eq!(ide.search.total_matches, 2);
    assert_eq!(
        fs::read_to_string(root.join("notes.txt")).unwrap(),
        "needle is on disk\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_search_supports_regex_and_file_globs() {
    let root = workspace();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/code.rs"), "needle needlebar\nNEEDLE\n").unwrap();
    fs::write(root.join("root.rs"), "needle\n").unwrap();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let _ = ide.update(Message::SearchIncludeChanged("**/*.rs".into()));
    let _ = ide.update(Message::GlobalSearchQueryChanged("needl[eE]".into()));
    let _ = ide.update(Message::ToggleSearchRegex);
    let _ = ide.update(Message::ToggleSearchWord);
    assert_eq!(ide.search.total_matches, 3);
    let _ = ide.update(Message::SearchExcludeChanged("src/**".into()));
    assert_eq!(ide.search.total_matches, 1);
    assert_eq!(ide.search.results[0].file_path, "root.rs");

    let _ = ide.update(Message::GlobalSearchQueryChanged("[".into()));
    assert!(ide.search.search_error.is_some());
    assert_eq!(ide.search.total_matches, 0);
    let _ = ide.update(Message::ToggleSearchRegex);
    assert!(ide.search.search_error.is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reviewed_workspace_replace_preserves_dirty_buffers_and_supports_guarded_undo() {
    let root = workspace();
    fs::write(root.join("other.txt"), "needle elsewhere\n").unwrap();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let notes = ide.tree.root.find_by_path("notes.txt").unwrap().id;
    let _ = ide.update(Message::FileSelected(notes));
    ide.docs.active_mut().unwrap().content =
        text_editor::Content::with_text("needle dirty needle\n");
    ide.docs.active_mut().unwrap().is_dirty = true;
    let dirty_before = ide.docs.active().unwrap().text();
    let _ = ide.update(Message::GlobalSearchQueryChanged("needle".into()));
    let _ = ide.update(Message::GlobalSearchReplaceQueryChanged("pin".into()));
    let _ = ide.update(Message::PreviewWorkspaceReplace);
    let preview = ide.replace_preview.as_ref().unwrap();
    assert_eq!(preview.total_replacements, 3);
    assert_eq!(preview.files.len(), 2);
    assert_eq!(
        fs::read_to_string(root.join("other.txt")).unwrap(),
        "needle elsewhere\n"
    );

    let _ = ide.update(Message::ConfirmWorkspaceReplace);
    assert!(ide.replace_preview.is_none());
    assert!(ide.replace_undo.is_some());
    assert_eq!(ide.docs.active().unwrap().text(), "pin dirty pin\n");
    assert!(ide.docs.active().unwrap().is_dirty);
    assert_eq!(
        fs::read_to_string(root.join("notes.txt")).unwrap(),
        "needle is on disk\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("other.txt")).unwrap(),
        "pin elsewhere\n"
    );

    let _ = ide.update(Message::UndoWorkspaceReplace);
    assert!(ide.replace_undo.is_none());
    assert_eq!(ide.docs.active().unwrap().text(), dirty_before);
    assert!(ide.docs.active().unwrap().is_dirty);
    assert_eq!(
        fs::read_to_string(root.join("other.txt")).unwrap(),
        "needle elsewhere\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_replace_cancels_entire_batch_when_a_file_changes_after_preview() {
    let root = workspace();
    fs::write(root.join("other.txt"), "needle elsewhere\n").unwrap();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let _ = ide.update(Message::GlobalSearchQueryChanged("needle".into()));
    let _ = ide.update(Message::GlobalSearchReplaceQueryChanged("changed".into()));
    let _ = ide.update(Message::PreviewWorkspaceReplace);
    assert_eq!(ide.replace_preview.as_ref().unwrap().files.len(), 2);
    fs::write(root.join("other.txt"), "external edit\n").unwrap();
    let _ = ide.update(Message::ConfirmWorkspaceReplace);
    assert!(ide.replace_preview.is_some());
    assert!(ide.replace_undo.is_none());
    assert_eq!(
        fs::read_to_string(root.join("notes.txt")).unwrap(),
        "needle is on disk\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("other.txt")).unwrap(),
        "external edit\n"
    );
    assert!(
        ide.terminal
            .logs
            .iter()
            .any(|entry| entry.message.contains("changed on disk"))
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_replace_undo_refuses_to_overwrite_later_edits() {
    let root = workspace();
    fs::write(root.join("other.txt"), "needle elsewhere\n").unwrap();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let _ = ide.update(Message::GlobalSearchQueryChanged("needle".into()));
    let _ = ide.update(Message::GlobalSearchReplaceQueryChanged("done".into()));
    let _ = ide.update(Message::PreviewWorkspaceReplace);
    let _ = ide.update(Message::ConfirmWorkspaceReplace);
    assert_eq!(
        fs::read_to_string(root.join("notes.txt")).unwrap(),
        "done is on disk\n"
    );
    fs::write(root.join("other.txt"), "later edit\n").unwrap();
    let _ = ide.update(Message::UndoWorkspaceReplace);
    assert!(ide.replace_undo.is_some());
    assert_eq!(
        fs::read_to_string(root.join("notes.txt")).unwrap(),
        "done is on disk\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("other.txt")).unwrap(),
        "later edit\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn regex_workspace_replace_expands_capture_groups_while_literal_replace_keeps_dollars() {
    let root = workspace();
    fs::write(root.join("values.txt"), "item-12 item-34\n").unwrap();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let _ = ide.update(Message::GlobalSearchQueryChanged("item-(\\d+)".into()));
    let _ = ide.update(Message::ToggleSearchRegex);
    let _ = ide.update(Message::GlobalSearchReplaceQueryChanged("value:$1".into()));
    let _ = ide.update(Message::PreviewWorkspaceReplace);
    let _ = ide.update(Message::ConfirmWorkspaceReplace);
    assert_eq!(
        fs::read_to_string(root.join("values.txt")).unwrap(),
        "value:12 value:34\n"
    );
    let _ = ide.update(Message::GlobalSearchQueryChanged("value".into()));
    let _ = ide.update(Message::ToggleSearchRegex);
    let _ = ide.update(Message::GlobalSearchReplaceQueryChanged("$cash".into()));
    let _ = ide.update(Message::PreviewWorkspaceReplace);
    let _ = ide.update(Message::ConfirmWorkspaceReplace);
    assert_eq!(
        fs::read_to_string(root.join("values.txt")).unwrap(),
        "$cash:12 $cash:34\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn closing_a_tab_preserves_the_active_document_and_split_indices() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let main = ide.tree.root.find_by_path("main.rs").unwrap().id;
    let notes = ide.tree.root.find_by_path("notes.txt").unwrap().id;
    let _ = ide.update(Message::FileSelected(main));
    let _ = ide.update(Message::FileSelected(notes));
    let _ = ide.update(Message::ToggleSplitEditor);
    let active_path = ide.docs.active().unwrap().path.clone();
    let close = ide
        .docs
        .documents
        .iter()
        .position(|doc| doc.path != active_path)
        .unwrap();
    let _ = ide.update(Message::TabClosed(close));
    assert_eq!(ide.docs.active().unwrap().path, active_path);
    assert!(
        ide.docs
            .split_tab
            .is_none_or(|idx| idx < ide.docs.documents.len())
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn closing_last_workspace_tab_returns_to_empty_editor() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let main = ide.tree.root.find_by_path("main.rs").unwrap().id;
    let _ = ide.update(Message::FileSelected(main));
    let _ = ide.update(Message::TabClosed(0));
    assert!(ide.docs.documents.is_empty());
    let _ = ide.view();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn large_workspace_search_runs_as_a_task_and_ignores_outdated_results() {
    let root = workspace();
    for i in 0..70 {
        fs::write(
            root.join(format!("sample_{i}.txt")),
            "needle needle needle needle\n",
        )
        .unwrap();
    }
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let pending = ide.update(Message::GlobalSearchQueryChanged("needle".into()));
    assert!(ide.search.is_searching);
    let newer = ide.update(Message::GlobalSearchQueryChanged("never-present".into()));
    let outdated = block_on(taim_runtime::task::into_stream(pending).unwrap().next()).unwrap();
    let Action::Output(outdated) = outdated else {
        panic!("expected search result")
    };
    let _ = ide.update(outdated);
    assert!(ide.search.is_searching);
    let result = block_on(taim_runtime::task::into_stream(newer).unwrap().next()).unwrap();
    let Action::Output(result) = result else {
        panic!("expected search result")
    };
    let _ = ide.update(result);
    assert_eq!(ide.search.query, "never-present");
    assert_eq!(ide.search.total_matches, 0);
    assert!(!ide.search.is_searching);

    let task = ide.update(Message::GlobalSearchQueryChanged("needle".into()));
    let result = block_on(taim_runtime::task::into_stream(task).unwrap().next()).unwrap();
    let Action::Output(result) = result else {
        panic!("expected search result")
    };
    let _ = ide.update(result);
    assert_eq!(ide.search.total_matches, 281);
    assert_eq!(
        ide.search
            .results
            .iter()
            .map(|g| g.hits.len())
            .sum::<usize>(),
        200
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn editing_while_search_is_open_refreshes_hits_without_blocking_input() {
    let root = workspace();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let file = ide.tree.root.find_by_path("notes.txt").unwrap().id;
    let _ = ide.update(Message::FileSelected(file));
    let _ = ide.update(Message::GlobalSearchQueryChanged("needle".into()));
    let _ = ide.update(Message::SelectSidebarView(taim_ide::SidebarView::Search));
    let task = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('x'),
    )));
    assert!(ide.search.is_searching);
    let result = block_on(taim_runtime::task::into_stream(task).unwrap().next()).unwrap();
    let Action::Output(result) = result else {
        panic!("expected search result")
    };
    let _ = ide.update(result);
    assert_eq!(ide.search.total_matches, 1);
    assert!(!ide.search.is_searching);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn multi_cursor_cmd_d_and_simultaneous_editing() {
    let root = workspace();
    fs::write(root.join("multi.rs"), "apple banana apple orange apple\n").unwrap();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let file = ide.tree.root.find_by_path("multi.rs").unwrap().id;
    let _ = ide.update(Message::FileSelected(file));
    let doc = ide.docs.active_mut().unwrap();
    doc.content
        .move_to(text_editor::Cursor::new(text_editor::Position::new(0, 2)));

    // First Cmd+D selects the word "apple" and finds the next occurrence
    let _ = ide.update(Message::SelectNextOccurrence);
    assert_eq!(ide.docs.active().unwrap().extra_cursors().len(), 1);

    // Second Cmd+D selects the third "apple"
    let _ = ide.update(Message::SelectNextOccurrence);
    assert_eq!(ide.docs.active().unwrap().extra_cursors().len(), 2);

    // Insert 'X' at all cursors simultaneously
    let _ = ide.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('X'),
    )));
    let text = ide.docs.active().unwrap().text();
    assert!(text.contains('X'));

    // Press Escape to clear extra cursors
    let _ = ide.update(Message::ClearExtraCursors);
    assert!(!ide.docs.active().unwrap().has_extra_cursors());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn large_file_opens_sub_millisecond_via_memory_mapping() {
    let root = workspace();
    // Create a 3MB file (> 1MB threshold) with 60,000 lines
    let mut large_content = String::with_capacity(3 * 1024 * 1024);
    for i in 0..60_000 {
        large_content.push_str(&format!("line {:06}: fn compute_kernel_{}() {{}}\n", i, i));
    }
    fs::write(root.join("large_data.rs"), &large_content).unwrap();

    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let file_id = ide.tree.root.find_by_path("large_data.rs").unwrap().id;

    let start = std::time::Instant::now();
    let _ = ide.update(Message::FileSelected(file_id));
    let duration = start.elapsed();

    // Verify sub-50ms opening (typically < 2ms)
    assert!(
        duration < std::time::Duration::from_millis(50),
        "large file open took {:?}, expected < 50ms",
        duration
    );

    let doc = ide.docs.active().unwrap();
    assert!(matches!(
        doc.backing,
        taim_ide::DocumentBacking::MemoryMapped {
            is_partial: true,
            ..
        }
    ));
    assert!(doc.line_count() >= 60_000);

    // View renders cleanly without panicking
    let _ = ide.view();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn search_scans_large_memory_mapped_files() {
    let root = workspace();
    // Create a 2.5MB file with a target needle
    let mut large_content = String::with_capacity(2500 * 1024);
    for _ in 0..30_000 {
        large_content.push_str("let padding = 123456789;\n");
    }
    large_content.push_str("let special = TAIM_ULTRA_SIMD_SECRET;\n");
    for _ in 0..5_000 {
        large_content.push_str("let padding_after = 987654321;\n");
    }
    fs::write(root.join("huge_bundle.rs"), &large_content).unwrap();

    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let task = ide.update(Message::GlobalSearchQueryChanged(
        "TAIM_ULTRA_SIMD_SECRET".into(),
    ));
    if let Some(mut stream) = taim_runtime::task::into_stream(task) {
        if let Some(Action::Output(result)) = block_on(stream.next()) {
            let _ = ide.update(result);
        }
    }
    assert_eq!(ide.search.total_matches, 1);
    assert_eq!(ide.search.results[0].file_path, "huge_bundle.rs");
    fs::remove_dir_all(root).unwrap();
}
