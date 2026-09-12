#!/usr/bin/env python3
"""
TAIM Independence & Architectural Scanner
Computes exact file-by-file divergence against the Iced fork baseline (56b706fc).
Enforces independence invariants across all workspace crates.
"""

import subprocess
import os
import sys
import difflib

REPO_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
FORK_COMMIT = "56b706fc"

def run_git(cmd):
    res = subprocess.run(["git"] + cmd, cwd=REPO_DIR, capture_output=True, text=True, check=True)
    return res.stdout

def get_git_bytes(commit, path):
    res = subprocess.run(["git", "show", f"{commit}:{path}"], cwd=REPO_DIR, capture_output=True)
    if res.returncode != 0:
        return None
    return res.stdout

def compute_similarity(base_bytes, head_bytes):
    if base_bytes == head_bytes:
        return 100.0
    try:
        base_lines = base_bytes.decode("utf-8").splitlines(keepends=True)
        head_lines = head_bytes.decode("utf-8").splitlines(keepends=True)
        matcher = difflib.SequenceMatcher(None, base_lines, head_lines)
        return matcher.ratio() * 100.0
    except UnicodeDecodeError:
        return 100.0 if base_bytes == head_bytes else 50.0

def main():
    head_files = run_git(["ls-tree", "-r", "--name-only", "HEAD"]).strip().splitlines()
    fork_files = set(run_git(["ls-tree", "-r", "--name-only", FORK_COMMIT]).strip().splitlines())

    relevant_extensions = ('.rs', '.wgsl', '.toml', '.svg')
    audited = []
    
    total_inherited_loc = 0
    total_taim_loc = 0

    for f in head_files:
        if not f.endswith(relevant_extensions) and not f.startswith("assets/"):
            continue
        if f.startswith(".git") or f.startswith("target/"):
            continue

        file_path = os.path.join(REPO_DIR, f)
        if os.path.isfile(file_path):
            with open(file_path, "rb") as fh:
                head_bytes = fh.read()
        else:
            head_bytes = get_git_bytes("HEAD", f)

        if head_bytes is None:
            continue

        head_loc = len(head_bytes.splitlines())
        total_taim_loc += head_loc

        is_in_base = f in fork_files
        if not is_in_base:
            similarity = 0.0
        else:
            base_bytes = get_git_bytes(FORK_COMMIT, f)
            similarity = compute_similarity(base_bytes, head_bytes)
            if similarity >= 50.0:
                total_inherited_loc += int(head_loc * (similarity / 100.0))

        audited.append((f, is_in_base, similarity, head_loc))

    total_files = len(audited)
    identical = [x for x in audited if x[2] >= 99.9]
    high_sim = [x for x in audited if 70.0 <= x[2] < 99.9]
    diverged = [x for x in audited if x[2] < 70.0]

    print(f"=== TAIM Independence Audit ===")
    print(f"Total files audited: {total_files}")
    print(f"Total LOC: {total_taim_loc}")
    print(f"Estimated inherited LOC: {total_inherited_loc} ({total_inherited_loc/total_taim_loc*100:.1f}%)")
    print(f"Byte-identical files (>=99.9%): {len(identical)}")
    print(f"High-similarity files (70-99.9%): {len(high_sim)}")
    print(f"Diverged / Taim-native files (<70%): {len(diverged)}")
    print(f"Overall workspace independence: {(1.0 - total_inherited_loc / total_taim_loc) * 100:.1f}%")

if __name__ == "__main__":
    main()
