#!/usr/bin/env python3
"""Disallow new active Iced runtime dependencies without erasing source attribution.

This guard checks Cargo manifests and the resolved lockfile. It is NOT a source
independence report: inherited Rust code still requires substantive rewriting
and source-lineage disclosure.
"""

import argparse
from pathlib import Path
import sys
import tomllib

FORBIDDEN_SOURCE = ("github.com/iced-rs/", "github.com/iced-rs%2f")


def iced_package(name):
    return name == "iced" or name.startswith(("iced_", "iced-"))


def nested_dependency_sections(tree):
    if not isinstance(tree, dict):
        return
    for key, value in tree.items():
        if key in ("dependencies", "dev-dependencies", "build-dependencies") and isinstance(value, dict):
            yield key, value
        elif key == "target" and isinstance(value, dict):
            for target, table in value.items():
                for nested_key, entries in nested_dependency_sections(table):
                    yield f"target.{target}.{nested_key}", entries


def check_manifest(path):
    doc = tomllib.loads(path.read_text(encoding="utf-8"))
    issues = []
    for section, dependencies in nested_dependency_sections(doc):
        for alias, spec in dependencies.items():
            actual_name = spec.get("package", alias) if isinstance(spec, dict) else alias
            if iced_package(actual_name):
                issues.append(f"{path}: {section} uses Iced crate {alias}/{actual_name}")
            if isinstance(spec, dict) and any(
                token in str(spec.get("git", "")).lower() for token in FORBIDDEN_SOURCE
            ):
                issues.append(f"{path}: {section} imports iced-rs Git source")
    return issues


def check_lock(path):
    doc = tomllib.loads(path.read_text(encoding="utf-8"))
    issues = []
    for entry in doc.get("package", []):
        name = entry.get("name", "")
        source = str(entry.get("source", "")).lower()
        if iced_package(name):
            issues.append(f"{path}: resolved Iced crate {name}")
        if any(token in source for token in FORBIDDEN_SOURCE):
            issues.append(f"{path}: resolved iced-rs Git source {name}")
    return issues


def check_repository(root):
    root = Path(root).resolve()
    if not (root / "Cargo.lock").is_file():
        raise ValueError("Cargo.lock is missing")
    issues = check_lock(root / "Cargo.lock")
    if not (root / "Cargo.toml").is_file():
        raise ValueError("root Cargo.toml is missing")
    # Analyze all tracked manifests, never recurse through transient targets.
    import subprocess

    manifest_names = subprocess.check_output(
        ["git", "-C", str(root), "ls-files", "--", "Cargo.toml", "*/Cargo.toml", "**/Cargo.toml"],
        text=True,
    ).splitlines()
    if "Cargo.toml" not in manifest_names:
        raise ValueError("root manifest not tracked")
    for rel in sorted(set(manifest_names)):
        path = root / rel
        if path.is_file():
            issues.extend(check_manifest(path))
    return issues


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    try:
        issues = check_repository(args.root)
    except (ValueError, OSError, tomllib.TOMLDecodeError) as error:
        print(f"NO-GO: {error}", file=sys.stderr)
        return 1
    except Exception as error:
        print(f"NO-GO: cannot inspect git tracked manifests: {error}", file=sys.stderr)
        return 1
    if issues:
        print("\n".join(issues), file=sys.stderr)
        return 1
    print("PASS: no resolved or declared Iced Cargo dependency detected")
    print("NOT A SOURCE-OWNERSHIP RESULT: inherited origin remains and must be attributed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
