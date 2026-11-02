//! Small, persistent key-value settings, like Flutter's
//! `shared_preferences`: a theme choice, a signed-in user's name, the last
//! tab opened.
//!
//! A [`Store`] is a file in the application's private data directory
//! ([`crate::platform::data_dir`]). Every change is written at once, through
//! a temporary file renamed into place, so a crash or a killed process
//! never leaves a half-written store. It is meant for settings, not for
//! large data or records: keep those in a database or files of your own.
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

        let directory = taim_winit::platform::data_dir()?;
        Self::open_at(directory.join(format!("{name}.store")))
    }

    /// Opens (or starts) a store kept in the file at `path`.
    pub fn open_at(path: impl Into<PathBuf>) -> Result<Self, Error> {
        let path = path.into();
        let values = match std::fs::read_to_string(&path) {
            Ok(contents) => parse(&contents),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(error.into()),
        };

        Ok(Self { path, values })
    }

    /// The file the store is kept in.
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
        let _ = self.values.insert(key.to_owned(), value);
        self.save()
    }

    /// Removes `key` and writes the store. Returns the value it held.
    pub fn remove(&mut self, key: &str) -> Result<Option<String>, Error> {
        let removed = self.values.remove(key);
        if removed.is_some() {
            self.save()?;
        }
        Ok(removed)
    }

    /// Every key, in order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }

    fn save(&self) -> Result<(), Error> {
        if let Some(directory) = self.path.parent() {
            std::fs::create_dir_all(directory)?;
        }

        let mut contents = String::new();
        for (key, value) in &self.values {
            contents.push_str(&escape(key));
            contents.push('=');
            contents.push_str(&escape(value));
            contents.push('\n');
        }

        let temporary = self.path.with_extension("store.tmp");
        {
            let mut file = std::fs::File::create(&temporary)?;
            file.write_all(contents.as_bytes())?;
            file.sync_all()?;
        }
        std::fs::rename(&temporary, &self.path)?;
        Ok(())
    }
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
    fn store_names_cannot_leave_the_data_directory() {
        for name in ["../secrets", "a/b", "", "with space"] {
            assert!(matches!(Store::open(name), Err(Error::Name(_))), "{name:?}");
        }
    }
}
