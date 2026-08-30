//! Versioned JSON report schema and human-readable summary output.
//!
//! `phase.md`'s exit gate requires "JSON report is schema-tested"; the
//! schema version here is a plain integer bumped on any breaking field
//! change, checked by `tests/bench/report_schema.rs`.

use crate::config::{CacheState, Config, RefreshRate};
use crate::metrics::allocations::AllocationSnapshot;
use crate::metrics::frame_time::FrameTimeSummary;
use crate::metrics::latency::LatencySummary;
use crate::metrics::memory::MemorySnapshot;
use crate::metrics::pacing::PacingSummary;

use std::time::Duration;

/// The current report schema version. Bump this and update
/// `tests/bench/report_schema.rs` whenever a field is removed, renamed,
/// or changes meaning (adding an optional field does not require a bump).
pub const SCHEMA_VERSION: u32 = 1;

/// A complete, self-describing benchmark report for one scene run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Report {
    /// The report schema version this value was produced with.
    pub schema_version: u32,

    /// The scene and configuration this report was produced from.
    pub scene: String,
    pub refresh_rate: RefreshRate,
    pub cache_state: CacheState,
    pub seed: u64,
    pub viewport: (f32, f32),
    pub scale_factor: f32,

    /// Reproducibility metadata, required by phase.md section: "store raw
    /// samples, median-of-runs summary, commit, compiler, GPU driver,
    /// build flags, power mode, and hardware ID."
    pub environment: Environment,

    /// One summary per measured run (phase.md requires >= 5).
    pub runs: Vec<RunSummary>,

    /// Median-of-runs summary across all `runs`, the number CI/regression
    /// gates should read.
    pub median: RunSummary,
}

/// Everything needed to judge whether two reports are comparable.
///
/// `phase.md` section "Measurements required": "Do not compare results
/// from unlike environments." This struct is the machine-checkable form
/// of "environment" that a comparison tool should assert matches before
/// treating two reports as comparable.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Environment {
    /// The git commit this report was built from, if known.
    pub git_commit: Option<String>,
    /// The rustc version string used to build `taim_bench`.
    pub rustc_version: Option<String>,
    /// The renderer backend actually selected at runtime (e.g. `"wgpu"`
    /// or `"tiny-skia"`), not just requested.
    pub renderer_backend: String,
    /// `cfg!(debug_assertions)`-derived build profile label.
    pub build_profile: String,
    /// The named device from `benchmarks/devices.toml`, if the run was
    /// invoked with `--device`; `None` for an ad hoc/unregistered machine.
    pub device: Option<String>,
    /// Free-form OS description (`std::env::consts::OS` plus, on Linux,
    /// as much of `/etc/os-release` as is easily available).
    pub os: String,
    /// Whether the `profiling` feature (and therefore the counting
    /// allocator backing [`crate::metrics::allocations`]) was active in
    /// this build. When `false`, every [`RunSummary::allocations`] field
    /// in this report reads as all-zero because it was never measured,
    /// not because zero allocations occurred.
    pub allocation_counting_active: bool,
}

impl Environment {
    /// Captures what can be determined from the current process without
    /// requiring external tools (`nvidia-smi`, etc.) to be installed.
    ///
    /// `git_commit` and `rustc_version` come from `build.rs`, which reads
    /// them from the tree and compiler that actually produced this
    /// binary. The same variables are also read at runtime as a fallback,
    /// which is how a build system that already knows them injects them
    /// without a rebuild.
    ///
    /// Neither is re-derived here by running `git` or `rustc`: this
    /// process may be running on a different machine, at a different
    /// commit, under a different toolchain than the one that built it,
    /// and an identity field that is confidently wrong is worse than one
    /// that is honestly absent. A field stays `None` when both sources
    /// failed -- an unidentified report. [`Environment::missing_identity`]
    /// names those fields, and `--save` refuses to write such a report
    /// unless the caller opts in.
    pub fn capture(renderer_backend: String, device: Option<String>) -> Self {
        Self {
            git_commit: resolve_identity(
                option_env!("TAIM_BENCH_GIT_COMMIT"),
                "TAIM_BENCH_GIT_COMMIT",
            ),
            rustc_version: resolve_identity(
                option_env!("TAIM_BENCH_RUSTC_VERSION"),
                "TAIM_BENCH_RUSTC_VERSION",
            ),
            renderer_backend,
            build_profile: if cfg!(debug_assertions) {
                "debug".to_owned()
            } else {
                "release".to_owned()
            },
            device,
            os: std::env::consts::OS.to_owned(),
            allocation_counting_active: crate::metrics::allocations::is_active(),
        }
    }

    /// The suffix `build.rs` appends to `git_commit` when the worktree
    /// that produced this binary had uncommitted changes.
    pub const DIRTY_SUFFIX: &'static str = "-dirty";

    /// Whether this report was produced from a worktree with uncommitted
    /// changes.
    ///
    /// A `-dirty` report names a commit that does **not** describe the
    /// sources that were measured, so nobody -- including its author --
    /// can reconstruct the tree it came from. That makes it useless as a
    /// baseline, which is why `--save` refuses one by default.
    pub fn is_dirty(&self) -> bool {
        self.git_commit
            .as_deref()
            .is_some_and(|commit| commit.ends_with(Self::DIRTY_SUFFIX))
    }

    /// Names the identity fields that could not be determined.
    ///
    /// An empty slice means the report can be attributed to a specific
    /// tree, compiler, and machine, which is the precondition for using
    /// it in any comparison a reader is expected to trust. `--save`
    /// refuses to write a report with a non-empty result unless
    /// `--allow-unidentified` is passed.
    pub fn missing_identity(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();

        if self.git_commit.is_none() {
            missing.push("git_commit");
        }

        if self.rustc_version.is_none() {
            missing.push("rustc_version");
        }

        if self.device.is_none() {
            missing.push("device");
        }

        missing
    }
}

/// Resolves one identity field from a build-time variable, falling back to
/// the same variable read at runtime.
///
/// An empty value counts as absent: an unknown provenance must stay
/// visibly unknown rather than become an empty string that serializes as
/// a present-looking field.
fn resolve_identity(built_in: Option<&'static str>, variable: &str) -> Option<String> {
    built_in
        .map(str::to_owned)
        .or_else(|| std::env::var(variable).ok())
        .filter(|value| !value.is_empty())
}

/// The summary produced from one measured run's raw frame samples.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RunSummary {
    pub frame_time: FrameTimeSummary,
    pub latency: LatencySummary,
    /// Cadence/jitter/queue-depth/missed-frame pacing summary (Phase 6).
    ///
    /// `taim_bench` runs entirely headlessly with no real compositor, so
    /// this is synthesized from CPU frame time against the target
    /// budget rather than real presentation feedback -- see
    /// `metrics::pacing`'s own module documentation. Still a genuine,
    /// real signal (a CPU-bound frame over budget will miss its target
    /// regardless of what the compositor does with it afterward), just
    /// not the same measurement a real windowed run would produce.
    pub pacing: PacingSummary,
    pub allocations: AllocationSnapshot,
    pub memory: MemorySnapshot,
}

impl Report {
    /// Builds a report from a configuration, the renderer actually used,
    /// and one [`RunSummary`] per measured run.
    ///
    /// # Panics
    ///
    /// Panics if `runs` is empty; a report with zero measured runs cannot
    /// produce a median and does not satisfy phase.md's five-run minimum.
    pub fn new(config: &Config, environment: Environment, runs: Vec<RunSummary>) -> Self {
        assert!(
            !runs.is_empty(),
            "a report requires at least one measured run"
        );

        let median = median_of(&runs);

        Self {
            schema_version: SCHEMA_VERSION,
            scene: config.scene.clone(),
            refresh_rate: config.refresh_rate,
            cache_state: config.cache_state,
            seed: config.seed,
            viewport: config.viewport,
            scale_factor: config.scale_factor,
            environment,
            runs,
            median,
        }
    }

    /// Serializes this report as pretty-printed JSON.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Parses a report from JSON, checking the schema version matches
    /// what this build of `taim_bench` understands.
    pub fn from_json(json: &str) -> Result<Self, ReportError> {
        let report: Self = serde_json::from_str(json)?;

        if report.schema_version != SCHEMA_VERSION {
            return Err(ReportError::SchemaMismatch {
                found: report.schema_version,
                expected: SCHEMA_VERSION,
            });
        }

        Ok(report)
    }

    /// A short, human-readable summary suitable for terminal output.
    pub fn human_summary(&self) -> String {
        let m = &self.median;

        format!(
            "scene: {scene} ({cache_state:?} cache, {hz} Hz gate)\n\
             renderer: {backend}\n\
             runs: {run_count} (frames/run: {frame_count})\n\
             frame time  p50={p50:?}  p95={p95:?}  p99={p99:?}   (framework: update+draw)\n\
             phases      update_p95={update_p95:?}  draw_p95={draw_p95:?}\n\
             missed frame rate: {missed:.3}%\n\
             latency     p50={lat_p50:?}  p95={lat_p95:?}\n\
             harness     readback_p95={present_p95:?}  (excluded from frame time)\n\
             rss: {rss}\n\
             allocations: {allocations}",
            scene = self.scene,
            cache_state = self.cache_state,
            hz = self.refresh_rate.as_hz(),
            backend = self.environment.renderer_backend,
            run_count = self.runs.len(),
            frame_count = m.frame_time.sample_count,
            p50 = m.frame_time.p50,
            p95 = m.frame_time.p95,
            p99 = m.frame_time.p99,
            update_p95 = m.frame_time.update_p95,
            draw_p95 = m.frame_time.draw_p95,
            present_p95 = m.frame_time.present_p95,
            missed = m.frame_time.missed_frame_rate * 100.0,
            lat_p50 = m.latency.p50,
            lat_p95 = m.latency.p95,
            rss = m
                .memory
                .rss_bytes
                .map(|bytes| format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0)))
                .unwrap_or_else(|| "not measured on this platform".to_owned()),
            // Without the counting allocator the counters are never
            // incremented, so every field reads zero. Printing those
            // zeros unqualified would look like a measurement of "no
            // allocations", which is the opposite of the truth.
            allocations = if self.environment.allocation_counting_active {
                format!(
                    "{} allocs, {} frees, {:.1} MiB allocated",
                    m.allocations.allocations,
                    m.allocations.deallocations,
                    m.allocations.bytes_allocated as f64 / (1024.0 * 1024.0)
                )
            } else {
                "not measured (rebuild with --features profiling)".to_owned()
            },
        )
    }
}

/// Errors produced while parsing a stored report.
#[derive(Debug)]
pub enum ReportError {
    /// The JSON did not match the [`Report`] shape at all.
    Invalid(serde_json::Error),
    /// The JSON parsed, but its `schema_version` does not match what this
    /// build of `taim_bench` understands.
    SchemaMismatch { found: u32, expected: u32 },
}

impl From<serde_json::Error> for ReportError {
    fn from(error: serde_json::Error) -> Self {
        ReportError::Invalid(error)
    }
}

impl std::fmt::Display for ReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReportError::Invalid(error) => write!(f, "invalid report JSON: {error}"),
            ReportError::SchemaMismatch { found, expected } => write!(
                f,
                "report schema version {found} does not match expected {expected}"
            ),
        }
    }
}

impl std::error::Error for ReportError {}

/// Computes the median run by total `p95` frame time, matching
/// phase.md's "median-of-runs summary" requirement. Ties resolve to the
/// lower-indexed of the two middle runs for determinism.
fn median_of(runs: &[RunSummary]) -> RunSummary {
    let mut indices: Vec<usize> = (0..runs.len()).collect();
    indices.sort_by_key(|&i| runs[i].frame_time.p95);

    let mid = indices.len() / 2;
    runs[indices[mid]]
}

/// Compares two [`FrameTimeSummary`] P95s and returns the relative
/// difference as a fraction (e.g. `0.05` for a 5% difference), used by
/// the exit-gate check ("two consecutive baseline runs differ by <= 5%
/// on P95").
pub fn relative_difference(a: Duration, b: Duration) -> f64 {
    let (a, b) = (a.as_secs_f64(), b.as_secs_f64());

    if a == 0.0 && b == 0.0 {
        return 0.0;
    }

    let larger = a.max(b);
    let smaller = a.min(b);

    (larger - smaller) / larger
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::frame_time::FrameTimeSummary;

    fn sample_run(p95_millis: u64) -> RunSummary {
        RunSummary {
            frame_time: FrameTimeSummary {
                sample_count: 100,
                p50: Duration::from_millis(p95_millis / 2),
                p95: Duration::from_millis(p95_millis),
                p99: Duration::from_millis(p95_millis + 1),
                missed_frame_rate: 0.0,
                update_p95: Duration::from_millis(p95_millis / 2),
                draw_p95: Duration::from_millis(p95_millis / 4),
                present_p95: Duration::from_millis(p95_millis / 4),
            },
            latency: LatencySummary {
                sample_count: 100,
                p50: Duration::from_millis(1),
                p95: Duration::from_millis(2),
            },
            pacing: PacingSummary {
                sample_count: 100,
                dropped_count: 0,
                missed_count: 0,
                missed_rate: 0.0,
                mean_target_error: Duration::ZERO,
                jitter: Duration::ZERO,
                jitter_ratio: 0.0,
                max_queue_depth: 0,
                mean_queue_depth: 0.0,
            },
            allocations: AllocationSnapshot::default(),
            memory: MemorySnapshot::default(),
        }
    }

    #[test]
    fn round_trips_through_json() {
        let config = Config::default();
        let environment = Environment::capture("tiny-skia".to_owned(), None);
        let report = Report::new(&config, environment, vec![sample_run(10); 5]);

        let json = report.to_json().expect("serialize");
        let parsed = Report::from_json(&json).expect("deserialize");

        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        assert_eq!(parsed.scene, report.scene);
        assert_eq!(parsed.runs.len(), 5);
    }

    #[test]
    fn rejects_a_future_schema_version() {
        let config = Config::default();
        let environment = Environment::capture("tiny-skia".to_owned(), None);
        let mut report = Report::new(&config, environment, vec![sample_run(10)]);
        report.schema_version = SCHEMA_VERSION + 1;

        let json = report.to_json().expect("serialize");
        let result = Report::from_json(&json);

        assert!(matches!(result, Err(ReportError::SchemaMismatch { .. })));
    }

    #[test]
    fn median_of_picks_the_middle_run_by_p95() {
        let runs = vec![sample_run(10), sample_run(30), sample_run(20)];
        let median = median_of(&runs);

        assert_eq!(median.frame_time.p95, Duration::from_millis(20));
    }

    #[test]
    fn relative_difference_is_symmetric_and_bounded() {
        let a = Duration::from_millis(100);
        let b = Duration::from_millis(105);

        let diff = relative_difference(a, b);

        assert!(diff > 0.0 && diff < 0.05);
        assert_eq!(relative_difference(a, b), relative_difference(b, a));
    }

    #[test]
    fn new_panics_on_empty_runs() {
        let config = Config::default();
        let environment = Environment::capture("tiny-skia".to_owned(), None);

        let result = std::panic::catch_unwind(|| Report::new(&config, environment, vec![]));

        assert!(result.is_err());
    }
}
