#!/usr/bin/env python3
"""Exercise the reference APK on an explicitly selected Android test device.

Installs/force-stops only the Taim counter. Requires Pillow for pixel assertions.
Coordinates refer to physical screenshot pixels, not logical Taim coordinates.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import threading
import time

from counter import APPS, PACKAGE, ROOT, verify_apk_package

COMPONENT = f"{PACKAGE}/{APPS['counter'][2]}"
PRESENT = "TAIM_ANDROID_PRESENT"
LAUNCH = ("shell", "am", "start", "-W", "-a", "android.intent.action.MAIN",
          "-c", "android.intent.category.LAUNCHER", "-f", "0x10200000", "-n", COMPONENT)


class Device:
    def __init__(self, adb: Path, serial: str, output: Path):
        self.command = [str(adb), "-s", serial]
        self.output = output
        self.pid = None
        # pid -> (process, lines, lock). Device ring buffers can be small and
        # busy (e.g. 256 KiB on ColorOS rotates within seconds), so `logcat -d`
        # loses the app's earliest lines. Follow each process from launch instead.
        self.streams = {}

    def call(self, *args, check=True, binary=False):
        result = subprocess.run(self.command + list(args), capture_output=True,
                                text=not binary, timeout=30, check=False)
        if check and result.returncode:
            raise RuntimeError(f"adb {' '.join(args)} failed: {result.stderr!r}")
        return result.stdout if binary else result.stdout.strip()

    def follow(self):
        """Start capturing the current process's log; call as soon as its pid is known."""
        if not self.pid or self.pid in self.streams:
            return
        process = subprocess.Popen(self.command + ["logcat", f"--pid={self.pid}", "-v", "threadtime"],
                                   stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                   text=True, errors="replace")
        lines, lock = [], threading.Lock()
        def pump():
            for line in process.stdout:
                with lock:
                    lines.append(line)
        threading.Thread(target=pump, daemon=True).start()
        self.streams[self.pid] = (process, lines, lock)

    def logs(self):
        if self.pid:
            self.follow()
            _, lines, lock = self.streams[self.pid]
            with lock:
                return "".join(lines)
        return self.call("logcat", "-d", "-t", "200", "-s", "RustStdoutStderr", "AndroidRuntime")

    def close(self):
        for process, _, _ in self.streams.values():
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()

    def same_process(self):
        current = self.call("shell", "pidof", PACKAGE, check=False)
        if current != self.pid:
            raise RuntimeError(f"App process changed or died: expected {self.pid}, found {current!r}")

    def wait_logs(self, predicate, description, timeout):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            logs = self.logs()
            if any(marker in logs for marker in ("Taim Android failed:", "panicked at", "FATAL EXCEPTION", "Fatal signal")):
                raise RuntimeError(f"Application failure while waiting for {description}; inspect app.log")
            if predicate(logs):
                self.same_process()
                return logs
            time.sleep(0.2)
        raise RuntimeError(f"Timed out waiting for {description}; inspect app.log")

    def screenshot(self, name):
        from PIL import Image
        raw = self.call("exec-out", "screencap", "-p", binary=True)
        image = Image.open(io.BytesIO(raw)).convert("RGB")
        (self.output / f"{name}.png").write_bytes(raw)
        return image


def counter_values(logs):
    return [int(match) for match in re.findall(r"\bINFO android_counter: counter=(\d+)\b", logs)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial", required=True)
    parser.add_argument("--apk", type=Path, required=True)
    parser.add_argument("--tap", type=int, nargs=2, required=True, metavar=("X", "Y"))
    parser.add_argument("--counter-region", type=int, nargs=4, required=True,
                        metavar=("X", "Y", "WIDTH", "HEIGHT"))
    parser.add_argument("--cycles", type=int, default=3)
    parser.add_argument("--process-recreation", action="store_true",
                        help="Background, kill the app process, reopen its saved task and verify restored state")
    parser.add_argument("--timeout", type=float, default=25)
    parser.add_argument("--output", type=Path,
                        default=ROOT / "target/android/runtime-smoke" / datetime.now(timezone.utc).strftime("run-%Y%m%dT%H%M%S%fZ"))
    args = parser.parse_args()
    if args.cycles < 1 or args.timeout <= 0:
        parser.error("cycles and timeout must be positive")
    if not args.apk.is_file():
        parser.error("APK does not exist; run counter.py build first")
    sdk = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
    if not sdk:
        parser.error("Set ANDROID_HOME to the Android SDK directory")
    from PIL import ImageChops
    args.output.mkdir(parents=True, exist_ok=True)
    if any(args.output.iterdir()):
        parser.error("Choose a new empty output directory so evidence from different runs cannot mix")
    device = Device(Path(sdk) / "platform-tools/adb", args.serial, args.output)
    report = {"status": "running", "started_utc": datetime.now(timezone.utc).isoformat(),
              "serial": args.serial, "apk_sha256": hashlib.sha256(args.apk.read_bytes()).hexdigest(),
              "tap": args.tap, "counter_region": args.counter_region, "checks": []}
    stage = "device metadata"
    try:
        verify_apk_package(Path(sdk), args.apk, PACKAGE)
        if device.call("get-state") != "device":
            raise RuntimeError("Selected device is not ready")
        if device.call("shell", "getprop", "sys.boot_completed") != "1":
            raise RuntimeError("Selected device has not completed boot")
        report["device"] = {name: device.call("shell", "getprop", prop) for name, prop in
                            [("model", "ro.product.model"), ("sdk", "ro.build.version.sdk"),
                             ("abi", "ro.product.cpu.abi"), ("fingerprint", "ro.build.fingerprint")]}
        report["device"]["page_size"] = int(device.call("shell", "getconf", "PAGE_SIZE"))
        vulkan = device.call("shell", "cmd", "gpu", "vkjson", check=False)
        (args.output / "vulkan.json").write_text(vulkan + "\n")
        report["source"] = {
            "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
            "working_tree_dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT)),
            "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        }
        stage = "install and launch"
        print(stage, flush=True)
        device.call("install", "--no-incremental", "-r", str(args.apk.resolve()))
        device.call("shell", "am", "force-stop", PACKAGE)
        report["launch"] = device.call(*LAUNCH)
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline and not device.pid:
            device.pid = device.call("shell", "pidof", PACKAGE, check=False) or None
            if not device.pid:
                time.sleep(0.1)
        if not device.pid:
            raise RuntimeError("Activity launch returned but the application exited")
        device.follow()
        report["pid"] = device.pid
        stage = "first successful presentation"
        device.wait_logs(lambda log: PRESENT in log, stage, args.timeout)
        image = device.screenshot("initial")
        x, y, width, height = args.counter_region
        if x < 0 or y < 0 or width <= 0 or height <= 0 or x + width > image.width or y + height > image.height:
            raise RuntimeError("Counter region lies outside the screenshot")
        if not (0 <= args.tap[0] < image.width and 0 <= args.tap[1] < image.height):
            raise RuntimeError("Tap lies outside the screenshot")
        box = (x, y, x + width, y + height)
        def settled_crop(name, accept):
            # A successful present can precede Android's window transition.
            # Require actual, stable count pixels without disabling animations.
            deadline = time.monotonic() + args.timeout
            candidate = None
            stable_since = time.monotonic()
            while time.monotonic() < deadline:
                device.same_process()
                current = device.screenshot(name).crop(box)
                extrema = current.getextrema()
                nonblank = max(high - low for low, high in extrema) > 32
                if candidate is None or ImageChops.difference(candidate, current).getbbox() is not None:
                    candidate = current
                    stable_since = time.monotonic()
                elif nonblank and accept(current) and time.monotonic() - stable_since >= 0.3:
                    return current
                time.sleep(0.15)
            raise RuntimeError(f"Counter pixels did not settle to the expected content: {name}")

        previous = settled_crop("initial", lambda image: True)
        report["checks"].append("first successful presentation and stable nonblank counter screenshot")
        count = 0

        def increment(label):
            nonlocal count, previous
            stage_name = f"touch increment {count + 1}"
            print(stage_name, flush=True)
            device.call("shell", "input", "tap", str(args.tap[0]), str(args.tap[1]))
            count += 1
            logs = device.wait_logs(lambda log: counter_values(log) == list(range(1, count + 1)),
                                    stage_name, args.timeout)
            current = settled_crop(label, lambda image: ImageChops.difference(previous, image).getbbox() is not None)
            previous = current
            report["checks"].append(stage_name + " and counter-region pixel change")
            return logs

        stage = "initial touch and reactive repaint"
        increment("increment-1")
        for cycle in range(1, args.cycles + 1):
            stage = f"background/resume cycle {cycle}"
            print(stage, flush=True)
            baseline = device.logs()
            released = baseline.count("Android presentation surface released")
            recreated = baseline.count("Android presentation surface recreated")
            presented = baseline.count(PRESENT)
            device.call("shell", "input", "keyevent", "KEYCODE_HOME")
            device.wait_logs(lambda log: log.count("Android presentation surface released") > released,
                             "native surface release", args.timeout)
            device.call(*LAUNCH)
            logs = device.wait_logs(lambda log: log.count("Android presentation surface recreated") > recreated
                                    and log.count(PRESENT) > presented, "surface recreation and presentation", args.timeout)
            if logs.count("Taim Android counter starting") != 1:
                raise RuntimeError("Application entry point ran again during surface recreation")
            settled_crop(f"resume-{cycle}", lambda image: ImageChops.difference(previous, image).getbbox() is None)
            report["checks"].append(stage + ": same process, retained pixels, recreated target presented")
            increment(f"increment-{count + 1}")
        device.same_process()
        if args.process_recreation:
            stage = "saved-task process recreation"
            print(stage, flush=True)
            old_pid = device.pid
            (args.output / "before-process-kill.log").write_text(device.logs())
            released = device.logs().count("Android presentation surface released")
            device.call("shell", "input", "keyevent", "KEYCODE_HOME")
            device.wait_logs(lambda log: log.count("Android presentation surface released") > released,
                             "background checkpoint", args.timeout)
            # am kill preserves the saved task; force-stop would test a different contract.
            device.call("shell", "am", "kill", PACKAGE)
            deadline = time.monotonic() + args.timeout
            while device.call("shell", "pidof", PACKAGE, check=False):
                if time.monotonic() >= deadline:
                    raise RuntimeError("Background process did not terminate")
                time.sleep(0.2)
            device.call(*LAUNCH)
            device.pid = device.call("shell", "pidof", PACKAGE, check=False)
            if not device.pid or device.pid == old_pid:
                raise RuntimeError("Recreation did not start a distinct app process")
            device.follow()
            device.wait_logs(lambda log: PRESENT in log, "restored presentation", args.timeout)
            settled_crop("process-restored", lambda image: ImageChops.difference(previous, image).getbbox() is None)
            device.call("shell", "input", "tap", str(args.tap[0]), str(args.tap[1]))
            count += 1
            device.wait_logs(lambda log: counter_values(log) == [count], "increment after process restoration", args.timeout)
            settled_crop("process-restored-increment", lambda image: ImageChops.difference(previous, image).getbbox() is not None)
            report["recreated_pid"] = device.pid
            report["checks"].append("saved task restored count pixels into a new process and next increment retained the value")
        report["final_counter"] = count
        report["status"] = "passed"
        report["limitations"] = ["Emulator results do not certify physical GPU/device support",
                                  "Presentation marker means successful compositor present, not scan-out latency",
                                  "Pixel assertions depend on the supplied counter-only region",
                                  "No IME, TalkBack, battery or Flutter comparison acceptance"]
    except KeyboardInterrupt:
        report["status"] = "interrupted"
        report["failed_stage"] = stage
        report["error"] = "Interrupted by operator"
    except (RuntimeError, OSError, ValueError, subprocess.SubprocessError) as error:
        report["status"] = "failed"
        report["failed_stage"] = stage
        report["error"] = str(error)
    finally:
        try:
            (args.output / "app.log").write_text(device.logs())
        except (RuntimeError, OSError, subprocess.SubprocessError) as error:
            report["log_error"] = str(error)
        device.close()
        report["finished_utc"] = datetime.now(timezone.utc).isoformat()
        report["artifacts_sha256"] = {
            path.name: hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sorted(args.output.iterdir())
            if path.suffix in (".png", ".log") or path.name == "vulkan.json"
        }
        (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    summary = {key: report[key] for key in ("status", "checks", "error") if key in report}
    summary["output"] = str(args.output)
    print(json.dumps(summary, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
