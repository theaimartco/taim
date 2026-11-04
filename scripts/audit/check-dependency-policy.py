#!/usr/bin/env python3
"""Bound the temporary paste maintenance exception; never certify a release."""
from __future__ import annotations

from datetime import date
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[2]
REVIEW_BY = date(2027, 1, 26)
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
CHECKSUM = "57c0d7b74b563b49d38dae00a0c37d4d6de9b432382b2892f0574ddcae73fd0a"
PARENTS = {("pulp", "0.22.3"), ("rav1e", "0.8.1")}


def validate(metadata: dict, lock: dict, today: date) -> list[str]:
    failures = []
    if today >= REVIEW_BY:
        failures.append("paste maintenance exception has expired; remove or re-review it")
    packages = metadata.get("packages", [])
    copies = [p for p in packages if p["name"] == "paste"]
    if not copies:
        return failures
    if len(copies) != 1:
        return failures + ["unexpected multiple paste packages"]
    package = copies[0]
    if package["version"] != "1.0.15" or package.get("source") != REGISTRY:
        failures.append("paste source/version differs from reviewed registry release")
    targets = package.get("targets", [])
    runtime_kinds = {"lib", "rlib", "dylib", "cdylib", "staticlib"}
    if not any(t["kind"] == ["proc-macro"] for t in targets) or any(runtime_kinds.intersection(t["kind"]) for t in targets):
        failures.append("paste must remain a build-time proc-macro without a runtime library")
    locked = [p for p in lock.get("package", []) if p["name"] == "paste"]
    if len(locked) != 1 or any(p.get("version") != "1.0.15" or p.get("source") != REGISTRY or p.get("checksum") != CHECKSUM for p in locked):
        failures.append("paste lockfile checksum/source/version differs from reviewed artifact")
    by_id = {p["id"]: p for p in packages}
    nodes = metadata.get("resolve", {}).get("nodes", [])
    parents = [by_id[n["id"]] for n in nodes if any(d["pkg"] == package["id"] for d in n.get("deps", []))]
    if not parents:
        failures.append("missing resolved paste dependency edges")
    for parent in parents:
        if (parent["name"], parent["version"]) not in PARENTS or parent.get("source") != REGISTRY:
            failures.append(f"unreviewed paste consumer: {parent['name']} {parent['version']}")
    return failures


def validate_advisory(advisory: dict) -> list[str]:
    if (advisory.get("id") != "RUSTSEC-2024-0436"
        or advisory.get("package") != "paste"
        or advisory.get("informational") != "unmaintained"
        or advisory.get("cvss") is not None):
        return ["paste exception only covers the reviewed informational maintenance advisory"]
    return []


def advisory_failures() -> list[str]:
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    paths = list((cargo_home / "advisory-dbs").glob("*/crates/paste/RUSTSEC-2024-0436.md"))
    if not paths:
        return ["missing RustSec advisory evidence; refresh with cargo deny first"]
    failures = []
    for path in paths:
        text = path.read_text()
        header = text.split("```toml\n", 1)[1].split("\n```", 1)[0]
        failures.extend(validate_advisory(tomllib.loads(header).get("advisory", {})))
    return failures


def main() -> int:
    try:
        metadata = json.loads(subprocess.check_output([
            "cargo", "metadata", "--all-features", "--locked", "--offline", "--format-version", "1",
        ], cwd=ROOT, text=True))
        lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
        failures = validate(metadata, lock, date.today())
        if any(p["name"] == "paste" for p in metadata.get("packages", [])):
            failures.extend(advisory_failures())
    except (OSError, ValueError, KeyError, IndexError, subprocess.CalledProcessError) as error:
        print(f"FAIL: dependency-policy evidence unavailable: {error}", file=sys.stderr)
        return 1
    if failures:
        for failure in failures:
            print("FAIL: " + failure, file=sys.stderr)
        return 1
    print(f"PASS: temporary paste maintenance policy bounded until {REVIEW_BY}")
    print("This is documented risk acceptance, not replacement of the unmaintained crate.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
