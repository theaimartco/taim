#!/usr/bin/env python3
"""Fail-closed verification of release evidence manifests. Not physical attestation."""

import argparse
from datetime import datetime
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

REQUIRED_GATES = (
    "local_verification_executed", "rust_clippy_tests", "wayland_vyom_presentation",
    "input_multilingual_ime", "real_screen_reader", "gpu_loss_recovery",
    "memory_gpu_idle_power", "physical_input_to_photon", "four_framework_comparison",
    "external_api_consumer", "security_license_review", "independent_rebuild",
)
SHA256 = re.compile(r"[0-9a-fA-F]{64}\Z")
COMMIT = re.compile(r"[0-9a-fA-F]{40}\Z")


def verify(manifest_path, expected_commit, repo_root=None):
    path = Path(manifest_path).resolve()
    manifest = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(manifest, dict) or manifest.get("schema_version") != 1:
        raise ValueError("unsupported or absent manifest schema")
    if not isinstance(expected_commit, str) or not COMMIT.fullmatch(expected_commit):
        raise ValueError("expected commit must be a full 40-hex SHA")
    if manifest.get("candidate_commit") != expected_commit:
        raise ValueError("candidate commit differs from requested HEAD")
    if repo_root is not None:
        root = Path(repo_root).resolve()
        head = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
        dirty = subprocess.check_output(["git", "-C", str(root), "status", "--porcelain"], text=True)
        if head != expected_commit or dirty:
            raise ValueError("release checkout has wrong HEAD or uncommitted files")
    operator = manifest.get("operator")
    platforms = manifest.get("supported_targets")
    if not isinstance(operator, str) or not operator.strip():
        raise ValueError("operator identity required")
    if not isinstance(platforms, list) or not platforms or any(not isinstance(p, str) or not p for p in platforms):
        raise ValueError("nonempty supported_targets list required")
    if "VYOM-native-Wayland" not in platforms:
        raise ValueError("no VYOM-native-Wayland target claimed/tested")
    gates = manifest.get("gates")
    if not isinstance(gates, dict) or set(gates) != set(REQUIRED_GATES):
        raise ValueError("missing, renamed or unexpected required gate")
    root = path.parent
    for name in REQUIRED_GATES:
        gate = gates[name]
        if not isinstance(gate, dict) or gate.get("status") != "PASS":
            raise ValueError(f"{name}: not independently marked PASS")
        reviewer = gate.get("reviewer")
        if not isinstance(reviewer, str) or not reviewer.strip() or reviewer.strip().casefold() == operator.strip().casefold():
            raise ValueError(f"{name}: require reviewer independent of operator")
        time = gate.get("reviewed_at")
        if not isinstance(time, str):
            raise ValueError(f"{name}: missing review timestamp")
        try:
            dt = datetime.fromisoformat(time.replace("Z", "+00:00"))
        except ValueError as error:
            raise ValueError(f"{name}: invalid review timestamp") from error
        if dt.tzinfo is None:
            raise ValueError(f"{name}: timestamp needs timezone")
        proofs = gate.get("evidence")
        if not isinstance(proofs, list) or not proofs:
            raise ValueError(f"{name}: at least one hashed evidence artifact required")
        for proof in proofs:
            if not isinstance(proof, dict):
                raise ValueError(f"{name}: invalid evidence record")
            rel, claimed = proof.get("path"), proof.get("sha256")
            if not isinstance(rel, str) or not rel or Path(rel).is_absolute():
                raise ValueError(f"{name}: evidence path must be relative")
            if not isinstance(claimed, str) or not SHA256.fullmatch(claimed):
                raise ValueError(f"{name}: invalid evidence SHA-256")
            target = (root / rel).resolve()
            if not target.is_relative_to(root) or not target.is_file() or target.stat().st_size > 128 * 1024 * 1024:
                raise ValueError(f"{name}: missing/outside/oversized evidence file: {rel}")
            h = hashlib.sha256()
            with target.open("rb") as stream:
                for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                    h.update(chunk)
            if h.hexdigest() != claimed.lower():
                raise ValueError(f"{name}: mismatched evidence SHA-256: {rel}")
    return {
        "schema_version": 1,
        "commit": expected_commit,
        "gates": len(REQUIRED_GATES),
        "status": "evidence_manifest_integrity_passed_not_independent_physical_certification",
        "note": "Hash checking does not prove measurements or reviewer attestations are genuine.",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--repo-root", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        result = verify(args.manifest, args.commit, args.repo_root)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    except (OSError, ValueError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        print(f"NO-GO: {error}", file=sys.stderr)
        return 1
    print("Evidence manifest integrity verified, independent device/security signoff still required.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
