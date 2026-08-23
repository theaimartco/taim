//! Captures reproducibility metadata at compile time that isn't available
//! to a running binary any other way: the exact git commit and rustc
//! version used to build it. `report::Environment::capture` reads these
//! back via `option_env!("AURA_BENCH_GIT_COMMIT")` /
//! `option_env!("AURA_BENCH_RUSTC_VERSION")`.
//!
//! Both are best-effort: if `git` isn't available (e.g. building from a
//! source tarball with no `.git`) or the invocation fails, the
//! corresponding `cargo:rustc-env` is simply not emitted and the report
//! field stays `None`, matching the existing "if known" contract in
//! `report::Environment`.

use std::process::Command;

fn main() {
    if let Some(commit) = git_commit() {
        println!("cargo:rustc-env=AURA_BENCH_GIT_COMMIT={commit}");
    }

    if let Some(rustc_version) = rustc_version() {
        println!("cargo:rustc-env=AURA_BENCH_RUSTC_VERSION={rustc_version}");
    }

    // Re-run only when the commit or rustc changes, not on every build.
    //
    // `.git/HEAD` alone is not enough: on a branch it holds `ref: refs/...`,
    // whose *contents* do not change when you commit, so the recorded commit
    // silently goes stale until you switch branches. Watch the ref HEAD
    // resolves to as well, and the index, which every commit rewrites.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");

    if let Some(reference) = head_reference() {
        println!("cargo:rerun-if-changed=../../.git/{reference}");
    }

    println!("cargo:rerun-if-env-changed=AURA_BENCH_GIT_COMMIT");
    println!("cargo:rerun-if-env-changed=AURA_BENCH_RUSTC_VERSION");
}

/// The ref path `HEAD` points at (e.g. `refs/heads/main`), or `None` when
/// `HEAD` is detached or unreadable.
fn head_reference() -> Option<String> {
    let head = std::fs::read_to_string("../../.git/HEAD").ok()?;

    head.trim().strip_prefix("ref: ").map(str::to_owned)
}

fn git_commit() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let commit = String::from_utf8(output.stdout).ok()?;
    let commit = commit.trim();

    // A worktree with uncommitted changes still builds a real commit's
    // sources plus local edits; mark that explicitly rather than let the
    // report imply the checked-in commit alone was measured (see
    // `scripts/perf/run-baseline.sh`'s `require_clean_worktree_or_warn`).
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false);

    Some(if dirty {
        format!("{commit}-dirty")
    } else {
        commit.to_owned()
    })
}

fn rustc_version() -> Option<String> {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());

    let output = Command::new(rustc).arg("--version").output().ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()
        .map(|s| s.trim().to_owned())
}
