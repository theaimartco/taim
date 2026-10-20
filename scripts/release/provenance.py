#!/usr/bin/env python3
"""Record candidate artifact provenance; NOT signing, publication, or reproducibility proof."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def output(*args):
    return subprocess.check_output(args, text=True).strip()


def digest(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def manifest(root, artifacts, allow_dirty=False):
    root = Path(root).resolve()
    status = output("git", "-C", str(root), "status", "--porcelain", "--untracked-files=normal")
    if status and not allow_dirty:
        raise ValueError("release candidate checkout is dirty; refuse provenance manifest")
    head = output("git", "-C", str(root), "rev-parse", "HEAD")
    lock = root / "Cargo.lock"
    if not lock.is_file():
        raise ValueError("Cargo.lock not found")
    entries = []
    for name in artifacts:
        artifact = Path(name).resolve()
        if not artifact.is_file():
            raise ValueError(f"artifact missing: {artifact}")
        entries.append({
            "path": str(artifact),
            "size_bytes": artifact.stat().st_size,
            "sha256": digest(artifact),
        })
    return {
        "schema_version": 1,
        "source_commit": head,
        "dirty_tree": bool(status),
        "rustc_version": output("rustc", "--version"),
        "cargo_version": output("cargo", "--version"),
        "cargo_lock_sha256": digest(lock),
        "artifacts": entries,
        "sbom_verified": False,
        "independent_rebuild_match": False,
        "signature_verified": False,
        "hardware_acceptance_verified": False,
        "release_approved": False,
        "notice": "Inventory only: requires independent clean rebuild, SBOM, signer verification and owner approval.",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--allow-dirty", action="store_true",
                        help="diagnostics only; never use dirty provenance for a release")
    parser.add_argument("artifacts", nargs="+", type=Path)
    args = parser.parse_args()
    try:
        data = manifest(args.root, args.artifacts, allow_dirty=args.allow_dirty)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")
    except (OSError, subprocess.CalledProcessError, ValueError) as error:
        print(f"candidate rejected: {error}", file=sys.stderr)
        return 1
    print(f"Candidate inventory recorded at {args.output}; release NOT approved")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
