use rustc_hash::FxHashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, RwLock};

/// Dynamic family names become `&'static str` to keep `Font` compact and Copy.
/// A hard ceiling prevents untrusted theme data from turning that intentional
/// process-lifetime storage into an unbounded leak.
const MAX_DYNAMIC_FAMILIES: usize = 4_096;

static FAMILIES: LazyLock<RwLock<FxHashSet<&'static str>>> = LazyLock::new(RwLock::default);
static LIMIT_REPORTED: AtomicBool = AtomicBool::new(false);

pub(super) fn intern(candidate: &str) -> Option<&'static str> {
    if let Some(known) = read().get(candidate).copied() {
        return Some(known);
    }

    let mut families = FAMILIES
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    // Another thread may have inserted the same family between our locks.
    if let Some(known) = families.get(candidate).copied() {
        return Some(known);
    }

    if families.len() >= MAX_DYNAMIC_FAMILIES {
        if !LIMIT_REPORTED.swap(true, Ordering::Relaxed) {
            log::warn!(
                "TAIM font-family interner reached its {MAX_DYNAMIC_FAMILIES}-name storage limit"
            );
        }

        return None;
    }

    let stored = Box::leak(candidate.to_owned().into_boxed_str());
    let _ = families.insert(stored);
    Some(stored)
}

fn read() -> std::sync::RwLockReadGuard<'static, FxHashSet<&'static str>> {
    FAMILIES
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
