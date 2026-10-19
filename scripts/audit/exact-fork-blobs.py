#!/usr/bin/env python3
"""Compare executable source blobs with every upstream Iced blob, across paths.

This is an exact SHA equality gate; partial copies still require deeper
heuristic and human review. It deliberately never rewrites the origin history.
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys

BASELINE = "56b706fc"
SOURCE_SUFFIXES = (".rs", ".wgsl", ".toml")

def parse_tree(content):
    result = {}
    for record in content.split(b"\0"):
        if not record:
            continue
        try:
            metadata, path = record.split(b"\t", 1)
            mode, kind, sha = metadata.split(b" ")
        except ValueError as err:
            raise ValueError("malformed NUL-separated git tree record") from err
        if kind != b"blob":
            continue
        result[path.decode("utf-8", "surrogateescape")] = sha.decode("ascii")
    return result

def exact_matches(upstream, current):
    blobs = {}
    for path, sha in upstream.items():
        blobs.setdefault(sha, []).append(path)
    matches = []
    for path, sha in sorted(current.items()):
        if not path.endswith(SOURCE_SUFFIXES):
            continue
        if sha in blobs:
            matches.append({
                "path": path,
                "upstream_paths": sorted(blobs[sha]),
                "identity": "same_path" if path in blobs[sha] else "relocated_exact_copy",
                "sha": sha,
            })
    return matches

def git_tree(root, commit):
    content = subprocess.check_output(
        ["git", "-C", str(root), "ls-tree", "-r", "-z", commit]
    )
    return parse_tree(content)

def audit(root, baseline=BASELINE):
    root = Path(root).resolve()
    provenance = {
        "baseline": baseline,
        "head": subprocess.check_output(
            ["git", "-C", str(root), "rev-parse", "HEAD"], text=True
        ).strip(),
    }
    match = exact_matches(git_tree(root, baseline), git_tree(root, "HEAD"))
    provenance.update({
        "exact_match_count": len(match),
        "same_path_count": sum(v["identity"] == "same_path" for v in match),
        "relocated_exact_copy_count": sum(
            v["identity"] == "relocated_exact_copy" for v in match
        ),
        "matches": match,
        "status": "NO_GO" if match else "EXACT_BLOBS_CLEAR_MANUAL_REVIEW_REQUIRED",
        "warning": "Zero exact SHA matches does not prove independent authorship.",
    })
    return provenance

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--strict", action="store_true")
    args = parser.parse_args()
    try:
        report = audit(args.root)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print("NO-GO: cannot verify source blobs:", error, file=sys.stderr)
        return 2
    print(f"Exact inherited blobs: {report['exact_match_count']}")
    print("Zero cannot prove absence of modified or moved inherited code.")
    return int(args.strict and bool(report["matches"]))

if __name__ == "__main__":
    raise SystemExit(main())
