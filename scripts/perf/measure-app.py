#!/usr/bin/env python3
"""Measure a launched Linux application, not the in-process benchmark harness."""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import signal
import statistics
import subprocess
import tempfile
import time


def positive(value):
    value = float(value)
    if not math.isfinite(value) or value <= 0:
        raise argparse.ArgumentTypeError("must be finite and positive")
    return value


def rss_bytes(pid):
    try:
        for line in Path(f"/proc/{pid}/status").read_text().splitlines():
            if line.startswith("VmRSS:"):
                return int(line.split()[1]) * 1024
    except (OSError, ValueError):
        pass
    return None


class EnergyCounter:
    """One explicitly selected powercap domain; never sum overlapping domains."""

    def __init__(self, path):
        self.path = Path(path)
        self.maximum = int((self.path.parent / "max_energy_range_uj").read_text())
        if self.maximum <= 0:
            raise ValueError("energy range must be positive")
        self.previous = self.read()
        self.total = 0

    def read(self):
        value = int(self.path.read_text())
        if not 0 <= value < self.maximum:
            raise ValueError("energy counter outside advertised range")
        return value

    def sample(self):
        current = self.read()
        self.total += (current - self.previous) % self.maximum
        self.previous = current


def stop(process):
    # The child owns a fresh process group. Clean up its helpers too, even if
    # the group leader has already exited. Never signal the runner's group.
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=2)
    except subprocess.TimeoutExpired:
        pass
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait()


def measure(args, command):
    report = {
        "schema_version": 1,
        "command": command,
        "platform": platform.platform(),
        "cache_state": args.cache_state,
        "workload": args.workload,
        "first_present_observed_upper_bound_ms": None,
        "first_pixel_ms": None,
        "rss_scope": "launched process only; excludes helpers and GPU allocations",
        "steady_rss_bytes": None,
        "energy": None,
        "status": "failed",
    }
    executable = Path(command[0]).resolve(strict=True)
    report["executable"] = str(executable)
    command = [str(executable), *command[1:]]
    with tempfile.TemporaryDirectory(prefix="taim-measure-") as directory:
        marker = Path(directory) / "first-present"
        environment = dict(os.environ, TAIM_FIRST_PRESENT_MARKER=str(marker))
        started = time.monotonic()
        process = subprocess.Popen(command, env=environment, start_new_session=True)
        try:
            deadline = started + args.startup_timeout
            while not marker.is_file():
                if process.poll() is not None:
                    raise RuntimeError(f"application exited before present: {process.returncode}")
                if time.monotonic() >= deadline:
                    raise RuntimeError("first-present marker timed out; verify the measurement feature and a working display")
                time.sleep(0.005)
            observed = time.monotonic()
            report["first_present_observed_upper_bound_ms"] = (observed - started) * 1000
            warm_until = observed + args.warmup
            while time.monotonic() < warm_until:
                if process.poll() is not None:
                    raise RuntimeError("application exited during warmup")
                time.sleep(min(args.sample_interval, max(0, warm_until - time.monotonic())))

            counter = EnergyCounter(args.energy_counter) if args.energy_counter else None
            samples = []
            measured_from = time.monotonic()
            deadline = measured_from + args.duration
            while True:
                if process.poll() is not None:
                    raise RuntimeError("application exited during measurement")
                rss = rss_bytes(process.pid)
                if rss is None:
                    raise RuntimeError("application RSS unavailable")
                samples.append(rss)
                if counter:
                    counter.sample()
                now = time.monotonic()
                if now >= deadline:
                    break
                time.sleep(min(args.sample_interval, deadline - now))
            duration = now - measured_from
            report["steady_rss_bytes"] = {
                "minimum": min(samples),
                "median": statistics.median(samples),
                "maximum": max(samples),
                "samples": len(samples),
            }
            report["measured_seconds"] = duration
            if counter:
                joules = counter.total / 1_000_000
                report["energy"] = {
                    "counter": str(counter.path.resolve()),
                    "scope": "selected hardware domain; includes other processes; no idle subtraction",
                    "joules": joules,
                    "average_watts": joules / duration,
                    "projected_wh_per_hour": joules / duration,
                    "limitation": "assumes fewer than one counter wrap between samples",
                }
            report["status"] = "ok"
        except (OSError, ValueError, RuntimeError) as error:
            report["error"] = str(error)
        finally:
            stop(process)
    # Hash after the experiment so reading the binary does not warm its cache
    # before launch. The executable must remain unchanged during the run.
    with executable.open("rb") as binary:
        report["executable_sha256"] = hashlib.file_digest(binary, "sha256").hexdigest()
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--duration", type=positive, default=60.0)
    parser.add_argument("--warmup", type=positive, default=10.0)
    parser.add_argument("--startup-timeout", type=positive, default=30.0)
    parser.add_argument("--sample-interval", type=positive, default=0.1)
    parser.add_argument("--cache-state", choices=["uncontrolled", "cold", "warm"], default="uncontrolled")
    parser.add_argument("--workload", required=True, help="describe the actual interaction or idle workload")
    parser.add_argument("--energy-counter", type=Path, help="explicit powercap energy_uj file")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("provide an executable path after --")
    if platform.system() != "Linux":
        parser.error("this runner currently requires Linux /proc")
    if args.sample_interval > args.duration:
        parser.error("sample interval must not exceed duration")
    try:
        report = measure(args, command)
    except (OSError, ValueError) as error:
        report = {"schema_version": 1, "command": command, "status": "failed", "error": str(error)}
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    return 0 if report["status"] == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
