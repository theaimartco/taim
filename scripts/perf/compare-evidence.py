#!/usr/bin/env python3
"""Validate and summarize SAME-device, real-frame benchmark evidence; do not acquire data."""

import argparse
import hashlib
import json
import math
from pathlib import Path
import sys

FRAMEWORKS = ("taim", "iced", "slint", "egui")
HARDWARE = ("machine_id", "kernel", "cpu", "gpu", "gpu_driver", "compositor",
            "display", "resolution", "scale", "refresh_hz", "power_mode", "governor")
WORKLOAD = ("scene", "scene_revision", "interaction_replay_sha256", "duration_seconds")


def positive(value, field):
    if type(value) not in (int, float) or not math.isfinite(value) or value <= 0:
        raise ValueError(f"{field} must be a finite positive number")
    return value


def nearest_rank(samples, p):
    ordered = sorted(samples)
    return ordered[max(0, math.ceil(len(ordered) * p / 100) - 1)]


def sample_summary(samples):
    if samples is None:
        return None
    return dict(samples=len(samples), p50=nearest_rank(samples, 50),
                p95=nearest_rank(samples, 95), p99=nearest_rank(samples, 99))


def validate_trace(report_path, evidence, path_key, hash_key, samples):
    """Verify the actual trace bytes and their numerical agreement with the manifest."""
    rel = evidence.get(path_key)
    expected = evidence.get(hash_key)
    if not isinstance(rel, str) or not rel or not isinstance(expected, str):
        raise ValueError(f"{report_path}: missing {path_key} or {hash_key}")
    if len(expected) != 64 or any(char not in "0123456789abcdef" for char in expected.lower()):
        raise ValueError(f"{report_path}: {hash_key} must be SHA-256 hex")
    folder = Path(report_path).resolve().parent
    target = (folder / rel).resolve()
    if not target.is_relative_to(folder) or not target.is_file():
        raise ValueError(f"{report_path}: trace missing or escapes report directory: {rel}")
    if target.stat().st_size > 64 * 1024 * 1024:
        raise ValueError(f"{report_path}: trace exceeds 64 MiB evidence limit")
    content = target.read_bytes()
    if hashlib.sha256(content).hexdigest() != expected.lower():
        raise ValueError(f"{report_path}: {path_key} hash mismatch")
    try:
        observed = [float(line) for line in content.decode("utf-8").splitlines()]
    except (ValueError, UnicodeDecodeError) as error:
        raise ValueError(f"{report_path}: malformed numeric trace") from error
    if len(observed) != len(samples):
        raise ValueError(f"{report_path}: trace sample count disagrees with manifest")
    for measured, recorded in zip(observed, samples):
        positive(measured, f"{path_key} physical trace")
        if not math.isclose(measured, recorded, rel_tol=1e-12, abs_tol=1e-9):
            raise ValueError(f"{report_path}: trace values disagree with manifest")
    return expected.lower()


def validate_scalar_source(report_path, evidence, field):
    """Require a present SHA-256-pinned source for any non-null scalar claim."""
    key = field + "_source"
    digest_key = field + "_sha256"
    rel = evidence.get(key)
    expected = evidence.get(digest_key)
    if not isinstance(rel, str) or not rel or not isinstance(expected, str):
        raise ValueError(f"{report_path}: non-null {field} needs {key} and {digest_key}")
    if len(expected) != 64 or any(c not in "0123456789abcdef" for c in expected.lower()):
        raise ValueError(f"{report_path}: invalid {digest_key}")
    folder = Path(report_path).resolve().parent
    target = (folder / rel).resolve()
    if not target.is_relative_to(folder) or not target.is_file():
        raise ValueError(f"{report_path}: evidence file missing or outside report folder")
    if target.stat().st_size > 64 * 1024 * 1024:
        raise ValueError(f"{report_path}: source file too large")
    if hashlib.sha256(target.read_bytes()).hexdigest() != expected.lower():
        raise ValueError(f"{report_path}: evidence hash mismatch for {field}")


def read_report(path, minimum):
    report = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(report, dict) or report.get("framework") not in FRAMEWORKS:
        raise ValueError(f"{path}: unknown framework")
    for field, keys in (("hardware", HARDWARE), ("workload", WORKLOAD)):
        block = report.get(field)
        if not isinstance(block, dict):
            raise ValueError(f"{path}: {field} missing")
        for key in keys:
            if block.get(key) in (None, ""):
                raise ValueError(f"{path}: missing {field}.{key}")
    positive(report["hardware"]["refresh_hz"], "refresh_hz")
    positive(report["workload"]["duration_seconds"], "duration_seconds")
    build = report.get("build")
    if not isinstance(build, dict) or build.get("release") is not True or not build.get("source_sha") or not build.get("compiler"):
        raise ValueError(f"{path}: release build, commit SHA and compiler required")
    evidence = report.get("evidence")
    if not isinstance(evidence, dict) or not evidence.get("frame_source") or not evidence.get("raw_trace_path"):
        raise ValueError(f"{path}: real frame trace provenance required")
    metrics = report.get("metrics")
    if not isinstance(metrics, dict):
        raise ValueError(f"{path}: metrics missing")
    frames = metrics.get("presented_frame_interval_ms")
    if not isinstance(frames, list) or len(frames) < minimum:
        raise ValueError(f"{path}: need >= {minimum} raw presented frame intervals")
    for n in frames:
        positive(n, "presented_frame_interval_ms")
    validate_trace(path, evidence, "raw_trace_path", "raw_trace_sha256", frames)
    latency = metrics.get("input_to_photon_ms")
    if latency is not None:
        if not isinstance(latency, list) or len(latency) < 30:
            raise ValueError(f"{path}: need >= 30 calibrated latency samples")
        for n in latency:
            positive(n, "input_to_photon_ms")
        if not evidence.get("latency_sensor"):
            raise ValueError(f"{path}: latency sensor provenance required")
        validate_trace(path, evidence, "latency_trace_path", "latency_trace_sha256", latency)
    scalars = {}
    for name in ("rss_bytes", "gpu_memory_bytes", "idle_power_watts"):
        value = metrics.get(name)
        if value is not None:
            positive(value, name)
            validate_scalar_source(path, evidence, name)
        scalars[name] = value
    return {
        "framework": report["framework"],
        "hardware": report["hardware"], "workload": report["workload"],
        "build": build, "evidence": evidence,
        "frame_ms": sample_summary(frames),
        "input_to_photon_ms": sample_summary(latency),
        **scalars,
    }


def compare(paths, minimum=1000, require_all=True):
    if len(paths) < 2:
        raise ValueError("Need at least two framework reports")
    reports = [read_report(path, minimum) for path in paths]
    result = {report["framework"]: report for report in reports}
    if len(result) != len(reports):
        raise ValueError("Duplicate framework in one comparison")
    if require_all and set(result) != set(FRAMEWORKS):
        raise ValueError(f"Require exactly {FRAMEWORKS}")
    reference = reports[0]
    for report in reports[1:]:
        if report["hardware"] != reference["hardware"]:
            raise ValueError("Different hardware, display, driver or power settings")
        if report["workload"] != reference["workload"]:
            raise ValueError("Different scene or interaction replay")
    return {
        "schema_version": 1,
        "status": "schema_validated_measurements_not_independently_verified",
        "hardware": reference["hardware"], "workload": reference["workload"],
        "frameworks": result,
        "limitations": [
            "Raw source and sensor authenticity must be reviewed independently.",
            "Presentation intervals are not CPU frame costs or physical input latency.",
            "Unavailable metrics are null, never fabricated as zero.",
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reports", nargs="+", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--min-frames", type=int, default=1000)
    parser.add_argument("--allow-partial", action="store_true")
    args = parser.parse_args()
    if args.min_frames < 30:
        parser.error("min-frames must be at least 30")
    try:
        data = compare(args.reports, args.min_frames, not args.allow_partial)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"Rejected: {error}", file=sys.stderr)
        return 1
    for name in FRAMEWORKS:
        if name in data["frameworks"]:
            p = data["frameworks"][name]["frame_ms"]
            print(f"{name}: n={p['samples']} p50={p['p50']:.3f}ms p95={p['p95']:.3f}ms p99={p['p99']:.3f}ms")
    print("Schema checked only; no physical performance independently certified.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
