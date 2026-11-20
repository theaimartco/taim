//! Stress tests for Taim.
//! Headless contract checks; these do not establish OS/device acceptance.
#[path = "apps/support.rs"]
mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Lifetime(Arc<AtomicUsize>);

impl Lifetime {
    fn new(live: &Arc<AtomicUsize>) -> Self {
        let _ = live.fetch_add(1, Ordering::SeqCst);
        Self(live.clone())
    }
}

impl Drop for Lifetime {
    fn drop(&mut self) {
        let _ = self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

mod stress {
    mod navigation;
    mod resource_pressure;
    mod window_lifecycle;
}
