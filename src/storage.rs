//! Small, persistent key-value settings, like Flutter's
//! `shared_preferences`: a theme choice, a signed-in user's name, the last
//! tab opened.
//!
//! A [`Store`] is a file in the application's private data directory
//! ([`crate::platform::data_dir`]). Every change is written at once, through
//! a temporary file renamed into place, so a crash or a killed process
//! never leaves a half-written store. On the web, where a page has no
//! files, it is an entry in the page's `localStorage` instead, written in
//! one call; the API is the same. It is meant for settings, not for
//! large data or records: keep those in a database or files of your own.
//! Each open store is a snapshot; concurrent writers replace the entire
//! snapshot, so applications must coordinate read-modify-write operations.
//! A failed write leaves the in-memory snapshot unchanged and can be retried.
//!
//! ```no_run
//! use taim::storage::Store;
//!
//! # fn main() -> Result<(), taim::storage::Error> {
//! let mut settings = Store::open("settings")?;
//! let launches: u32 = settings.get_as("launches").unwrap_or(0);
//! settings.set("launches", launches + 1)?;
//! # Ok(())
//! # }
//! ```

use std::collections::BTreeMap;
use std::fmt::Display;
#[cfg(not(target_arch = "wasm32"))]
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Why the store could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The store's name is not a plain file name.
    #[error("store names are letters, digits, `-` and `_`: {0:?}")]
    Name(String),
    /// The platform has no data directory.
    #[error(transparent)]
    Platform(#[from] taim_winit::platform::Error),
    /// Reading or writing the file failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// A named set of string settings that survives restarts.
#[derive(Debug)]
pub struct Store {
    path: PathBuf,
    values: BTreeMap<String, String>,
}

impl Store {
    /// Opens (or starts) the store called `name` in the application's data
    /// directory.
    pub fn open(name: &str) -> Result<Self, Error> {
        let valid = !name.is_empty()
            && name.len() <= 64
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !valid {
            return Err(Error::Name(name.to_owned()));
        }

        #[cfg(target_arch = "wasm32")]
        {
            Self::open_at(format!("{name}.store"))
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let directory = taim_winit::platform::data_dir()?;
            Self::open_at(directory.join(format!("{name}.store")))
        }
    }

    /// Opens (or starts) a store kept in the file at `path`. On the web,
    /// `path` names the `localStorage` entry instead.
    pub fn open_at(path: impl Into<PathBuf>) -> Result<Self, Error> {
        let path = path.into();

        #[cfg(target_arch = "wasm32")]
        let values = taim_winit::platform::web_storage::read(&web_key(&path))?
            .map(|contents| parse(&contents))
            .unwrap_or_default();

        #[cfg(not(target_arch = "wasm32"))]
        let values = match std::fs::read_to_string(&path) {
            Ok(contents) => parse(&contents),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(error.into()),
        };

        Ok(Self { path, values })
    }

    /// The file the store is kept in (on the web, the name of its
    /// `localStorage` entry, without the `taim.` prefix).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The value stored under `key`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// The value stored under `key`, parsed; `None` if missing or unparsable.
    pub fn get_as<T: FromStr>(&self, key: &str) -> Option<T> {
        self.get(key)?.parse().ok()
    }

    /// Stores `value` under `key` and writes the store.
    pub fn set(&mut self, key: &str, value: impl Display) -> Result<(), Error> {
        let value = value.to_string();
        if self.values.get(key) == Some(&value) {
            return Ok(());
        }
        let previous = self.values.insert(key.to_owned(), value);
        if let Err(error) = self.save() {
            if let Some(previous) = previous {
                let _ = self.values.insert(key.to_owned(), previous);
            } else {
                let _ = self.values.remove(key);
            }
            return Err(error);
        }
        Ok(())
    }

    /// Removes `key` and writes the store. Returns the value it held.
    pub fn remove(&mut self, key: &str) -> Result<Option<String>, Error> {
        let removed = self.values.remove(key);
        if let Some(value) = &removed
            && let Err(error) = self.save()
        {
            let _ = self.values.insert(key.to_owned(), value.clone());
            return Err(error);
        }
        Ok(removed)
    }

    /// Every key, in order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }

    fn contents(&self) -> String {
        let mut contents = String::new();
        for (key, value) in &self.values {
            contents.push_str(&escape(key));
            contents.push('=');
            contents.push_str(&escape(value));
            contents.push('\n');
        }
        contents
    }

    #[cfg(target_arch = "wasm32")]
    fn save(&self) -> Result<(), Error> {
        taim_winit::platform::web_storage::write(&web_key(&self.path), &self.contents())?;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save(&self) -> Result<(), Error> {
        if let Some(directory) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(directory)?;
        }

        let contents = self.contents();

        let (temporary, mut file) = pending_write(&self.path)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary.0, &self.path)?;
        Ok(())
    }
}

// Exclusive creation prevents another writer (or a pre-existing symlink)
// from replacing the file being written. Keep it beside the destination so
// rename stays on the same filesystem. The guard removes failed writes too.
#[cfg(not(target_arch = "wasm32"))]
struct PendingWrite(PathBuf);

#[cfg(not(target_arch = "wasm32"))]
impl Drop for PendingWrite {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn pending_write(path: &Path) -> std::io::Result<(PendingWrite, std::fs::File)> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "store path needs a file name",
        )
    })?;
    for _ in 0..128 {
        let mut temporary = name.to_os_string();
        temporary.push(format!(
            ".{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let temporary = path.with_file_name(temporary);
        let mut options = std::fs::OpenOptions::new();
        let _ = options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            let _ = options.mode(0o600);
        }
        match options.open(&temporary) {
            Ok(file) => return Ok((PendingWrite(temporary), file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not reserve a temporary settings file",
    ))
}

/// The `localStorage` key for a store: prefixed, so a page's other
/// entries cannot collide with it.
#[cfg(target_arch = "wasm32")]
fn web_key(path: &Path) -> String {
    format!("taim.{}", path.to_string_lossy())
}

/// One `key=value` per line; `\`, `=`, newlines and carriage returns are
/// escaped so any string round-trips.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '=' => out.push_str("\\e"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('e') => out.push('='),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

fn parse(contents: &str) -> BTreeMap<String, String> {
    contents
        .lines()
        .filter_map(|line| {
            // The separator is the first unescaped `=`; escaped ones are
            // written as `\e`, so a plain split finds it.
            let (key, value) = line.split_once('=')?;
            Some((unescape(key), unescape(value)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("taim-store-{}-{name}", std::process::id()))
    }

    #[test]
    fn values_survive_reopening() {
        let path = temporary("reopen");
        let _ = std::fs::remove_file(&path);

        let mut store = Store::open_at(&path).expect("opens");
        store.set("theme", "dark").expect("writes");
        store.set("launches", 3).expect("writes");

        let store = Store::open_at(&path).expect("reopens");
        assert_eq!(store.get("theme"), Some("dark"));
        assert_eq!(store.get_as::<u32>("launches"), Some(3));
        assert_eq!(store.get_as::<u32>("theme"), None, "unparsable is None");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn any_text_round_trips() {
        let path = temporary("escape");
        let _ = std::fs::remove_file(&path);
        let awkward = "a=b\\c\nline two\r\nनमस्ते = hello";

        let mut store = Store::open_at(&path).expect("opens");
        store.set("k=e\\y\n", awkward).expect("writes");

        let store = Store::open_at(&path).expect("reopens");
        assert_eq!(store.get("k=e\\y\n"), Some(awkward));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn removing_a_key_persists() {
        let path = temporary("remove");
        let _ = std::fs::remove_file(&path);

        let mut store = Store::open_at(&path).expect("opens");
        store.set("a", 1).expect("writes");
        store.set("b", 2).expect("writes");
        assert_eq!(store.remove("a").expect("writes"), Some("1".to_owned()));

        let store = Store::open_at(&path).expect("reopens");
        assert_eq!(store.keys().collect::<Vec<_>>(), vec!["b"]);
        assert!(
            !path.with_extension("store.tmp").exists(),
            "no temporary left"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn failed_mutations_keep_the_snapshot_and_can_be_retried() {
        let directory = temporary("write-failure");
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("test directory");
        let path = directory.join("settings.store");
        let backup = directory.join("saved.store");
        let mut store = Store::open_at(&path).expect("opens");
        store.set("theme", "light").expect("initial write");
        store.set("removed", "keep").expect("initial write");

        // Force failure at rename, after the temporary file has been written.
        std::fs::rename(&path, &backup).expect("retain original settings");
        std::fs::create_dir(&path).expect("block destination with a directory");
        assert!(store.set("theme", "dark").is_err());
        assert_eq!(store.get("theme"), Some("light"));
        assert!(store.set("new", "retry me").is_err());
        assert_eq!(store.get("new"), None);
        assert!(store.remove("removed").is_err());
        assert_eq!(store.get("removed"), Some("keep"));
        assert_eq!(
            std::fs::read_dir(&directory).expect("list").count(),
            2,
            "failed writes leave no temporary files"
        );
        std::fs::remove_dir(&path).expect("unblock");
        std::fs::rename(&backup, &path).expect("restore settings");

        store.set("theme", "dark").expect("retry update");
        store.set("new", "retry me").expect("retry insertion");
        assert_eq!(
            store.remove("removed").expect("retry removal"),
            Some("keep".into())
        );
        let reopened = Store::open_at(&path).expect("reopen");
        assert_eq!(reopened.get("theme"), Some("dark"));
        assert_eq!(reopened.get("new"), Some("retry me"));
        assert_eq!(reopened.get("removed"), None);
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn concurrent_snapshots_publish_complete_files_without_temp_collisions() {
        let directory = temporary("concurrent");
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("test directory");
        let path = directory.join("settings.store");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let workers: Vec<_> = (0..8)
            .map(|worker| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let mut store = Store::open_at(path).expect("opens");
                    let _ = barrier.wait();
                    for sequence in 0..40 {
                        let value = format!("{worker}:{sequence}:{}", "नमस्ते=".repeat(128));
                        store.set("value", value).expect("concurrent atomic write");
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().expect("writer completes");
        }
        let store = Store::open_at(&path).expect("reopens");
        let value = store.get("value").expect("complete value");
        assert!((0..8).any(|worker| value == format!("{worker}:39:{}", "नमस्ते=".repeat(128))));
        assert_eq!(std::fs::read_dir(&directory).expect("list").count(), 1);
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn writes_do_not_follow_legacy_temp_symlinks_and_are_private() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};

        let directory = temporary("symlink");
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("test directory");
        let path = directory.join("settings.store");
        let victim = directory.join("unrelated.txt");
        std::fs::write(&victim, "do not overwrite").expect("victim");
        symlink(&victim, path.with_extension("store.tmp")).expect("old temporary path");
        let mut store = Store::open_at(&path).expect("opens");
        store.set("key", "value").expect("writes");
        assert_eq!(
            std::fs::read_to_string(victim).expect("victim intact"),
            "do not overwrite"
        );
        assert_eq!(
            std::fs::metadata(path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o077,
            0
        );
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn store_names_cannot_leave_the_data_directory() {
        for name in ["../secrets", "a/b", "", "with space"] {
            assert!(matches!(Store::open(name), Err(Error::Name(_))), "{name:?}");
        }
    }
}
