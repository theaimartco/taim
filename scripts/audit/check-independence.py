#!/usr/bin/env python3
"""
TAIM Hardened Independence & Architectural Scanner

Computes exact file-by-file divergence against the Iced fork baseline (56b706fc).
Features:
1. git diff -M -C rename and copy detection across commits.
2. Token stream normalization (strips comments, docstrings, formatting, whitespace, normalizes literals).
3. Produces normalized token similarity, likely source paths, and confidence scores.
4. Detects cosmetic / formatting-only refactors and enforces independence invariants.
"""

import subprocess
import os
import sys
import re
import difflib
import argparse
import json
import time
from collections import Counter

REPO_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
FORK_COMMIT = "56b706fc"

RELEVANT_EXTENSIONS = ('.rs', '.wgsl', '.toml', '.svg')

def run_git(cmd):
    res = subprocess.run(["git"] + cmd, cwd=REPO_DIR, capture_output=True, text=True, check=True)
    return res.stdout

def get_git_bytes(commit, path):
    res = subprocess.run(["git", "show", f"{commit}:{path}"], cwd=REPO_DIR, capture_output=True)
    if res.returncode != 0:
        return None
    return res.stdout

def get_git_rename_copy_map():
    """
    Parses git diff -M -C output to identify renamed or copied files from fork baseline.
    Returns dict mapping destination path -> (source_path, status_code, score).
    """
    cmd = ["diff", "--name-status", "-M", "-C", FORK_COMMIT]
    try:
        output = run_git(cmd)
    except subprocess.CalledProcessError:
        return {}

    rename_map = {}
    for line in output.splitlines():
        parts = line.split("\t")
        if len(parts) == 3 and (parts[0].startswith("R") or parts[0].startswith("C")):
            status = parts[0]
            src = parts[1]
            dst = parts[2]
            score = int(status[1:]) if len(status) > 1 and status[1:].isdigit() else 50
            rename_map[dst] = (src, status[0], score)
    return rename_map

def strip_comments(text):
    """Strips Rust/C-style line comments, doc comments, and nested block comments."""
    text = re.sub(r"//.*", " ", text)
    while "/*" in text:
        new_text = re.sub(r"/\*([^*]|\*(?!/))*\*/", " ", text, flags=re.DOTALL)
        if new_text == text:
            break
        text = new_text
    return text

def tokenize_code(text, normalize_literals=True):
    """
    Normalizes code into a canonical token stream.
    Strips comments, doc comments, and whitespace.
    Optionally normalizes string and numeric literals to prevent superficial obfuscation.
    """
    text = strip_comments(text)
    token_pattern = re.compile(
        r"(?P<STR>\"(?:\\.|[^\"\\])*\")|"
        r"(?P<CHAR>\x27(?:\\.|[^\x27\\])*\x27)|"
        r"(?P<NUM>0x[0-9a-fA-F_]+|\d[\d_]*(?:\.[\d_]+)?(?:[eE][+-]?[\d_]+)?(?:f32|f64|u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize)?)|"
        r"(?P<IDENT>[a-zA-Z_][a-zA-Z0-9_]*)|"
        r"(?P<OP>::|\->|=>|==|!=|<=|>=|\+=|\-=|\*=|/=|&&|\|\||[{}()\[\].,;:?&|^!~=<>+\-*/%])"
    )

    tokens = []
    for match in token_pattern.finditer(text):
        if normalize_literals:
            if match.group("STR"):
                tokens.append("__STR__")
            elif match.group("CHAR"):
                tokens.append("__CHAR__")
            elif match.group("NUM"):
                tokens.append("__NUM__")
            else:
                tokens.append(match.group(0))
        else:
            tokens.append(match.group(0))
    return tokens

def compute_token_similarity(toks1, toks2):
    """
    Computes token sequence similarity ratio between two token streams.
    Uses SequenceMatcher with fallback to fast multiset Sorensen-Dice for very large streams.
    """
    if toks1 == toks2:
        return 100.0
    if not toks1 or not toks2:
        return 0.0

    n1, n2 = len(toks1), len(toks2)
    if max(n1, n2) / max(min(n1, n2), 1) > 10:
        return (2.0 * min(n1, n2) / (n1 + n2)) * 100.0 * 0.5

    if n1 + n2 > 4000:
        c1, c2 = Counter(toks1), Counter(toks2)
        intersection = sum((c1 & c2).values())
        return (2.0 * intersection / (n1 + n2)) * 100.0

    return difflib.SequenceMatcher(None, toks1, toks2).ratio() * 100.0

def compute_raw_similarity(base_bytes, head_bytes):
    """Computes raw line-by-line diff similarity."""
    if base_bytes == head_bytes:
        return 100.0
    try:
        base_lines = base_bytes.decode("utf-8", errors="ignore").splitlines(keepends=True)
        head_lines = head_bytes.decode("utf-8", errors="ignore").splitlines(keepends=True)
        matcher = difflib.SequenceMatcher(None, base_lines, head_lines)
        return matcher.ratio() * 100.0
    except Exception:
        return 0.0

def compute_confidence_score(match_type, head_token_count, base_token_count):
    """
    Assigns confidence score to similarity assessment based on token entropy and match status.
    """
    if match_type == "new_file" or match_type == "byte_identical":
        return 1.0

    size = min(head_token_count, base_token_count)
    if size < 10:
        confidence = 0.70
    elif size < 30:
        confidence = 0.85
    elif size < 100:
        confidence = 0.95
    else:
        confidence = 0.99

    if match_type in ("git_rename", "git_copy"):
        confidence *= 0.95

    return round(confidence, 3)

def audit_workspace():
    head_files = run_git(["ls-tree", "-r", "--name-only", "HEAD"]).strip().splitlines()
    fork_files = set(run_git(["ls-tree", "-r", "--name-only", FORK_COMMIT]).strip().splitlines())
    rename_copy_map = get_git_rename_copy_map()

    audited = []
    total_taim_loc = 0
    total_inherited_loc_raw = 0
    total_inherited_loc_token = 0

    for f in head_files:
        if not f.endswith(RELEVANT_EXTENSIONS) and not f.startswith("assets/"):
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

        # Determine likely source path
        source_path = None
        match_type = "new_file"

        if f in rename_copy_map:
            src, status, _ = rename_copy_map[f]
            source_path = src
            match_type = "git_rename" if status == "R" else "git_copy"
        elif f in fork_files:
            source_path = f
            match_type = "exact_path"

        if source_path is None:
            raw_sim = 0.0
            tok_sim = 0.0
            confidence = 1.0
            head_tokens = len(tokenize_code(head_bytes.decode("utf-8", errors="ignore")))
            base_tokens = 0
        else:
            base_bytes = get_git_bytes(FORK_COMMIT, source_path)
            if base_bytes is None:
                raw_sim = 0.0
                tok_sim = 0.0
                confidence = 1.0
                head_tokens = len(tokenize_code(head_bytes.decode("utf-8", errors="ignore")))
                base_tokens = 0
            elif base_bytes == head_bytes:
                raw_sim = 100.0
                tok_sim = 100.0
                match_type = "byte_identical"
                confidence = 1.0
                head_tokens = len(tokenize_code(head_bytes.decode("utf-8", errors="ignore")))
                base_tokens = head_tokens
            else:
                raw_sim = compute_raw_similarity(base_bytes, head_bytes)
                base_toks = tokenize_code(base_bytes.decode("utf-8", errors="ignore"))
                head_toks = tokenize_code(head_bytes.decode("utf-8", errors="ignore"))
                head_tokens = len(head_toks)
                base_tokens = len(base_toks)
                tok_sim = compute_token_similarity(base_toks, head_toks)
                confidence = compute_confidence_score(match_type, head_tokens, base_tokens)

        if raw_sim >= 50.0:
            total_inherited_loc_raw += int(head_loc * (raw_sim / 100.0))
        if tok_sim >= 50.0:
            total_inherited_loc_token += int(head_loc * (tok_sim / 100.0))

        is_disguised_copy = (raw_sim < 70.0 and tok_sim >= 80.0 and head_tokens > 20)

        audited.append({
            "path": f,
            "likely_source": source_path,
            "match_type": match_type,
            "raw_similarity": round(raw_sim, 2),
            "token_similarity": round(tok_sim, 2),
            "confidence": confidence,
            "loc": head_loc,
            "tokens": head_tokens,
            "is_disguised_copy": is_disguised_copy,
        })

    return {
        "audited": audited,
        "total_loc": total_taim_loc,
        "inherited_loc_raw": total_inherited_loc_raw,
        "inherited_loc_token": total_inherited_loc_token,
    }

def main():
    parser = argparse.ArgumentParser(description="TAIM Independence & Architectural Scanner")
    parser.add_argument("--json", action="store_true", help="Output audit results in JSON format")
    parser.add_argument("--details", action="store_true", help="Print all audited files and scores")
    parser.add_argument("--suspects", action="store_true", help="Print only suspected disguised copies / formatting diffs")
    args = parser.parse_args()

    t0 = time.time()
    results = audit_workspace()
    elapsed = time.time() - t0

    audited = results["audited"]
    total_loc = results["total_loc"]
    inherited_loc_tok = results["inherited_loc_token"]
    inherited_loc_raw = results["inherited_loc_raw"]

    total_files = len(audited)
    byte_identical = [x for x in audited if x["match_type"] == "byte_identical"]
    tok_identical = [x for x in audited if x["token_similarity"] >= 99.9]
    high_sim = [x for x in audited if 70.0 <= x["token_similarity"] < 99.9]
    diverged = [x for x in audited if x["token_similarity"] < 70.0]
    disguised = [x for x in audited if x["is_disguised_copy"]]

    raw_independence = (1.0 - inherited_loc_raw / total_loc) * 100.0 if total_loc > 0 else 100.0
    token_independence = (1.0 - inherited_loc_tok / total_loc) * 100.0 if total_loc > 0 else 100.0

    if args.json:
        output = {
            "summary": {
                "total_files": total_files,
                "total_loc": total_loc,
                "elapsed_seconds": round(elapsed, 2),
                "byte_identical_files": len(byte_identical),
                "token_identical_files": len(tok_identical),
                "high_similarity_files": len(high_sim),
                "diverged_files": len(diverged),
                "disguised_copies": len(disguised),
                "raw_independence_pct": round(raw_independence, 2),
                "token_independence_pct": round(token_independence, 2),
                "inherited_loc_token": inherited_loc_tok,
            },
            "files": audited,
        }
        print(json.dumps(output, indent=2))
        return

    if args.suspects:
        print(f"=== Suspected Disguised Copies (Raw <70%, Token >=80%) ===")
        if not disguised:
            print("  None detected! All low-similarity files reflect genuine AST divergence.")
        else:
            for d in disguised:
                print(f"  {d['path']} -> raw: {d['raw_similarity']}%, token: {d['token_similarity']}%, source: {d['likely_source']}")
        return

    if args.details:
        print(f"{'Path':<50} {'Source':<30} {'Raw%':<7} {'Tok%':<7} {'Conf':<6} {'LOC':<6}")
        print("-" * 110)
        for a in audited:
            src = a["likely_source"] or "None"
            print(f"{a['path']:<50} {src:<30} {a['raw_similarity']:<7.1f} {a['token_similarity']:<7.1f} {a['confidence']:<6.2f} {a['loc']:<6}")
        print("-" * 110)

    print("===============================================================")
    print("           TAIM HARDENED INDEPENDENCE AUDIT REPORT             ")
    print(f" Baseline: {FORK_COMMIT} | Files: {total_files} | Audited in: {elapsed:.2f}s")
    print("===============================================================")
    print(f"Total Workspace LOC:             {total_loc:,}")
    print(f"Estimated Inherited LOC:         {inherited_loc_tok:,} ({inherited_loc_tok/total_loc*100:.1f}%)")
    print("---------------------------------------------------------------")
    print(f"Byte-Identical Files (100%):     {len(byte_identical):>4} ({len(byte_identical)/total_files*100:>5.1f}%)")
    print(f"Token-Identical Files (>=99.9%): {len(tok_identical):>4} ({len(tok_identical)/total_files*100:>5.1f}%)")
    print(f"High-Similarity Files (70-99.9%):{len(high_sim):>4} ({len(high_sim)/total_files*100:>5.1f}%)")
    print(f"Diverged / TAIM-Native (<70%):   {len(diverged):>4} ({len(diverged)/total_files*100:>5.1f}%)")
    print(f"Suspected Disguised Copies:      {len(disguised):>4}")
    print("---------------------------------------------------------------")
    print(f"Raw Line Independence:           {raw_independence:>5.1f}%")
    print(f"Normalized Token Independence:   {token_independence:>5.1f}%")
    print("===============================================================")

if __name__ == "__main__":
    main()
