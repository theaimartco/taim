//! Event-to-submit latency correlation.
//!
//! `phase.md` asks for "event timestamp to submit and to compositor
//! presentation feedback." Phase 1 runs entirely headlessly with no real
//! compositor, so there is no presentation feedback to correlate against
//! yet (that channel is Phase 6's `wgpu/src/window/frame_feedback.rs`).
//! This module measures the part that *is* available headlessly: wall
//! clock from "event handed to `UserInterface::update`" to "readback from
//! `Renderer::screenshot` returned", which stands in for "event to
//! submit" until real presentation feedback exists.

use std::time::{Duration, Instant};

/// One event-to-present latency measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LatencySample {
    /// Time from the event being dispatched to the readback completing.
    pub event_to_present: Duration,
}

/// Measures the latency of a single event-driven frame.
///
/// `drive` is called with nothing and must perform the full
/// event-dispatch-through-present cycle for one frame; the returned
/// duration covers exactly that call.
pub fn measure(drive: impl FnOnce()) -> LatencySample {
    let start = Instant::now();
    drive();
    let event_to_present = start.elapsed();

    LatencySample { event_to_present }
}

/// Summary statistics over a set of latency samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LatencySummary {
    /// Number of samples the summary was computed over.
    pub sample_count: usize,
    /// Median event-to-present latency.
    pub p50: Duration,
    /// 95th percentile event-to-present latency.
    pub p95: Duration,
}

/// Summarizes a set of latency samples into percentiles.
pub fn summarize(samples: &[LatencySample]) -> LatencySummary {
    let mut values: Vec<Duration> = samples.iter().map(|s| s.event_to_present).collect();
    values.sort_unstable();

    LatencySummary {
        sample_count: values.len(),
        p50: percentile(&values, 0.50),
        p95: percentile(&values, 0.95),
    }
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }

    let rank = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn measure_captures_a_positive_duration() {
        let sample = measure(|| {
            thread::sleep(Duration::from_millis(1));
        });

        assert!(sample.event_to_present >= Duration::from_millis(1));
    }

    #[test]
    fn summarize_handles_empty_input() {
        let summary = summarize(&[]);

        assert_eq!(summary.sample_count, 0);
        assert_eq!(summary.p50, Duration::ZERO);
        assert_eq!(summary.p95, Duration::ZERO);
    }
}
