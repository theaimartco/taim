//! Explicit, versioned application-state checkpoints. Secrets are never opted in automatically.
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const MAX_BYTES: usize = 1024 * 1024;
type Snapshot = Box<dyn Fn() -> Result<Value, serde_json::Error>>;

#[derive(Default)]
pub(crate) struct Restoration {
    restored: BTreeMap<String, Value>,
    fields: BTreeMap<String, (u32, Snapshot)>,
}

/// A checkpoint cannot be represented safely or exceeds the host limit.
#[derive(Debug, thiserror::Error)]
pub enum RestorationError {
    /// A key must be nonempty, unique in its scope, and at most 128 bytes.
    #[error("restoration key is invalid or duplicated")]
    Key,
    /// Serialized state exceeded the one MiB checkpoint limit.
    #[error("restoration checkpoint exceeds one MiB")]
    TooLarge,
    /// A value could not be serialized.
    #[error("restoration serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),
}

impl Restoration {
    pub fn load(&mut self, snapshot: &str) {
        if snapshot.len() <= MAX_BYTES {
            self.restored = serde_json::from_str(snapshot).unwrap_or_default();
        }
    }
    pub fn initial<T: DeserializeOwned>(&self, key: &str, version: u32) -> Option<T> {
        let entry = self.restored.get(key)?;
        (entry.get("version")?.as_u64()? == u64::from(version))
            .then(|| serde_json::from_value(entry.get("value")?.clone()).ok())
            .flatten()
    }
    pub fn register<T: Clone + Serialize + Send + Sync + 'static>(
        &mut self,
        key: &str,
        version: u32,
        state: crate::State<T>,
    ) -> Result<(), RestorationError> {
        if key.is_empty() || key.len() > 128 || self.fields.contains_key(key) {
            return Err(RestorationError::Key);
        }
        let _ = self.fields.insert(
            key.to_owned(),
            (version, Box::new(move || serde_json::to_value(state.get()))),
        );
        Ok(())
    }
    pub fn snapshot(&self) -> Result<String, RestorationError> {
        let mut output = BTreeMap::new();
        for (key, (version, read)) in &self.fields {
            let _ = output.insert(key, json!({"version":version,"value":read()?}));
        }
        let bytes = serde_json::to_string(&output)?;
        if bytes.len() > MAX_BYTES {
            return Err(RestorationError::TooLarge);
        }
        Ok(bytes)
    }
}
