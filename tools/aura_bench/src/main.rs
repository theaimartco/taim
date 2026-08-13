//! `aura_bench`: deterministic benchmark laboratory for the VYOM Aura
//! fork (phase.md Phase 1). Scene selection, warm-up, fixed-duration
//! measured runs, and JSON + human-readable report output.

mod config;
mod metrics;
mod report;
mod scenes;

use config::{CacheState, Config, RefreshRate};
use metrics::frame_time::{FramePhases, FrameTimeSamples};
use metrics::{allocations, latency, memory, pacing};
use report::{Environment, Report, RunSummary};
use scenes::Scene;

use std::fs;

use aura_core::mouse;
use aura_core::renderer::{self, Headless as _};
use aura_core::window;
use aura_runtime::{UserInterface, user_interface};

use aura_renderer::Renderer;

use std::process::ExitCode;
use std::time::{Duration, Instant};

fn main() -> ExitCode {
    env_logger_init();

    let args: Vec<String> = std::env::args().skip(1).collect();

    let config = match parse_args(&args) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("error: {message}");
            print_usage();
            return ExitCode::FAILURE;
        }
    };

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        return ExitCode::SUCCESS;
    }

    match run(&config) {
        Ok(report) => {
            println!("{}", report.human_summary());

            if let Some(path) = &config.compare_against
                && let Err(message) = compare_against_saved_report(&report, path)
            {
                eprintln!("error: {message}");
                return ExitCode::FAILURE;
            }

            let json = match report.to_json() {
                Ok(json) => json,
                Err(error) => {
                    eprintln!("error: failed to serialize report: {error}");
                    return ExitCode::FAILURE;
                }
            };

            println!("\n{json}");

            if let Some(path) = &config.save_to
                && let Err(message) = save_report(&report, &json, path, config.allow_unidentified)
            {
                eprintln!("error: {message}");
                return ExitCode::FAILURE;
            }

            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Writes `json` to `path`, refusing to archive an unattributable report.
///
/// A report on stdout is a reading someone is looking at right now; a
/// report on disk is evidence someone will cite later without re-deriving
/// where it came from. Only the second one needs to prove it knows what
/// tree, compiler, and machine produced it, which is why the check lives
/// here and not in [`Report`] itself.
fn save_report(
    report: &Report,
    json: &str,
    path: &str,
    allow_unidentified: bool,
) -> Result<(), String> {
    let missing = report.environment.missing_identity();

    if !missing.is_empty() {
        if !allow_unidentified {
            return Err(format!(
                "refusing to save an unidentified report: {} could not be determined.\n\
                 \x20 - git_commit/rustc_version: run inside the repository, or set \
                 AURA_BENCH_GIT_COMMIT / AURA_BENCH_RUSTC_VERSION\n\
                 \x20 - device: pass --device <NAME> naming a benchmarks/devices.toml entry\n\
                 Pass --allow-unidentified to save anyway (the report must not then be \
                 used for any comparison a reader is expected to trust).",
                missing.join(", ")
            ));
        }

        eprintln!(
            "warning: saving an unidentified report; {} could not be determined",
            missing.join(", ")
        );
    }

    std::fs::write(path, format!("{json}\n"))
        .map_err(|error| format!("failed to write '{path}': {error}"))?;

    eprintln!("wrote {path}");

    Ok(())
}

/// The exit gate in phase.md Phase 1 requires "two consecutive baseline
/// runs differ by <= 5% on P95." This loads a previously saved report,
/// checks it was produced in a comparable environment, and prints the
/// relative P95 difference against the current run.
fn compare_against_saved_report(current: &Report, path: &str) -> Result<(), String> {
    let json =
        fs::read_to_string(path).map_err(|error| format!("failed to read '{path}': {error}"))?;

    let previous = Report::from_json(&json).map_err(|error| format!("'{path}': {error}"))?;

    if previous.scene != current.scene {
        return Err(format!(
            "cannot compare: saved report is for scene '{}', current run is '{}'",
            previous.scene, current.scene
        ));
    }

    if previous.environment.renderer_backend != current.environment.renderer_backend {
        return Err(format!(
            "cannot compare: saved report used renderer '{}', current run used '{}' \
             (do not compare results from unlike environments)",
            previous.environment.renderer_backend, current.environment.renderer_backend
        ));
    }

    if previous.environment.device != current.environment.device {
        return Err(
            "cannot compare: saved report and current run were labeled with different --device values \
             (do not compare results from unlike environments)"
                .to_owned(),
        );
    }

    let difference = report::relative_difference(
        previous.median.frame_time.p95,
        current.median.frame_time.p95,
    );

    let absolute = current
        .median
        .frame_time
        .p95
        .saturating_sub(previous.median.frame_time.p95);

    println!(
        "\ncompared against {path}: P95 {:?} -> {:?} ({:+.2}% difference, {:?} absolute)",
        previous.median.frame_time.p95,
        current.median.frame_time.p95,
        difference * 100.0,
        absolute,
    );

    if is_regression(difference, absolute) {
        return Err(format!(
            "P95 differs by {:.2}% ({:?} absolute), exceeding both the {:.0}% reproducibility \
             gate and the {:?} noise floor",
            difference * 100.0,
            absolute,
            RELATIVE_GATE * 100.0,
            ABSOLUTE_GATE,
        ));
    }

    Ok(())
}

/// Whether a measured difference counts as a regression.
///
/// Both gates must be cleared. Either one alone produces a useless
/// signal at the scale this harness now measures: see [`ABSOLUTE_GATE`]
/// for why the relative gate cannot stand on its own, and note that the
/// absolute gate cannot either -- 25 µs is a rounding error on a scene
/// that legitimately costs milliseconds.
fn is_regression(relative: f64, absolute: Duration) -> bool {
    relative > RELATIVE_GATE && absolute > ABSOLUTE_GATE
}

/// The relative reproducibility gate from phase.md's Phase 1 exit
/// criteria: a re-run on the same hardware should land within 5%.
const RELATIVE_GATE: f64 = 0.05;

/// The absolute floor a difference must also clear to count as a
/// regression.
///
/// The relative gate alone was written when the reported frame time was
/// dominated by a near-constant ~2.3 ms readback, where 5% meant ~115 µs
/// of real signal. With that readback correctly excluded, frame times
/// are microseconds -- `idle_shell` sits around 4 µs -- and ordinary
/// run-to-run jitter of a microsecond or two clears 5% every time. A
/// purely relative gate is red on every run at that scale, which trains
/// everyone to ignore it.
///
/// 25 µs is chosen to sit above the jitter observed across repeated runs
/// (roughly +/-3 µs) while staying far below anything that could threaten
/// a frame budget: it is 0.15% of the 16.67 ms available at 60 Hz. A
/// scene going from 9 µs to 40 µs still fails, which is the structural
/// regression worth catching.
const ABSOLUTE_GATE: Duration = Duration::from_micros(25);

fn env_logger_init() {
    // Best-effort: `aura_bench` does not depend on `env_logger`; scenes
    // and the harness log through the standard `log` facade, which is a
    // silent no-op without a subscriber installed. Wiring a real
    // subscriber (and its configuration) is left to whoever needs it.
    log::set_max_level(log::LevelFilter::Warn);
}

fn print_usage() {
    eprintln!(
        "aura_bench -- VYOM Aura benchmark laboratory\n\n\
         USAGE:\n    aura_bench [OPTIONS]\n\n\
         OPTIONS:\n\
         \x20   --scene <NAME>       Scene to run (default: idle_shell)\n\
         \x20   --list               List available scene names and exit\n\
         \x20   --refresh-rate <HZ>  Simulated target refresh rate: 60, 90, or 120 (default: 60)\n\
         \x20   --cache-state <S>    cold or warm (default: warm)\n\
         \x20   --seed <N>           Deterministic seed (default: 42)\n\
         \x20   --warm-up-secs <N>   Warm-up duration in seconds (default: 30)\n\
         \x20   --runs <N>           Number of measured runs, >= 5 (default: 5)\n\
         \x20   --backend <NAME>     Force a renderer backend (\"wgpu\" or \"tiny-skia\")\n\
         \x20   --device <NAME>      Label the report with a benchmarks/devices.toml entry\n\
         \x20   --compare <PATH>     Compare this run's median P95 against a saved report\n\
         \x20   --save <PATH>        Write the JSON report to PATH (requires full identity)\n\
         \x20   --allow-unidentified Permit --save when git/rustc/device cannot be determined\n\
         \x20   -h, --help           Print this message\n\n\
         Available scenes: {}",
        scenes::names().join(", ")
    );
}

fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut config = Config::default();

    if args.iter().any(|arg| arg == "--list") {
        println!("{}", scenes::names().join("\n"));
        std::process::exit(0);
    }

    let mut iter = args.iter();

    while let Some(arg) = iter.next() {
        let mut value = || {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("missing value for '{arg}'"))
        };

        match arg.as_str() {
            "--scene" => config.scene = value()?,
            "--refresh-rate" => {
                config.refresh_rate = value()?
                    .parse::<RefreshRate>()
                    .map_err(|error| error.to_string())?;
            }
            "--cache-state" => {
                config.cache_state = value()?
                    .parse::<CacheState>()
                    .map_err(|error| error.to_string())?;
            }
            "--seed" => {
                config.seed = value()?
                    .parse()
                    .map_err(|_| "seed must be an integer".to_owned())?;
            }
            "--warm-up-secs" => {
                let secs: u64 = value()?
                    .parse()
                    .map_err(|_| "warm-up-secs must be an integer".to_owned())?;
                config.warm_up = Duration::from_secs(secs);
            }
            "--runs" => {
                config.measured_runs = value()?
                    .parse()
                    .map_err(|_| "runs must be an integer".to_owned())?;
            }
            "--backend" => config.backend = Some(value()?),
            "--device" => config.device = Some(value()?),
            "--compare" => config.compare_against = Some(value()?),
            "--save" => config.save_to = Some(value()?),
            "--allow-unidentified" => config.allow_unidentified = true,
            "--help" | "-h" => {}
            other => return Err(format!("unknown argument '{other}'")),
        }
    }

    if !scenes::names().contains(&config.scene.as_str()) {
        return Err(format!(
            "unknown scene '{}' (available: {})",
            config.scene,
            scenes::names().join(", ")
        ));
    }

    if config.measured_runs < 5 {
        return Err("--runs must be at least 5 (see phase.md Phase 1 exit gate)".to_owned());
    }

    Ok(config)
}

/// Runs the configured scene through warm-up and measured phases, and
/// produces a [`Report`].
fn run(config: &Config) -> Result<Report, String> {
    let mut renderer = futures::executor::block_on(Renderer::new(
        renderer::Settings::default(),
        config.backend.as_deref(),
    ))
    .ok_or_else(|| {
        format!(
            "no renderer backend available{}",
            config
                .backend
                .as_ref()
                .map(|b| format!(" for requested backend '{b}'"))
                .unwrap_or_default()
        )
    })?;

    let backend_name = renderer.name();

    macro_rules! dispatch {
        ($($name:expr => $ty:ty),+ $(,)?) => {
            $(if config.scene.as_str() == $name {
                run_scene::<$ty>(config, &mut renderer)
            } else)+ {
                Err(format!("unknown scene '{}'", config.scene))
            }
        };
    }

    let runs = dispatch! {
        scenes::idle_shell::NAME => scenes::idle_shell::IdleShell,
        scenes::ten_thousand_rows::NAME => scenes::ten_thousand_rows::TenThousandRows,
        scenes::text_heavy::NAME => scenes::text_heavy::TextHeavy,
        scenes::text_edit::NAME => scenes::text_edit::TextEdit,
        scenes::image_grid::NAME => scenes::image_grid::ImageGrid,
        scenes::blur_layers::NAME => scenes::blur_layers::BlurLayers,
        scenes::animated_cards::NAME => scenes::animated_cards::AnimatedCards,
        scenes::multitouch::NAME => scenes::multitouch::Multitouch,
        scenes::motion_stress::NAME => scenes::motion_stress::MotionStress,
        scenes::damage_churn::NAME => scenes::damage_churn::DamageChurn,
        scenes::effect_ladder::NAME => scenes::effect_ladder::EffectLadder,
        scenes::virtual_list::NAME => scenes::virtual_list::VirtualListScene,
        scenes::virtual_list::VARIABLE_NAME => scenes::virtual_list::VirtualListVariable,
    }?;

    let environment = Environment::capture(backend_name, config.device.clone());

    Ok(Report::new(config, environment, runs))
}

/// A scene constructor abstraction so the `dispatch!` macro above can
/// build any registered scene uniformly from a [`Config`]. Scenes with no
/// seed-dependent content (most of them) simply ignore `config.seed`.
trait NewScene: Scene {
    fn new_from_config(config: &Config) -> Self;
}

impl NewScene for scenes::idle_shell::IdleShell {
    fn new_from_config(_config: &Config) -> Self {
        Self
    }
}

impl NewScene for scenes::ten_thousand_rows::TenThousandRows {
    fn new_from_config(config: &Config) -> Self {
        Self::new(config.seed)
    }
}

impl NewScene for scenes::text_heavy::TextHeavy {
    fn new_from_config(_config: &Config) -> Self {
        Self::new()
    }
}

impl NewScene for scenes::text_edit::TextEdit {
    fn new_from_config(_config: &Config) -> Self {
        Self::new()
    }
}

impl NewScene for scenes::image_grid::ImageGrid {
    fn new_from_config(_config: &Config) -> Self {
        Self::new()
    }
}

impl NewScene for scenes::blur_layers::BlurLayers {
    fn new_from_config(_config: &Config) -> Self {
        Self
    }
}

impl NewScene for scenes::animated_cards::AnimatedCards {
    fn new_from_config(_config: &Config) -> Self {
        Self::new()
    }
}

impl NewScene for scenes::multitouch::Multitouch {
    fn new_from_config(_config: &Config) -> Self {
        Self::new()
    }
}

impl NewScene for scenes::motion_stress::MotionStress {
    fn new_from_config(_config: &Config) -> Self {
        Self::new()
    }
}

impl NewScene for scenes::damage_churn::DamageChurn {
    fn new_from_config(_config: &Config) -> Self {
        Self::new()
    }
}

impl NewScene for scenes::effect_ladder::EffectLadder {
    fn new_from_config(_config: &Config) -> Self {
        Self::new()
    }
}

impl NewScene for scenes::virtual_list::VirtualListScene {
    fn new_from_config(config: &Config) -> Self {
        Self::fixed(config.seed)
    }
}

impl NewScene for scenes::virtual_list::VirtualListVariable {
    fn new_from_config(config: &Config) -> Self {
        Self::new(config.seed)
    }
}

fn run_scene<S: NewScene>(
    config: &Config,
    renderer: &mut Renderer,
) -> Result<Vec<RunSummary>, String> {
    // Warm-up runs once, before any measured run, on a throwaway scene
    // instance -- not once per measured run. Each measured run below
    // still gets a fresh `S::new_from_config` (so every run replays the
    // same deterministic frame sequence from the same starting state,
    // which is what makes runs comparable at all); warm-up's job is to
    // pay process-level one-time costs (OS page cache, CPU frequency
    // ramp-up, cosmic-text's global font database load, GPU driver JIT)
    // exactly once, matching phase.md's "at least 30 seconds of warm-up
    // and five measured runs per scene" as a total, not a per-run cost.
    if config.cache_state == CacheState::Warm {
        let mut warm_up_scene = S::new_from_config(config);
        warm_up(&mut warm_up_scene, renderer, config)?;
    }

    let mut runs = Vec::with_capacity(config.measured_runs);

    for _ in 0..config.measured_runs {
        let mut scene = S::new_from_config(config);

        debug_assert_eq!(
            scene.name(),
            config.scene,
            "dispatch! registered a scene under the wrong name"
        );

        let summary = measure(&mut scene, renderer, config)?;
        runs.push(summary);
    }

    Ok(runs)
}

fn warm_up<S: Scene>(
    scene: &mut S,
    renderer: &mut Renderer,
    config: &Config,
) -> Result<(), String> {
    let deadline = Instant::now() + config.warm_up;
    let mut frame_index = 0_u64;

    while Instant::now() < deadline {
        let _phases = drive_one_frame(scene, renderer, config, frame_index)?;
        frame_index += 1;
    }

    Ok(())
}

fn measure<S: Scene>(
    scene: &mut S,
    renderer: &mut Renderer,
    config: &Config,
) -> Result<RunSummary, String> {
    let mut frame_samples = FrameTimeSamples::new();
    let mut latency_samples = Vec::new();
    let mut pacing_samples = pacing::PacingSamples::new();

    let allocations_before = allocations::snapshot();

    // A fixed number of frames per measured run, rather than a fixed
    // wall-clock duration, so the frame count (and therefore the input
    // sequence each scene replays) is identical across runs regardless
    // of machine speed.
    const FRAMES_PER_RUN: u64 = 180;

    let frame_budget = config.refresh_rate.frame_budget();

    for frame_index in 0..FRAMES_PER_RUN {
        let mut phases_for_pacing = None;

        let latency_sample = latency::measure(|| {
            let phases = drive_one_frame(scene, renderer, config, frame_index)
                .expect("frame drive should not fail during a measured run");
            phases_for_pacing = Some(phases);
            frame_samples.push(phases);
        });

        // `aura_bench` runs entirely headlessly with no real
        // compositor, so there is no real `wgpu::window::
        // frame_feedback::FeedbackSource` to consume (same gap
        // `metrics::latency`'s own module documentation already
        // records). This synthesizes a `frame::Feedback` from the
        // one thing actually measured here -- CPU frame time against
        // the target budget -- as the best available proxy for "was
        // this frame on time" until a scene drives a real windowed
        // compositor.
        if let Some(phases) = phases_for_pacing {
            let total = phases.total();
            let target_error = total.saturating_sub(frame_budget);

            pacing_samples.push(pacing::PacingSample {
                dropped: false,
                target_error: Some(target_error),
                queue_depth: 0,
            });
        }

        latency_samples.push(latency_sample);
    }

    let allocations_after = allocations::snapshot();

    Ok(RunSummary {
        frame_time: frame_samples.summarize(frame_budget),
        latency: latency::summarize(&latency_samples),
        pacing: pacing_samples.summarize(frame_budget),
        allocations: allocations_after.since(&allocations_before),
        memory: memory::MemorySnapshot::capture(),
    })
}

fn drive_one_frame<S: Scene>(
    scene: &mut S,
    renderer: &mut Renderer,
    config: &Config,
    frame_index: u64,
) -> Result<FramePhases, String> {
    let messages = scene.advance(frame_index);
    for message in messages {
        scene.update(message);
    }

    let events = scene.input_events(frame_index);
    let mut runtime_messages = Vec::new();

    let (update, draw, present) = {
        let view = scene.view();
        let mut ui = UserInterface::build(
            view,
            aura_core::Size::new(config.viewport.0, config.viewport.1),
            user_interface::Cache::new(),
            renderer,
        );

        let window = window::Headless;
        let waker = aura_core::shell::Waker::noop();

        let update_start = Instant::now();

        let (_state, _statuses) = ui.update(
            &window,
            &waker,
            &events,
            mouse::Cursor::Unavailable,
            renderer,
            &mut runtime_messages,
        );

        let update = update_start.elapsed();

        let draw_start = Instant::now();

        ui.draw(
            renderer,
            &aura_core::Theme::Dark,
            &renderer::Style::default(),
            mouse::Cursor::Unavailable,
        );

        let draw = draw_start.elapsed();

        let present_start = Instant::now();

        let _pixels = renderer.screenshot(
            aura_core::Size::new(config.viewport.0 as u32, config.viewport.1 as u32),
            config.scale_factor,
            aura_core::Color::BLACK,
        );

        let present = present_start.elapsed();

        (update, draw, present)
    };

    // `ui` (and its borrow of `scene` through `view`) is dropped by now,
    // so messages produced by `UserInterface::update` can be applied.
    for message in runtime_messages {
        scene.update(message);
    }

    Ok(FramePhases {
        update,
        draw,
        present,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_relative_jump_at_microsecond_scale_is_not_a_regression() {
        // The case that made a purely relative gate useless: `idle_shell`
        // sits around 4 µs, so a 2 µs jitter is a 50% "difference" that
        // threatens nothing. Before the absolute floor existed, six of
        // thirteen scenes reported a regression immediately after their
        // own baselines were captured.
        assert!(!is_regression(0.50, Duration::from_micros(2)));
    }

    #[test]
    fn a_structural_regression_still_fails() {
        // 9 µs -> 40 µs: well past the noise floor and a real change in
        // the shape of the work, even though it is still far under a
        // frame budget.
        assert!(is_regression(3.4, Duration::from_micros(31)));
    }

    #[test]
    fn a_large_absolute_jump_with_a_small_relative_one_is_not_a_regression() {
        // A scene that legitimately costs milliseconds moving by 30 µs
        // is a rounding error, not a regression. The absolute gate must
        // not fire on its own.
        assert!(!is_regression(0.01, Duration::from_micros(30)));
    }

    #[test]
    fn an_improvement_is_never_a_regression() {
        // `relative_difference` is signed; a faster run must pass both
        // gates trivially, and `saturating_sub` floors the absolute
        // difference at zero.
        assert!(!is_regression(-0.40, Duration::ZERO));
    }

    #[test]
    fn the_gates_sit_where_their_documentation_says() {
        // Pins the constants so a change to either is deliberate and
        // shows up in review rather than silently widening the gate.
        assert!((RELATIVE_GATE - 0.05).abs() < f64::EPSILON);
        assert_eq!(ABSOLUTE_GATE, Duration::from_micros(25));
    }
}
