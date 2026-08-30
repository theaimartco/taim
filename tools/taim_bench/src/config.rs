//! Run configuration: which scene to run, on what simulated device, with
//! what seed, and in what cache state.

use std::time::Duration;

/// A fully resolved benchmark run configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// The scene to run (see [`crate::scenes`]).
    pub scene: String,

    /// The logical viewport size, in points.
    pub viewport: (f32, f32),

    /// The device scale factor (pixels per point).
    pub scale_factor: f32,

    /// The target refresh rate being simulated, used only to annotate the
    /// report; the headless harness does not pace itself to it.
    pub refresh_rate: RefreshRate,

    /// The deterministic seed handed to scenes that need pseudo-random but
    /// reproducible content (e.g. `ten_thousand_rows`' row order).
    pub seed: u64,

    /// The cache state a scene should start each measured run from.
    pub cache_state: CacheState,

    /// Minimum warm-up duration before measured runs begin.
    pub warm_up: Duration,

    /// Number of measured runs (phase.md requires at least 5).
    pub measured_runs: usize,

    /// Preferred headless renderer backend. `None` means "try `wgpu`,
    /// fall back to `tiny-skia`" (see [`taim_renderer::Renderer`]'s
    /// `Headless` implementation).
    pub backend: Option<String>,

    /// The named device (from `benchmarks/devices.toml`) this run claims
    /// to be executing on, recorded verbatim into the report's
    /// [`crate::report::Environment`]. `None` for an ad hoc/unregistered
    /// machine; comparisons across reports with different `device`
    /// values (or one `None` and one `Some`) should be treated as
    /// unlike environments per phase.md's "do not compare results from
    /// unlike environments."
    pub device: Option<String>,

    /// Path to a previously saved JSON report (see [`crate::report::Report`])
    /// to compare this run's median P95 against, printing the relative
    /// difference. This is what phase.md's exit gate ("two consecutive
    /// baseline runs differ by <= 5% on P95") is checked against.
    pub compare_against: Option<String>,

    /// Path to write this run's JSON report to, in addition to stdout.
    ///
    /// Saving is the point at which a report stops being a console
    /// reading and becomes an archived artifact others will compare
    /// against, so it is also the point at which
    /// [`crate::report::Environment::missing_identity`] is enforced.
    pub save_to: Option<String>,

    /// Whether to save a report that cannot be fully attributed to a
    /// tree, compiler, and named device.
    ///
    /// Off by default: an unattributable report in `benchmarks/baseline/`
    /// is indistinguishable from an attributable one at a glance, and
    /// silently becomes the number someone quotes.
    pub allow_unidentified: bool,
    /// Permits `--save` from a worktree with uncommitted changes.
    ///
    /// Separate from [`Config::allow_unidentified`] on purpose: "I cannot
    /// tell what produced this" and "I know exactly what produced this and
    /// it is uncommitted" are different admissions, and one must not
    /// silently grant the other.
    pub allow_dirty: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scene: String::from("idle_shell"),
            viewport: (1280.0, 800.0),
            scale_factor: 1.0,
            refresh_rate: RefreshRate::Hz60,
            seed: 42,
            cache_state: CacheState::Warm,
            warm_up: Duration::from_secs(30),
            measured_runs: 5,
            backend: None,
            device: None,
            compare_against: None,
            save_to: None,
            allow_unidentified: false,
            allow_dirty: false,
        }
    }
}

/// The simulated target refresh rate, used to select the gates in
/// `phase.md` section 3 that a report should be judged against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RefreshRate {
    /// 60 Hz gate.
    Hz60,
    /// 90 Hz gate.
    Hz90,
    /// 120 Hz gate.
    Hz120,
}

impl RefreshRate {
    /// The nominal frame budget for this refresh rate.
    pub fn frame_budget(self) -> Duration {
        match self {
            RefreshRate::Hz60 => Duration::from_secs_f64(1.0 / 60.0),
            RefreshRate::Hz90 => Duration::from_secs_f64(1.0 / 90.0),
            RefreshRate::Hz120 => Duration::from_secs_f64(1.0 / 120.0),
        }
    }

    /// The numeric hertz value.
    pub fn as_hz(self) -> u32 {
        match self {
            RefreshRate::Hz60 => 60,
            RefreshRate::Hz90 => 90,
            RefreshRate::Hz120 => 120,
        }
    }
}

impl std::str::FromStr for RefreshRate {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "60" => Ok(RefreshRate::Hz60),
            "90" => Ok(RefreshRate::Hz90),
            "120" => Ok(RefreshRate::Hz120),
            other => Err(format!(
                "unknown refresh rate '{other}' (expected 60, 90, or 120)"
            )),
        }
    }
}

/// The cache state a scene should be exercised in.
///
/// Cold and warm runs are expected to diverge once Phase 4/5 introduce
/// retained layers and persistent GPU resources; Phase 1 records both
/// labels now so later phases have a stable place to report the split
/// without changing the report schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheState {
    /// Every measured run starts from an empty cache (new renderer, new
    /// `UserInterface`, no warm glyph/image/layer caches).
    Cold,
    /// The cache is warmed up before measured runs begin (default).
    Warm,
}

impl std::str::FromStr for CacheState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cold" => Ok(CacheState::Cold),
            "warm" => Ok(CacheState::Warm),
            other => Err(format!(
                "unknown cache state '{other}' (expected cold or warm)"
            )),
        }
    }
}
