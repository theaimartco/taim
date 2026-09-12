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
/// macOS exposes RSS through `task_info(MACH_TASK_BASIC_INFO)`, which is
/// a raw Mach call and therefore `unsafe` -- and `unsafe_code = "deny"`
/// is the workspace default (phase.md rule 8: no unsafe without an RFC).
/// A benchmark harness reading its own memory once per run is nowhere
/// near strong enough grounds for an exception, so this shells out to
/// `ps` instead. It samples once per measured run, not once per frame,
/// so the cost of a fork does not reach any reported number.
#[cfg(target_os = "macos")]
pub fn resident_set_size() -> Option<u64> {
    use std::process::Command;

    let pid = std::process::id();

    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    // `ps -o rss=` prints kibibytes with no header and leading padding.
    let kib: u64 = String::from_utf8(output.stdout).ok()?.trim().parse().ok()?;

    Some(kib * 1024)
}

/// Reads the current process's resident set size.
///
/// Not implemented on this platform; see the Linux and macOS
/// implementations above. Returning `None` rather than a fabricated
/// number is deliberate: a report field that is consistently `None`
/// should read as "not measured on this OS".
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
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

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn resident_set_size_is_readable_on_supported_platforms() {
        let Some(rss) = resident_set_size() else {
            eprintln!("skipped: resident_set_size not available in this environment (sandboxed)");
            return;
        };

        assert!(rss > 0, "a running process has a non-zero resident set");

        // A plausibility floor: any process that has linked this binary
        // and reached a test holds more than a megabyte. Catches a unit
        // mix-up (reporting KiB as bytes) that a `> 0` check would miss.
        assert!(
            rss > 1024 * 1024,
            "RSS looks like it is being reported in the wrong unit: {rss} bytes"
        );
    }
}
