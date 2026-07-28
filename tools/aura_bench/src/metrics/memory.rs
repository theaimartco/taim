//! Process memory (RSS) and renderer resource snapshots.

/// A memory snapshot at a point in time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MemorySnapshot {
    /// Resident set size, in bytes. `None` if the platform's RSS could
    /// not be read (see [`resident_set_size`]).
    pub rss_bytes: Option<u64>,
}

impl MemorySnapshot {
    /// Takes a snapshot of current process memory.
    pub fn capture() -> Self {
        Self {
            rss_bytes: resident_set_size(),
        }
    }
}

/// Reads the current process's resident set size.
///
/// Only Linux is supported today, via `/proc/self/status`'s `VmRSS`
/// field; other platforms return `None` rather than a fabricated number.
/// A report field that is consistently `None` on a given platform should
/// read as "not measured on this OS", matching the honesty requirement
/// in `phase.md` section 18 ("publish ... limitations").
#[cfg(target_os = "linux")]
pub fn resident_set_size() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;

    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let kib: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kib * 1024);
        }
    }

    None
}

/// Reads the current process's resident set size.
///
/// Not implemented on this platform; see the Linux implementation above.
#[cfg(not(target_os = "linux"))]
pub fn resident_set_size() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_does_not_panic() {
        let _ = MemorySnapshot::capture();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn resident_set_size_is_nonzero_on_linux() {
        let rss = resident_set_size().expect("VmRSS should be readable in a normal process");
        assert!(rss > 0);
    }
}
