#!/usr/bin/env python3
"""Run TAIM's local, fail-closed code checks without enabling GitHub Actions.

Passing automated checks is NOT proof of independent authorship or a
production-release decision. A separate human review is always required.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

DEFAULT_ROOT = Path(__file__).resolve().parents[2]


def commands(python: str = sys.executable) -> list[tuple[str, list[str]]]:
    """Preserve command order: source evidence before expensive build tests."""
    checks = [
        ("source_lineage", [
            python, "scripts/audit/source-lineage-gate.py",
            "--strict", "--output", "artifacts/taim-local/source-lineage.json",
        ]),
        ("source_gate_tests", [
            python, "-m", "unittest", "discover", "-s", "scripts/audit",
            "-p", "test_*.py", "-v",
        ]),
        ("local_gate_tests", [
            python, "-m", "unittest", "discover", "-s", "scripts/qa",
            "-p", "test_*.py", "-v",
        ]),
        ("release_contract_tests", [
            python, "-m", "unittest", "discover", "-s", "scripts/release",
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
        ("public_api_docs", ["cargo", "rustdoc", "-p", "taim", "--lib", "--all-features", "--locked", "--", "-D", "warnings"]),
        ("workspace_tests", [
            "cargo", "test", "--workspace", "--all-features", "--locked",
        ]),
        ("production_tests", ["cargo", "test", "--workspace", "--locked"]),
        ("cpu_golden_tests", ["cargo", "test", "--test", "golden", "--locked"]),
        ("frame_time_contract", ["cargo", "test", "-p", "taim_bench", "frame_time", "--locked"]),
        ("license_and_advisory_check", ["cargo", "deny", "check", "licenses", "advisories"]),
        ("dependency_maintenance_policy", [python, "scripts/audit/check-dependency-policy.py"]),
        ("external_consumer", ["bash", "scripts/release/external-consumer-smoke.sh"]),
        ("source_bundle", [python, "scripts/release/build-source-bundle.py"]),
        ("dependency_sbom", [python, "scripts/release/sbom.py", "--all-features",
                             "--output", "artifacts/taim-local/cargo.spdx.json"]),
        ("package_inventory", [
            "cargo", "package", "-p", "taim", "--list", "--allow-dirty",
        ]),
    ]
    if sys.platform.startswith("linux"):
        checks.extend([
            ("atspi_private_bus", [
                "dbus-run-session", "--", "cargo", "test", "-p", "taim-accessibility",
                "--features", "atspi", "--test", "atspi_bus", "--locked", "--", "--ignored",
            ]),
            ("desktop_theme_private_bus", [
                "dbus-run-session", "--", "cargo", "test", "-p", "taim_winit",
                "--features", "linux-theme-detection", "--lib",
                "system_theme::tests::portal_initial_read_and_racing_changes_are_delivered_in_order",
                "--locked", "--", "--ignored",
            ]),
        ])
    return checks



def classify(results: list[dict], all_tools_present: bool) -> str:
    """Never represent missing tests or skipped tests as production success."""
    if not all_tools_present or not results or any(x["status"] != "PASS" for x in results):
        return "NO_GO"
    expected = [name for name, _ in commands()]
    if [result.get("name") for result in results] != expected:
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
    checkout_errors = []
    try:
        sha = subprocess.check_output(
            ["git", "rev-parse", "--verify", "HEAD"], cwd=root, text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        dirty = subprocess.check_output(
            ["git", "status", "--porcelain", "--untracked-files=no"],
            cwd=root, text=True, stderr=subprocess.DEVNULL,
        )
        if dirty:
            checkout_errors.append("modified tracked files")
        if len(sha) != 40 or any(c not in "0123456789abcdef" for c in sha.lower()):
            checkout_errors.append("invalid full commit SHA")
    except (OSError, subprocess.CalledProcessError):
        checkout_errors.append("missing full Git checkout")

    steps = commands()
    needed = sorted({cmd[0] for _, cmd in steps})
    missing = [exe for exe in needed if shutil.which(exe) is None]
    results: list[dict] = []
    if checkout_errors:
        results.append({
            "name": "checkout_preflight", "status": "FAIL",
            "returncode": None, "command": ["git", "rev-parse", "HEAD"],
            "errors": checkout_errors,
        })
    for name, cmd in ([] if checkout_errors else steps):
        path = evidence / (name + ".log")
        if shutil.which(cmd[0]) is None:
            path.write_text("Missing executable: " + cmd[0] + "\n", encoding="utf-8")
            results.append({
                "name": name, "status": "MISSING_TOOL", "returncode": None,
                "command": cmd, "log": str(path),
            })
            print(f"{name}: MISSING_TOOL", flush=True)
            if args.stop_on_failure:
                break
            continue
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
            "log_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        })
        print(f"{name}: {status}", flush=True)
        if status != "PASS" and args.stop_on_failure:
            break

    status = classify(results, not missing and not checkout_errors)
    summary = {
        "schema_version": 1,
        "head_sha": sha,
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "status": status,
        "missing_tools": missing,
        "checkout_errors": checkout_errors,
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
