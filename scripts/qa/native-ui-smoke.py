#!/usr/bin/env python3
"""Exercise the reference counter's real window/AT-SPI action path in private services."""
from __future__ import annotations

import argparse
import ast
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
ACCESSIBLE = "org.a11y.atspi.Accessible"
ROOT_PATH = "/org/a11y/atspi/accessible/root"


def strings(reply: str) -> list[str]:
    return [ast.literal_eval(match.group()) for match in re.finditer(r"'(?:[^'\\]|\\.)*'|\"(?:[^\"\\]|\\.)*\"", reply)]


def integer(reply: str) -> int:
    match = re.search(r"uint32\s+(\d+)", reply) or re.search(r"\(\s*(\d+)", reply)
    if not match:
        raise ValueError(f"not an integer reply: {reply}")
    return int(match.group(1))


def rpc(address: str | None, destination: str, path: str, method: str, *args: str) -> str:
    bus = ["--session"] if address is None else ["--address", address]
    result = subprocess.run(["gdbus", "call", *bus, "--dest", destination,
                             "--object-path", path, "--method", method, *args],
                            capture_output=True, text=True, timeout=3, check=False)
    if result.returncode:
        raise RuntimeError(result.stderr.strip())
    return result.stdout.strip()


def eventually(probe, timeout: float = 8):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            value = probe()
            if value is not None:
                return value
        except (RuntimeError, ValueError, subprocess.TimeoutExpired) as error:
            last = error
        time.sleep(0.04)
    raise RuntimeError(f"native UI probe timed out: {last}")


def inside(app: Path, output: Path) -> None:
    runtime = Path(os.environ.get("XDG_RUNTIME_DIR", ""))
    bus = os.environ.get("DBUS_SESSION_BUS_ADDRESS", "")
    if (not runtime.is_relative_to(Path(tempfile.gettempdir()))
            or not bus.startswith(("unix:path=/tmp/", "unix:abstract=/tmp/"))):
        raise RuntimeError("refuse to probe outside the private runtime/session")
    address = strings(rpc(None, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus.GetAddress"))[0]
    marker = output / "first-present"
    env = dict(os.environ, TAIM_BACKEND="tiny-skia", TAIM_FRAME_TRACE="1", TAIM_FIRST_PRESENT_MARKER=str(marker))
    with (output / "window.log").open("w") as log:
        process = subprocess.Popen([str(app)], env=env, stdout=log, stderr=subprocess.STDOUT)
        try:
            def owner():
                if process.poll() is not None:
                    raise RuntimeError(f"reference app exited: {process.returncode}")
                names = strings(rpc(address, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus.ListNames"))
                for name in names:
                    if not name.startswith(":"):
                        continue
                    try:
                        pid = integer(rpc(address, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus.GetConnectionUnixProcessID", repr(name)))
                    except RuntimeError:
                        continue  # a short-lived probe connection already disappeared
                    if pid == process.pid:
                        return name
                return None
            destination = eventually(owner)

            def name(path):
                return strings(rpc(address, destination, path, "org.freedesktop.DBus.Properties.Get", repr(ACCESSIBLE), repr("Name")))[0]

            def discover():
                pending, visited, buttons, count = [ROOT_PATH], set(), {}, None
                while pending:
                    path = pending.pop()
                    if path in visited or path.endswith("/null"):
                        continue
                    visited.add(path)
                    if len(visited) > 256:
                        raise RuntimeError("reference semantic tree exceeded its smoke-test budget")
                    label = name(path)
                    if label == "Count: 0":
                        count = path
                    role = integer(rpc(address, destination, path, ACCESSIBLE + ".GetRole"))
                    if role == 43:  # AT-SPI PUSH_BUTTON, same ABI as vocabulary.rs
                        buttons[label] = path
                    children = strings(rpc(address, destination, path, ACCESSIBLE + ".GetChildren"))
                    pending.extend(value for value in children if value.startswith("/org/a11y/atspi/"))
                return (buttons, count) if "+1" in buttons and "Reset" in buttons and count else None
            buttons, count = eventually(discover)
            eventually(lambda: True if marker.is_file() else None)
            for button, expected in [("+1", "Count: 1"), ("Reset", "Count: 0")]:
                def activate():
                    # An enabled interface may still be being registered by the bus worker.
                    result = rpc(address, destination, buttons[button], "org.a11y.atspi.Action.DoAction", "0")
                    if result != "(true,)":
                        raise AssertionError(f"{button}: native action rejected: {result}")
                    return True
                eventually(activate)
                actual = name(count)
                if actual != expected:
                    raise AssertionError(f"action completion preceded UI publication: expected {expected}, got {actual}")
            report = {"status": "PASS", "backend": "tiny-skia", "display": "private Xvfb",
                      "native_present": True, "actions": ["+1", "Reset"],
                      "count_sequence": [0, 1, 0], "read_after_action_publication": True,
                      "physical_device_acceptance": False}
            (output / "result.json").write_text(json.dumps(report, indent=2) + "\n")
            print(f"PASS: native window + AT-SPI actions + UI publication; evidence: {output}")
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, default=ROOT / "target/debug/counter")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--inside-private", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    try:
        for tool in ["dbus-run-session", "xvfb-run", "gdbus"]:
            if shutil.which(tool) is None:
                raise RuntimeError(f"missing required smoke-test tool: {tool}")
        app = args.app.resolve()
        if not app.is_file():
            raise RuntimeError("build counter with taim/measurement,taim/accessibility first")
        if args.inside_private:
            if args.output is None:
                raise RuntimeError("private child requires an evidence directory")
            inside(app, args.output.resolve())
            return 0
        output = (args.output or ROOT / "target/native-ui-smoke" / uuid.uuid4().hex).resolve()
        output.mkdir(parents=True, exist_ok=False)
        with tempfile.TemporaryDirectory(prefix="taim-private-ui-") as directory:
            runtime = Path(directory) / "runtime"
            runtime.mkdir(mode=0o700)
            env = dict(os.environ, XDG_RUNTIME_DIR=str(runtime))
            result = subprocess.run(["dbus-run-session", "--", "xvfb-run", "-a", sys.executable,
                                     str(Path(__file__).resolve()), "--inside-private", "--app", str(app),
                                     "--output", str(output)], env=env, timeout=40, check=False)
            return result.returncode
    except (OSError, ValueError, RuntimeError, AssertionError, subprocess.TimeoutExpired) as error:
        parser.exit(1, f"Native UI smoke failed: {error}\n")


if __name__ == "__main__":
    raise SystemExit(main())
