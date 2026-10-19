#!/usr/bin/env python3
"""Validate and summarize SAME-device, real-frame benchmark evidence; do not acquire data."""

import argparse
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
    latency = metrics.get("input_to_photon_ms")
    if latency is not None:
        if not isinstance(latency, list) or len(latency) < 30:
            raise ValueError(f"{path}: need >= 30 calibrated latency samples")
        for n in latency:
            positive(n, "input_to_photon_ms")
        if not evidence.get("latency_sensor"):
            raise ValueError(f"{path}: latency sensor provenance required")
    scalars = {}
    for name in ("rss_bytes", "gpu_memory_bytes", "idle_power_watts"):
        value = metrics.get(name)
        scalars[name] = None if value is None else positive(value, name)
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
