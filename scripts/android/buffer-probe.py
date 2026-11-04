#!/usr/bin/env python3
"""Run the independent Vulkan buffer-copy APK on a selected Android test device."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

from counter import APPS, ROOT, verify_apk_package


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial", required=True)
    parser.add_argument("--apk", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--timeout", type=float, default=25)
    args = parser.parse_args()
    if not args.apk.is_file() or args.timeout <= 0:
        parser.error("Provide an existing probe APK and a positive timeout")
    sdk = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
    if not sdk:
        parser.error("Set ANDROID_HOME to the Android SDK directory")
    args.output.mkdir(parents=True, exist_ok=True)
    if any(args.output.iterdir()):
        parser.error("Choose a new empty output directory")
    adb = [str(Path(sdk) / "platform-tools/adb"), "-s", args.serial]
    package = APPS["gpu-probe"][1]

    def call(*cmd):
        result = subprocess.run(adb + list(cmd), capture_output=True, text=True, timeout=30, check=True)
        return result.stdout.strip()

    report = {"status": "running", "started_utc": datetime.now(timezone.utc).isoformat(),
              "serial": args.serial, "apk_sha256": hashlib.sha256(args.apk.read_bytes()).hexdigest()}
    logs = ""
    since = None
    try:
        verify_apk_package(Path(sdk), args.apk, package)
        report["device"] = {"sdk": call("shell", "getprop", "ro.build.version.sdk"),
                            "fingerprint": call("shell", "getprop", "ro.build.fingerprint"),
                            "page_size": int(call("shell", "getconf", "PAGE_SIZE"))}
        report["source"] = {"git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                            "working_tree_dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT))}
        call("install", "--no-incremental", "-r", str(args.apk.resolve()))
        call("shell", "am", "force-stop", package)
        # Read the guest clock, not the host clock. No global log buffer is cleared.
        since = call("shell", "date '+%m-%d %H:%M:%S.%3N'")
        call("shell", "am", "start", "-W", "-n", f"{package}/android.app.NativeActivity")
        deadline = time.monotonic() + args.timeout
        while time.monotonic() < deadline:
            logs = call("logcat", "-d", "-T", since, "-s", "RustStdoutStderr")
            # The GLES pass follows the Vulkan pass; wait for both (or a crash).
            if "GPU_PROBE_GL_RESULT" in logs or "panicked at" in logs:
                break
            time.sleep(0.2)
        checks = re.findall(r"GPU_PROBE(_BELT)? iteration=(\d+) ok=(true|false)", logs)
        expected = [(mode, str(index)) for mode in ("", "_BELT") for index in range(8)]
        complete = [(mode, index) for mode, index, _ok in checks] == expected
        report["copy_checks"] = [{"path": "staging_belt" if mode else "mapped_init" if index == "0" else "queue_write",
                                   "iteration": int(index), "passed": ok == "true"} for mode, index, ok in checks]
        passed = complete and all(ok == "true" for _mode, _index, ok in checks) and "GPU_PROBE_RESULT passed=true" in logs
        report["status"] = "passed" if passed else "failed"
        # Informational: the same checks through the GLES backend, isolating the Vulkan path.
        gl = re.findall(r"GPU_PROBE_GL(_BELT)? iteration=(\d+) ok=(true|false)", logs)
        report["gles"] = {"available": "GPU_PROBE_GL_RESULT unavailable" not in logs,
                          "checks": len(gl), "passed": sum(ok == "true" for _m, _i, ok in gl)}
        if not passed:
            report["error"] = "Independent Vulkan buffer roundtrip failed or did not complete; inspect probe.log"
        report["limitations"] = ["No Taim renderer/view/window code participates in this probe",
                                  "A failure isolates the stack below Taim; it does not identify one upstream component",
                                  "A pass does not certify surfaces, text, gestures or Android lifecycle"]
    except KeyboardInterrupt:
        report["status"] = "interrupted"
        report["error"] = "Interrupted by operator"
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        report["status"] = "failed"
        report["error"] = str(error)
    finally:
        if not logs and since:
            try:
                logs = call("logcat", "-d", "-T", since, "-s", "RustStdoutStderr")
            except (OSError, subprocess.SubprocessError):
                pass
        try:
            call("shell", "am", "force-stop", package)
        except (OSError, subprocess.SubprocessError):
            pass
        (args.output / "probe.log").write_text(logs + "\n")
        report["probe_log_sha256"] = hashlib.sha256((logs + "\n").encode()).hexdigest()
        report["finished_utc"] = datetime.now(timezone.utc).isoformat()
        (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"status": report["status"], "checks": len(report.get("copy_checks", [])),
                      "page_size": report.get("device", {}).get("page_size"), "output": str(args.output)}, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
