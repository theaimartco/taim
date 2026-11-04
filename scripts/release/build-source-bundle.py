#!/usr/bin/env python3
"""Create a reproducible source bundle; never sign, publish, or approve a release."""
from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def distributable(name: str) -> bool:
    path = PurePosixPath(name)
    excluded = {".git", ".codex", ".agents", ".aws", "__pycache__",
                "target", "build", ".gradle", ".gradle-bootstrap", ".cxx",
                ".externalNativeBuild", ".dart_tool", ".pytest_cache",
                ".mypy_cache", ".ruff_cache", "htmlcov"}
    local_files = {".DS_Store", "Thumbs.db", "local.properties", "key.properties",
                   ".flutter-plugins", ".flutter-plugins-dependencies", "lcov.info"}
    return (not path.is_absolute() and ".." not in path.parts
            and not excluded.intersection(path.parts)
            and bool(path.parts) and path.parts[0] not in {"artifacts", "pkg", "dist", "traces", "logs"}
            and not any(part in local_files or part.startswith(".env")
                        or part == ".coverage" or part.startswith(".coverage.") for part in path.parts)
            and path.suffix not in {".pyc", ".pyo", ".pem", ".key", ".p12", ".pfx",
                                    ".jks", ".keystore", ".apk", ".aab", ".apks",
                                    ".profraw", ".profdata", ".swp", ".swo"}
            and not path.name.endswith("~")
            and not (path.name.startswith("hs_err_pid") and path.suffix == ".log"))


def source_bytes(root: Path, name: str) -> bytes:
    path = root / name
    if not distributable(name) or path.is_symlink() or not path.resolve().is_relative_to(root.resolve()):
        raise ValueError(f"unsafe source path: {name}")
    return path.read_bytes()


def write_bundle(root: Path, names: list[str], output: Path, version: str,
                 source_commit: str, dirty: bool, epoch: int) -> dict:
    if not re.fullmatch(r"[A-Za-z0-9.+-]+", version) or epoch < 0:
        raise ValueError("invalid bundle version or timestamp")
    names = sorted(set(names))
    required = {"Cargo.toml", "Cargo.lock", "LICENSE", "CREDITS.md", "rust-toolchain.toml"}
    if not required.issubset(names):
        raise ValueError("source inventory is missing manifests, attribution or pinned toolchain")
    output.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(prefix=output.name + ".", suffix=".tmp", dir=output.parent)
    temporary = Path(temporary_name)
    prefix = f"taim-{version}"
    entries = []
    try:
        with os.fdopen(descriptor, "wb") as raw, gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
                def add(name: str, data: bytes, mode: int = 0o644) -> None:
                    item = tarfile.TarInfo(f"{prefix}/{name}")
                    item.size, item.mode, item.mtime = len(data), mode, epoch
                    archive.addfile(item, io.BytesIO(data))

                for name in names:
                    data = source_bytes(root, name)
                    mode = 0o755 if (root / name).stat().st_mode & 0o111 else 0o644
                    entries.append({"path": name, "size_bytes": len(data), "mode": mode,
                                    "sha256": hashlib.sha256(data).hexdigest()})
                    add(name, data, mode)
                manifest = {"schema_version": 1, "version": version,
                            "source_commit": source_commit, "working_tree_modified": dirty,
                            "source_date_epoch": epoch, "files": entries,
                            "production_release_approved": False}
                add("TAIM-SOURCE.json", (json.dumps(manifest, sort_keys=True, indent=2) + "\n").encode())
        # Do not produce a mixed candidate if source changed during packaging.
        for entry in entries:
            mode = 0o755 if (root / entry["path"]).stat().st_mode & 0o111 else 0o644
            if mode != entry["mode"] or hashlib.sha256(source_bytes(root, entry["path"])).hexdigest() != entry["sha256"]:
                raise ValueError(f"source changed during packaging: {entry['path']}")
        temporary.replace(output)
        return manifest
    finally:
        temporary.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--allow-dirty", action="store_true", help="Create a labeled working-tree candidate, not an approved release")
    args = parser.parse_args()
    root = args.root.resolve()
    try:
        def git(*parts: str) -> bytes:
            return subprocess.check_output(["git", "-C", str(root), *parts])
        dirty = bool(git("status", "--porcelain"))
        if dirty and not args.allow_dirty:
            raise ValueError("dirty checkout: use --allow-dirty only for an unapproved candidate")
        version = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
        names = [name.decode() for name in git("ls-files", "-co", "--exclude-standard", "-z").split(b"\0") if name]
        names = [name for name in names if distributable(name) and (root / name).exists()]
        output = args.output or root / "target" / "source-dist" / f"taim-{version}.tar.gz"
        manifest = write_bundle(root, names, output, version, git("rev-parse", "HEAD").decode().strip(),
                                dirty, int(git("show", "-s", "--format=%ct", "HEAD")))
        checksum = hashlib.sha256(output.read_bytes()).hexdigest()
        output.with_name(output.name + ".sha256").write_text(f"{checksum}  {output.name}\n")
        print(f"Created {output} ({len(manifest['files'])} source files)")
        print(f"SHA-256: {checksum}; production release approved: false")
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Source bundle rejected: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
