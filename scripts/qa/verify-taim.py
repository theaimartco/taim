#!/usr/bin/env python3
"""Run TAIM's local, fail-closed code checks without enabling GitHub Actions.

Passing automated checks is NOT proof of independent authorship or a
production-release decision. A separate human review is always required.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

DEFAULT_ROOT = Path(__file__).resolve().parents[2]


def commands(python: str = sys.executable) -> list[tuple[str, list[str]]]:
    """Preserve command order: source evidence before expensive build tests."""
    return [
        ("source_lineage", [
            python, "scripts/audit/source-lineage-gate.py",
            "--strict", "--output", "artifacts/taim-local/source-lineage.json",
        ]),
        ("source_gate_tests", [
            python, "-m", "unittest", "discover", "-s", "scripts/audit",
            "-p", "test_*.py", "-v",
        ]),
        ("rustfmt", ["cargo", "fmt", "--all", "--", "--check"]),
        ("workspace_check", [
            "cargo", "check", "--workspace", "--all-targets",
            "--all-features", "--locked",
        ]),
        ("clippy", [
            "cargo", "clippy", "--workspace", "--all-targets",
            "--all-features", "--locked", "--", "-D", "warnings",
        ]),
        ("workspace_tests", [
            "cargo", "test", "--workspace", "--all-features", "--locked",
        ]),
        ("package_inventory", [
            "cargo", "package", "-p", "taim", "--list", "--allow-dirty",
        ]),
    ]


def classify(results: list[dict], all_tools_present: bool) -> str:
    """Never represent missing tests or skipped tests as production success."""
    if not all_tools_present or not results or any(x["status"] != "PASS" for x in results):
        return "NO_GO"
    if len(results) != len(commands()):
        return "NO_GO"
    return "CODE_CHECKS_PASSED_HUMAN_ORIGIN_AND_DEVICE_REVIEW_REQUIRED"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    parser.add_argument(
        "--evidence-dir", type=Path, default=Path("artifacts/taim-local"),
        help="Log location (relative paths are resolved inside the checkout)",
    )
    parser.add_argument(
        "--stop-on-failure", action="store_true",
        help="Stop instead of collecting evidence from subsequent checks",
    )
    args = parser.parse_args(argv)
    root = args.root.resolve()
    evidence = args.evidence_dir
    if not evidence.is_absolute():
        evidence = root / evidence
    evidence.mkdir(parents=True, exist_ok=True)

    sha = "UNKNOWN"
    try:
        sha = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=root, text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
    except (OSError, subprocess.CalledProcessError):
        pass

    steps = commands()
    needed = sorted({cmd[0] for _, cmd in steps})
    missing = [exe for exe in needed if shutil.which(exe) is None]
    results: list[dict] = []
    if not missing:
        for name, cmd in steps:
            path = evidence / (name + ".log")
            try:
                with path.open("w", encoding="utf-8") as out:
                    run = subprocess.run(
                        cmd, cwd=root, stdout=out, stderr=subprocess.STDOUT,
                        check=False, timeout=3600,
                    )
                status = "PASS" if run.returncode == 0 else "FAIL"
                code = run.returncode
            except subprocess.TimeoutExpired:
                status, code = "TIMEOUT", None
            except OSError as exc:
                status, code = "ERROR", None
                path.write_text(str(exc) + "\n", encoding="utf-8")
            results.append({
                "name": name, "status": status, "returncode": code,
                "command": cmd, "log": str(path),
            })
            print(f"{name}: {status}", flush=True)
            if status != "PASS" and args.stop_on_failure:
                break

    status = classify(results, not missing)
    summary = {
        "schema_version": 1,
        "head_sha": sha,
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "status": status,
        "missing_tools": missing,
        "checks": results,
        "source_authorship_verified": False,
        "production_release_certified": False,
        "required_human_checks": [
            "manual original-source and license provenance review",
            "Naga/WGSL compilation and screenshot visual equivalence",
            "real VYOM/Wayland device, IME, accessibility, and GPU recovery",
            "same-device performance and power measurements",
            "signed package provenance and explicit release approval",
        ],
    }
    path = evidence / "verification-summary.json"
    path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(f"TAIM verification: {status}")
    print(f"Evidence: {path}")
    if missing:
        print("Missing tools: " + ", ".join(missing))
    print("Automated evidence cannot establish 100% independent authorship.")
    return 0 if status.startswith("CODE_CHECKS_PASSED_") else 2


if __name__ == "__main__":
    raise SystemExit(main())
