//! Stable schema and missing-metric failure tests for `taim_bench`'s
//! JSON report format.
//!
//! This test drives the actual `taim_bench` binary and parses its real
//! JSON output (rather than unit-testing `taim_bench`'s internal `Report`
//! type directly), because the contract phase.md asks for here --
//! "JSON report is schema-tested" -- is about what a consumer of the
//! report file sees, which is exactly this stdout stream. `taim_bench`
//! is a bin-only crate (see `tools/taim_bench/Cargo.toml`), so its types
//! are not otherwise reachable from an integration test in this
//! directory.

use std::process::Command;

/// Required top-level fields the report schema must always produce.
/// Extend this list (never shrink it without a `schema_version` bump)
/// when `tools/taim_bench/src/report.rs` gains a new required field.
const REQUIRED_TOP_LEVEL_FIELDS: &[&str] = &[
    "schema_version",
    "scene",
    "refresh_rate",
    "cache_state",
    "seed",
    "viewport",
    "scale_factor",
    "environment",
    "runs",
    "median",
];

const REQUIRED_ENVIRONMENT_FIELDS: &[&str] = &[
    "git_commit",
    "rustc_version",
    "renderer_backend",
    "build_profile",
    "device",
    "os",
    "allocation_counting_active",
];

const REQUIRED_RUN_SUMMARY_FIELDS: &[&str] = &["frame_time", "latency", "allocations", "memory"];

const REQUIRED_FRAME_TIME_FIELDS: &[&str] = &[
    "sample_count",
    "p50",
    "p95",
    "p99",
    "missed_frame_rate",
    "build_p95",
    "update_p95",
    "draw_p95",
    "present_p95",
];

fn run_taim_bench(args: &[&str]) -> serde_json::Value {
    let output = Command::new(env!("CARGO"))
        .args(["run", "--quiet", "--package", "taim_bench", "--"])
        .args(args)
        .output()
        .expect("failed to run taim_bench");

    assert!(
        output.status.success(),
        "taim_bench exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("taim_bench stdout was not valid UTF-8");

    let json_start = stdout
        .find('{')
        .expect("taim_bench stdout did not contain a JSON report");

    serde_json::from_str(&stdout[json_start..]).expect("taim_bench stdout was not valid JSON")
}

fn assert_has_fields(value: &serde_json::Value, fields: &[&str], context: &str) {
    let object = value
        .as_object()
        .unwrap_or_else(|| panic!("{context} was not a JSON object"));

    for field in fields {
        assert!(
            object.contains_key(*field),
            "{context} is missing required field '{field}'"
        );
    }
}

#[test]
fn report_has_every_required_top_level_field() {
    let report = run_taim_bench(&[
        "--scene",
        "idle_shell",
        "--warm-up-secs",
        "0",
        "--runs",
        "5",
    ]);

    assert_has_fields(&report, REQUIRED_TOP_LEVEL_FIELDS, "report");
    assert_eq!(
        report["schema_version"], 2,
        "schema_version must be 2 until a breaking change bumps it"
    );
}

#[test]
fn report_environment_has_every_required_field() {
    let report = run_taim_bench(&[
        "--scene",
        "idle_shell",
        "--warm-up-secs",
        "0",
        "--runs",
        "5",
    ]);

    assert_has_fields(
        &report["environment"],
        REQUIRED_ENVIRONMENT_FIELDS,
        "report.environment",
    );

    let backend = report["environment"]["renderer_backend"]
        .as_str()
        .expect("renderer_backend must be a string");

    assert!(
        backend == "wgpu" || backend == "tiny-skia",
        "renderer_backend was '{backend}', expected 'wgpu' or 'tiny-skia'"
    );
}

#[test]
fn report_has_the_configured_number_of_runs_each_with_every_required_field() {
    let report = run_taim_bench(&[
        "--scene",
        "idle_shell",
        "--warm-up-secs",
        "0",
        "--runs",
        "5",
    ]);

    let runs = report["runs"]
        .as_array()
        .expect("runs must be a JSON array");
    assert_eq!(
        runs.len(),
        5,
        "requested --runs 5 but got {} runs",
        runs.len()
    );

    for (index, run) in runs.iter().enumerate() {
        assert_has_fields(run, REQUIRED_RUN_SUMMARY_FIELDS, &format!("runs[{index}]"));
        assert_has_fields(
            &run["frame_time"],
            REQUIRED_FRAME_TIME_FIELDS,
            &format!("runs[{index}].frame_time"),
        );
    }

    assert_has_fields(&report["median"], REQUIRED_RUN_SUMMARY_FIELDS, "median");
}

#[test]
fn refusing_fewer_than_five_runs_is_enforced_by_the_binary() {
    let output = Command::new(env!("CARGO"))
        .args(["run", "--quiet", "--package", "taim_bench", "--"])
        .args(["--scene", "idle_shell", "--runs", "1"])
        .output()
        .expect("failed to run taim_bench");

    assert!(
        !output.status.success(),
        "taim_bench should reject --runs below phase.md's 5-run minimum"
    );
}

#[test]
fn unknown_scene_name_fails_cleanly_rather_than_panicking() {
    let output = Command::new(env!("CARGO"))
        .args(["run", "--quiet", "--package", "taim_bench", "--"])
        .args(["--scene", "does_not_exist"])
        .output()
        .expect("failed to run taim_bench");

    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown scene"),
        "expected a clean 'unknown scene' error, got: {stderr}"
    );
}

#[test]
fn every_registered_scene_produces_a_schema_valid_report() {
    let list_output = Command::new(env!("CARGO"))
        .args(["run", "--quiet", "--package", "taim_bench", "--", "--list"])
        .output()
        .expect("failed to list taim_bench scenes");

    assert!(list_output.status.success());

    let scenes = String::from_utf8(list_output.stdout).expect("scene list was not valid UTF-8");

    for scene in scenes.lines().filter(|line| !line.is_empty()) {
        let report = run_taim_bench(&["--scene", scene, "--warm-up-secs", "0", "--runs", "5"]);

        assert_has_fields(
            &report,
            REQUIRED_TOP_LEVEL_FIELDS,
            &format!("report for scene '{scene}'"),
        );
        assert_eq!(report["scene"], scene);
    }
}
