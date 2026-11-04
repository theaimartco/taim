//! Durable, deferrable Android jobs and the activity-independent Rust worker contract.
use serde::Deserialize;
use serde_json::{Value, json};

/// OS network constraint. This does not add INTERNET permission to the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkNetwork {
    /// No network constraint.
    #[default]
    None,
    /// Any connected network.
    Connected,
    /// An unmetered network.
    Unmetered,
    /// A network not marked as roaming.
    NotRoaming,
    /// A metered network.
    Metered,
}
/// OS scheduling constraints; timing remains controlled by Android quotas and power policy.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkConstraints {
    /// Required network class.
    pub network: WorkNetwork,
    /// Require charging.
    pub charging: bool,
    /// Require battery not low.
    pub battery_not_low: bool,
    /// Require storage not low.
    pub storage_not_low: bool,
}
/// Repeated delivery policy for retryable outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkBackoff {
    /// Linear backoff, with an initial delay of 10,000–18,000,000 ms.
    Linear(u64),
    /// Exponential backoff, with the same bounded initial delay.
    Exponential(u64),
}
/// Deferrable timing, not an exact alarm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkSchedule {
    /// Run no earlier than this delay (0–7 days).
    Once {
        /// Minimum delay in milliseconds.
        delay_ms: u64,
    },
    /// Repeat at an OS-managed interval (15 minutes–7 days).
    Periodic {
        /// Interval in milliseconds.
        interval_ms: u64,
        /// Flex window, at least 5 minutes or 5% of the interval, whichever is larger.
        flex_ms: u64,
    },
}
/// Collision policy for an existing active logical job ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkPolicy {
    /// Return the existing active generation unchanged.
    Keep,
    /// Cancel the old generation and schedule a new one; old output is fenced out.
    Replace,
}
/// A persistent Android JobScheduler job. Use small JSON inputs and idempotent workers.
#[derive(Debug, Clone)]
pub struct WorkSpec {
    /// Logical app-local ID, 1–4095; reserved OS IDs are mapped by the native host.
    pub id: u16,
    /// Application-defined handler name, 1–64 ASCII letters/digits/dots/dashes/underscores.
    pub worker: String,
    /// JSON object, at most 16 KiB. This is stored in the app-private job ledger.
    pub input: Value,
    /// Deferrable execution timing.
    pub schedule: WorkSchedule,
    /// OS execution constraints.
    pub constraints: WorkConstraints,
    /// Retry backoff policy.
    pub backoff: WorkBackoff,
    /// Delivery-attempt limit, 1–16 per one-off job or periodic retry cycle.
    pub maximum_attempts: u8,
    /// Cooperative execution deadline, 1,000–120,000 ms.
    pub execution_timeout_ms: u32,
    /// Survive device reboot through JobScheduler; requires RECEIVE_BOOT_COMPLETED.
    pub persisted: bool,
    /// Active-ID collision policy.
    pub policy: WorkPolicy,
}
impl WorkSpec {
    /// One-off job with keep-existing, persistence, three attempts and a 60s cooperative deadline.
    pub fn once(id: u16, worker: impl Into<String>, input: Value) -> Self {
        Self {
            id,
            worker: worker.into(),
            input,
            schedule: WorkSchedule::Once { delay_ms: 0 },
            constraints: WorkConstraints::default(),
            backoff: WorkBackoff::Exponential(30_000),
            maximum_attempts: 3,
            execution_timeout_ms: 60_000,
            persisted: true,
            policy: WorkPolicy::Keep,
        }
    }
    pub(super) fn wire(self) -> Option<Value> {
        if !(1..=4095).contains(&self.id)
            || self.worker.is_empty()
            || self.worker.len() > 64
            || !self
                .worker
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            || !self.input.is_object()
            || self.input.to_string().len() > 16 * 1024
            || !(1..=16).contains(&self.maximum_attempts)
            || !(1_000..=120_000).contains(&self.execution_timeout_ms)
        {
            return None;
        }
        let (delay, interval, flex) = match self.schedule {
            WorkSchedule::Once { delay_ms } if delay_ms <= 604_800_000 => (delay_ms, None, None),
            WorkSchedule::Periodic {
                interval_ms,
                flex_ms,
            } if (900_000..=604_800_000).contains(&interval_ms)
                && flex_ms >= 300_000_u64.max(interval_ms / 20)
                && flex_ms <= interval_ms =>
            {
                (0, Some(interval_ms), Some(flex_ms))
            }
            _ => return None,
        };
        let (policy, backoff) = match self.backoff {
            WorkBackoff::Linear(ms) => ("linear", ms),
            WorkBackoff::Exponential(ms) => ("exponential", ms),
        };
        if !(10_000..=18_000_000).contains(&backoff) {
            return None;
        }
        Some(
            json!({"op":"workEnqueue","id":self.id,"worker":self.worker,"input":self.input,"delay":delay,"interval":interval,"flex":flex,
            "network":match self.constraints.network {WorkNetwork::None=>"none",WorkNetwork::Connected=>"connected",WorkNetwork::Unmetered=>"unmetered",WorkNetwork::NotRoaming=>"notRoaming",WorkNetwork::Metered=>"metered"},
            "charging":self.constraints.charging,"batteryNotLow":self.constraints.battery_not_low,"storageNotLow":self.constraints.storage_not_low,
            "backoffPolicy":policy,"backoff":backoff,"maxAttempts":self.maximum_attempts,"timeout":self.execution_timeout_ms,
            "persisted":self.persisted,"policy":match self.policy {WorkPolicy::Keep=>"keep",WorkPolicy::Replace=>"replace"}}),
        )
    }
}
/// Durable worker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkState {
    /// Awaiting OS execution.
    Queued,
    /// A delivery has started; abrupt process death can leave this until recovery.
    Running,
    /// Awaiting a retry.
    Retry,
    /// One-off job completed successfully.
    Succeeded,
    /// Failed permanently or exhausted attempts.
    Failed,
    /// Explicitly cancelled.
    Cancelled,
}
/// Immutable input for a single delivery, loaded without a UI Activity.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkInput {
    /// Logical job ID.
    pub id: u16,
    /// Replacement generation; useful for application fencing/idempotency keys.
    pub generation: String,
    /// Application-defined handler name.
    pub worker: String,
    /// Attempt number, starting at one.
    pub attempt: u32,
    /// Small JSON object supplied at enqueue.
    pub input: Value,
    /// App-private files directory in this process.
    pub files_dir: String,
    /// App-private cache directory.
    pub cache_dir: String,
    /// Assigned OS network handle on API 28+, when available. No global process binding is performed.
    pub network_handle: Option<u64>,
}
/// Worker outcome. Retried jobs may execute more than once; external effects must be idempotent.
#[derive(Debug, Clone)]
pub enum WorkResult {
    /// Success output object, at most 16 KiB.
    Success(Value),
    /// Permanent failure output object, at most 16 KiB.
    Failure(Value),
    /// Retry under the configured attempt/backoff policy.
    Retry,
}
impl WorkResult {
    pub(crate) fn wire(self) -> Value {
        let (outcome, output) = match self {
            Self::Success(v) => ("success", v),
            Self::Failure(v) => ("failure", v),
            Self::Retry => ("retry", json!({})),
        };
        if !output.is_object() || output.to_string().len() > 16 * 1024 {
            return json!({"outcome":"failure","output":{"code":"invalidOutput"}});
        }
        json!({"outcome":outcome,"output":output})
    }
}
/// Progress/cancellation bridge error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WorkError {
    /// No assigned network, invalid descriptor, or socket-network binding failed.
    #[error("background network unavailable")]
    NetworkUnavailable,
    /// The execution has been stopped or replaced.
    #[error("background work stopped")]
    Stopped,
    /// Progress must be an object of at most 4 KiB.
    #[error("invalid background progress")]
    InvalidProgress,
    /// Progress was throttled or the bounded manager queue is busy.
    #[error("background progress busy")]
    Busy,
}
pub(crate) trait WorkHost {
    fn stopped(&mut self) -> bool;
    fn progress(&mut self, value: Value) -> bool;
    fn bind_socket(&mut self, fd: i32) -> bool;
}
/// Synchronous, activity-independent worker context. Check cancellation around bounded I/O chunks.
pub struct WorkContext<'a> {
    /// Immutable delivery information and application paths.
    pub input: WorkInput,
    host: &'a mut dyn WorkHost,
}
impl<'a> WorkContext<'a> {
    #[cfg(target_os = "android")]
    pub(crate) fn new(input: WorkInput, host: &'a mut dyn WorkHost) -> Self {
        Self { input, host }
    }
    /// True after cancellation, replacement, OS stop, timeout or service destruction.
    pub fn is_stopped(&mut self) -> bool {
        self.host.stopped()
    }
    /// Bind an unconnected socket to this delivery's assigned network, without changing UI/process routing.
    pub fn bind_socket(&mut self, fd: i32) -> Result<(), WorkError> {
        if self.host.stopped() {
            return Err(WorkError::Stopped);
        }
        if fd < 0 || !self.host.bind_socket(fd) {
            return Err(WorkError::NetworkUnavailable);
        }
        Ok(())
    }
    /// Queue bounded progress. Native delivery is throttled to at most four updates/second.
    pub fn set_progress(&mut self, value: Value) -> Result<(), WorkError> {
        if !value.is_object() || value.to_string().len() > 4096 {
            return Err(WorkError::InvalidProgress);
        }
        if self.host.stopped() {
            return Err(WorkError::Stopped);
        }
        if self.host.progress(value) {
            Ok(())
        } else if self.host.stopped() {
            Err(WorkError::Stopped)
        } else {
            Err(WorkError::Busy)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn work_rejects_invalid_identity_payload_and_attempts() {
        for id in [0, 4096] {
            assert!(WorkSpec::once(id, "sync", json!({})).wire().is_none());
        }
        let mut spec = WorkSpec::once(1, "bad/handler", json!({}));
        assert!(spec.clone().wire().is_none());
        spec.worker = "sync".into();
        spec.input = json!([1]);
        assert!(spec.clone().wire().is_none());
        spec.input = json!({"data":"🌍".repeat(4096)});
        assert!(spec.clone().wire().is_none());
        spec.input = json!({});
        spec.maximum_attempts = 0;
        assert!(spec.wire().is_none());
    }
    #[test]
    fn timing_respects_os_periodic_flex_and_backoff_bounds() {
        let mut spec = WorkSpec::once(1, "sync", json!({}));
        for schedule in [
            WorkSchedule::Once {
                delay_ms: 604_800_001,
            },
            WorkSchedule::Periodic {
                interval_ms: 899_999,
                flex_ms: 300_000,
            },
            WorkSchedule::Periodic {
                interval_ms: 900_000,
                flex_ms: 299_999,
            },
        ] {
            spec.schedule = schedule;
            assert!(spec.clone().wire().is_none());
        }
        spec.schedule = WorkSchedule::Periodic {
            interval_ms: 900_000,
            flex_ms: 300_000,
        };
        assert!(spec.clone().wire().is_some());
        spec.backoff = WorkBackoff::Linear(9999);
        assert!(spec.wire().is_none());
    }
    #[test]
    fn invalid_worker_output_is_a_bounded_permanent_failure() {
        assert_eq!(
            WorkResult::Success(json!("not an object")).wire()["output"]["code"],
            "invalidOutput"
        );
        assert_eq!(WorkResult::Retry.wire()["outcome"], "retry");
    }
}
