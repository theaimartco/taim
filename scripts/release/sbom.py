#!/usr/bin/env python3
"""Produce an SPDX 2.3 Cargo dependency SBOM, not release approval."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib
from urllib.parse import quote


def build(metadata, lock, created):
    packages = metadata["packages"]
    identities = {p["id"]: "SPDXRef-Cargo-" + hashlib.sha256(p["id"].encode()).hexdigest()[:24]
                  for p in packages}
    checksums = {(p["name"], p["version"], p.get("source")): p.get("checksum")
                 for p in lock.get("package", [])}
    records = []
    for package in sorted(packages, key=lambda p: p["id"]):
        registry = package.get("source", "") or ""
        crates_io = registry == "registry+https://github.com/rust-lang/crates.io-index"
        name, version = package["name"], package["version"]
        record = {"SPDXID": identities[package["id"]], "name": name,
                  "versionInfo": version, "filesAnalyzed": False,
                  "downloadLocation": f"https://crates.io/api/v1/crates/{quote(name)}/{quote(version)}/download" if crates_io else "NOASSERTION",
                  "licenseConcluded": "NOASSERTION", "licenseDeclared": package.get("license") or "NOASSERTION",
                  "copyrightText": "NOASSERTION"}
        if crates_io:
            record["externalRefs"] = [{"referenceCategory": "PACKAGE-MANAGER",
                "referenceType": "purl", "referenceLocator": f"pkg:cargo/{quote(name)}@{quote(version)}"}]
        checksum = checksums.get((name, version, package.get("source")))
        if checksum:
            record["checksums"] = [{"algorithm": "SHA256", "checksumValue": checksum}]
        records.append(record)
    relationships = []
    for root in metadata.get("workspace_members", []):
        if root in identities:
            relationships.append({"spdxElementId": "SPDXRef-DOCUMENT", "relationshipType": "DESCRIBES",
                                  "relatedSpdxElement": identities[root]})
    for node in (metadata.get("resolve") or {}).get("nodes", []):
        for dependency in sorted(set(node.get("dependencies", []))):
            relationships.append({"spdxElementId": identities[node["id"]], "relationshipType": "DEPENDS_ON",
                                  "relatedSpdxElement": identities[dependency]})
    # Include graph/feature resolution in document identity; never equate the
    # inventory with proof that every listed crate is linked into an APK.
    identity = hashlib.sha256(json.dumps(metadata, sort_keys=True).encode()).hexdigest()
    return {"spdxVersion": "SPDX-2.3", "dataLicense": "CC0-1.0", "SPDXID": "SPDXRef-DOCUMENT",
            "name": "Taim Cargo resolved dependency inventory",
            "documentNamespace": "https://taim.rs/spdx/cargo/" + identity,
            "creationInfo": {"creators": ["Tool: taim-sbom-1"], "created": created},
            "documentComment": "Cargo resolution inventory, including applicable build/dev dependencies. Not an APK byte inventory or independent license/release approval.",
            "packages": records, "relationships": relationships}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest-path", type=Path, default=Path("Cargo.toml"))
    parser.add_argument("--target", help="Cargo target triple for dependency filtering")
    parser.add_argument("--features", help="Explicit Cargo feature list")
    parser.add_argument("--all-features", action="store_true")
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    command = ["cargo", "metadata", "--locked", "--offline", "--format-version", "1",
               "--manifest-path", str(args.manifest_path.resolve())]
    if args.target: command += ["--filter-platform", args.target]
    if args.features: command += ["--features", args.features]
    if args.all_features: command += ["--all-features"]
    metadata = json.loads(subprocess.check_output(command))
    lock = tomllib.loads((Path(metadata["workspace_root"]) / "Cargo.lock").read_text())
    created = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    report = build(metadata, lock, created)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"SPDX dependency inventory: {args.output} ({len(report['packages'])} packages)")


if __name__ == "__main__":
    main()
