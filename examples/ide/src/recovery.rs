//! Crash recovery snapshots for dirty workspace documents.

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_FILE: AtomicU64 = AtomicU64::new(1);
const FORMAT_VERSION: u32 = 1;
const MAX_SNAPSHOT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryEntry {
    pub path: String,
    pub name: String,
    pub syntax: String,
    pub text: String,
    pub disk_snapshot: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RecoveryCandidate {
    pub entry: RecoveryEntry,
    source: PathBuf,
    index: usize,
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    version: u32,
    workspace: String,
    entries: Vec<RecoveryEntry>,
}

pub struct RecoveryStore {
    directory: PathBuf,
    session_file: PathBuf,
    workspace: String,
    pending: Vec<RecoveryCandidate>,
    unreadable: usize,
}

impl RecoveryStore {
    pub fn default_base() -> io::Result<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            return std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join("Library/Application Support/Virgo/Backups"))
                .ok_or_else(|| io::Error::other("HOME is unavailable"));
        }
        #[cfg(target_os = "windows")]
        {
            return std::env::var_os("LOCALAPPDATA")
                .map(|root| PathBuf::from(root).join("Virgo/Backups"))
                .ok_or_else(|| io::Error::other("LOCALAPPDATA is unavailable"));
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            if let Some(root) = std::env::var_os("XDG_STATE_HOME") {
                return Ok(PathBuf::from(root).join("virgo/backups"));
            }
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".local/state/virgo/backups"))
                .ok_or_else(|| io::Error::other("HOME is unavailable"))
        }
    }

    pub fn open(base: &Path, workspace: &Path) -> io::Result<Self> {
        let workspace_bytes = workspace.as_os_str().as_encoded_bytes();
        let workspace: String = workspace_bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in workspace_bytes {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        }
        let directory = base.join(format!("{hash:016x}"));
        fs::create_dir_all(&directory)?;
        #[cfg(unix)]
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
        let mut store = Self {
            directory,
            session_file: PathBuf::new(),
            workspace,
            pending: Vec::new(),
            unreadable: 0,
        };
        store.reload_pending()?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        loop {
            let suffix = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
            let path = store.directory.join(format!(
                "session-{}-{nonce}-{suffix}.json",
                std::process::id()
            ));
            match create_private_file(&path) {
                Ok(mut file) => {
                    let empty = store.snapshot(Vec::new());
                    file.write_all(&encode(&empty)?)?;
                    file.sync_all()?;
                    store.session_file = path;
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(store)
    }

    pub fn pending(&self) -> &[RecoveryCandidate] {
        &self.pending
    }

    pub fn unreadable_count(&self) -> usize {
        self.unreadable
    }

    pub fn persist(&self, entries: Vec<RecoveryEntry>) -> io::Result<()> {
        if entries.is_empty() {
            match fs::remove_file(&self.session_file) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error),
            }
        }
        let bytes = encode(&self.snapshot(entries))?;
        if bytes.len() as u64 > MAX_SNAPSHOT_BYTES {
            return Err(io::Error::other(
                "recovery draft exceeds the 64 MB safety limit",
            ));
        }
        atomic_write(&self.session_file, &bytes)
    }

    pub fn dismiss(&mut self, candidate_index: usize) -> io::Result<()> {
        let Some(candidate) = self.pending.get(candidate_index).cloned() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "draft not found",
            ));
        };
        let mut snapshot = read_snapshot(&candidate.source)?;
        if snapshot.workspace != self.workspace || snapshot.version != FORMAT_VERSION {
            return Err(io::Error::other("recovery draft changed on disk"));
        }
        if snapshot.entries.get(candidate.index) != Some(&candidate.entry) {
            return Err(io::Error::other("recovery draft changed on disk"));
        }
        snapshot.entries.remove(candidate.index);
        if snapshot.entries.is_empty() {
            fs::remove_file(&candidate.source)?;
        } else {
            atomic_write(&candidate.source, &encode(&snapshot)?)?;
        }
        self.reload_pending()
    }

    fn snapshot(&self, entries: Vec<RecoveryEntry>) -> Snapshot {
        Snapshot {
            version: FORMAT_VERSION,
            workspace: self.workspace.clone(),
            entries,
        }
    }

    fn reload_pending(&mut self) -> io::Result<()> {
        let mut candidates = Vec::new();
        let mut unreadable = 0;
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let path = entry.path();
            if path == self.session_file || path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            if !entry.file_type()?.is_file() {
                continue;
            }
            let snapshot = match read_snapshot(&path) {
                Ok(snapshot) => snapshot,
                Err(_) => {
                    unreadable += 1; // Keep damaged drafts on disk for manual inspection.
                    continue;
                }
            };
            if snapshot.workspace != self.workspace || snapshot.version != FORMAT_VERSION {
                continue;
            }
            for (index, entry) in snapshot.entries.into_iter().enumerate() {
                candidates.push(RecoveryCandidate {
                    entry,
                    source: path.clone(),
                    index,
                });
            }
        }
        candidates.sort_by(|a, b| a.source.cmp(&b.source).then(a.index.cmp(&b.index)));
        self.pending = candidates;
        self.unreadable = unreadable;
        Ok(())
    }
}

fn read_snapshot(path: &Path) -> io::Result<Snapshot> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_SNAPSHOT_BYTES {
        return Err(io::Error::other("invalid recovery snapshot"));
    }
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn encode(snapshot: &Snapshot) -> io::Result<Vec<u8>> {
    serde_json::to_vec(snapshot).map_err(io::Error::other)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("recovery file has no parent"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("recovery file has no name"))?;
    let temporary = parent.join(format!(
        ".{}.{}-{}.tmp",
        name.to_string_lossy(),
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> io::Result<()> {
        let mut file = create_private_file(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn create_private_file(path: &Path) -> io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreadable_snapshot_is_reported_and_kept_on_disk() {
        let root = std::env::temp_dir().join(format!(
            "virgo-recovery-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        let workspace = root.join("workspace");
        let base = root.join("backups");
        fs::create_dir_all(&workspace).unwrap();
        let store = RecoveryStore::open(&base, &workspace).unwrap();
        let corrupt = store.directory.join("corrupt.json");
        fs::write(&corrupt, b"not json").unwrap();
        drop(store);
        let reopened = RecoveryStore::open(&base, &workspace).unwrap();
        assert_eq!(reopened.unreadable_count(), 1);
        assert!(corrupt.exists());
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn recovery_snapshot_permissions_are_private() {
        let root = std::env::temp_dir().join(format!(
            "virgo-recovery-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        let workspace = root.join("workspace");
        let base = root.join("backups");
        fs::create_dir_all(&workspace).unwrap();
        let store = RecoveryStore::open(&base, &workspace).unwrap();
        store
            .persist(vec![RecoveryEntry {
                path: "main.rs".to_string(),
                name: "main.rs".to_string(),
                syntax: "rust".to_string(),
                text: "unsaved".to_string(),
                disk_snapshot: Some("saved".to_string()),
            }])
            .unwrap();
        assert_eq!(
            fs::metadata(&store.directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&store.session_file)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
}
