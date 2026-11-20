#!/usr/bin/env python3
"""Run the packaged sandbox fixture twice; verify storage, restart and GPU present.

This launches only com.theaimartco.taim.storeprobe and stops only processes it
starts. Existing fixture settings are retained. No personal app data is read.
"""
import argparse
import datetime as dt
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import time
import uuid

spec = importlib.util.spec_from_file_location("macos_package", Path(__file__).with_name("package.py"))
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)


def smoke(bundle, output, keep_open):
    report = {"status": "failed", "started_utc": dt.datetime.now(dt.timezone.utc).isoformat(), "checks": []}
    bundle = Path(bundle).resolve()
    output = Path(output).resolve()
    package.require(not output.exists(), "Use a new output directory")
    output.mkdir(parents=True)
    process = None
    marker = None
    try:
        report["bundle"] = package.validate_bundle(bundle)
        info = plistlib.loads((bundle / "Contents/Info.plist").read_bytes())
        bundle_id = info["CFBundleIdentifier"]
        package.require(bundle_id == "com.theaimartco.taim.storeprobe", "Smoke runner accepts only the dedicated store probe fixture")
        executable = bundle / "Contents/MacOS" / info["CFBundleExecutable"]
        directory = Path.home() / "Library/Containers" / bundle_id / "Data/Library/Application Support" / bundle_id
        previous = None
        for cycle in (1, 2):
            token = str(uuid.uuid4())
            marker = directory / (".present-" + token)
            env = dict(os.environ, TAIM_BACKEND="wgpu", TAIM_FRAME_TRACE="1",
                       TAIM_FIRST_PRESENT_MARKER=str(marker), TAIM_MACOS_PROBE_RUN=token)
            with (output / f"launch-{cycle}.log").open("wb") as log:
                process = subprocess.Popen([str(executable)], env=env, stdout=log, stderr=log)
            deadline = time.monotonic() + 30
            state = None
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise package.PackagingError(f"Probe exited early ({process.returncode}); see launch-{cycle}.log")
                try:
                    candidate = json.loads((directory / "probe-report.json").read_text())
                    if candidate.get("run_id") == token and marker.is_file():
                        state = candidate
                        break
                except (FileNotFoundError, json.JSONDecodeError):
                    pass
                time.sleep(0.1)
            package.require(state is not None, f"Launch {cycle} did not prove fresh sandbox storage and successful GPU presentation")
            package.require(state.get("storage_roundtrip") is True and state.get("expected_container") is True,
                            "Fixture did not verify container storage")
            package.require(Path(state["container_path"]).resolve() == directory.resolve(), "Storage escaped the expected app container")
            if previous is not None:
                package.require(state["launches"] == previous + 1, "Stored launch count did not survive restart")
            previous = state["launches"]
            (output / f"launch-{cycle}-storage.json").write_text(json.dumps(state, indent=2) + "\n")
            report["checks"].append(f"launch {cycle}: signed App Sandbox, private settings roundtrip, wgpu frame presented")
            marker.unlink()
            marker = None
            if cycle == 2 and keep_open:
                report["left_running_pid"] = process.pid
                process = None
            else:
                process.terminate()
                process.wait(timeout=10)
                process = None
        report["checks"].append("settings survive a new process; launch count increments exactly once")
        report["status"] = "passed"
        report["visual_input_acceptance"] = "requires user confirmation; frame presentation does not prove correct pixels"
        return report
    except (OSError, ValueError, package.PackagingError, subprocess.TimeoutExpired) as error:
        report["error"] = str(error)
        return report
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        if marker and marker.is_file():
            marker.unlink()
        report["finished_utc"] = dt.datetime.now(dt.timezone.utc).isoformat()
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundle", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--keep-open", action="store_true")
    args = parser.parse_args()
    try:
        package.require(sys.platform == "darwin", "Run on macOS")
        report = smoke(args.bundle, args.output, args.keep_open)
        print(json.dumps(report, indent=2))
        return 0 if report["status"] == "passed" else 1
    except package.PackagingError as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
