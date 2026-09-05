//! Opt-in marker consumed by `scripts/perf/measure-app.py`.

use std::sync::Once;

pub(crate) fn first_present() {
    static FIRST: Once = Once::new();
    FIRST.call_once(|| {
        if let Some(path) = std::env::var_os("TAIM_FIRST_PRESENT_MARKER") {
            // The runner creates a private directory. Never replace a pre-existing
            // file, including a symlink. File existence is the complete protocol.
            if let Err(error) = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
                log::warn!("Could not record first successful present: {error}");
            }
        }
    });
}
