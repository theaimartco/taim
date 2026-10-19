#!/usr/bin/env python3
"""Strict heuristic audit of active source versus the recorded Iced fork point.

Do not equate a zero similarity count with legal proof of original authorship.
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys

BASELINE = "56b706fc"
COUNTS = ("byte_identical_files", "token_identical_files",
          "high_similarity_files", "disguised_copies", "inherited_loc_token")


def assess(data):
    summary = data.get("summary")
    files = data.get("files")
    if not isinstance(summary, dict) or not isinstance(files, list):
        raise ValueError("missing summary or per-file evidence")
    if summary.get("total_files") != len(files) or not files:
        raise ValueError("incomplete file inventory")
    if type(summary.get("total_loc")) is not int or summary["total_loc"] <= 0:
        raise ValueError("invalid source line inventory")
    blockers = []
    for field in COUNTS:
        n = summary.get(field)
        if type(n) is not int or n < 0:
            raise ValueError(f"missing or invalid metric: {field}")
        if n:
            blockers.append(f"{field}={n}")
    return {
        "schema_version": 1,
        "status": "NO_GO" if blockers else "AUTOMATED_SIMILARITY_CLEAR_REVIEW_REQUIRED",
        "blockers": blockers,
        "summary": summary,
        "manual_review_required": True,
        "baseline": BASELINE,
        "scope": "heuristic source similarity, not legal ownership",
    }


def run_audit(root):
    root = Path(root).resolve()
    for name in ("LICENSE", "CREDITS.md", "scripts/audit/check-independence.py"):
        if not (root / name).is_file():
            raise ValueError("required attribution or scanner missing: " + name)
    subprocess.run(["git", "-C", str(root), "cat-file", "-e", BASELINE + "^{commit}"],
                   check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    ancestor = subprocess.run(["git", "-C", str(root),
                              "merge-base", "--is-ancestor", BASELINE, "HEAD"])
    if ancestor.returncode:
        raise ValueError("Iced fork point not in current Git ancestry")
    process = subprocess.run(
        [sys.executable, str(root / "scripts/audit/check-independence.py"), "--json"],
        cwd=root, capture_output=True, text=True, check=True)
    result = assess(json.loads(process.stdout))
    result["head_sha"] = subprocess.check_output(
        ["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--strict", action="store_true",
                        help="fail release promotion unless all automated inherited-code counts are zero")
    args = parser.parse_args()
    try:
        verdict = run_audit(args.root)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(verdict, indent=2) + "\n")
    except (ValueError, OSError, subprocess.CalledProcessError,
            json.JSONDecodeError) as error:
        print(f"NO-GO: source audit failed: {error}", file=sys.stderr)
        return 2
    print("TAIM source-lineage audit:", verdict["status"])
    for finding in verdict["blockers"]:
        print(" -", finding)
    print("Manual code-origin review and Iced MIT attribution remain mandatory.")
    return int(args.strict and bool(verdict["blockers"]))


if __name__ == "__main__":
    raise SystemExit(main())
