#!/usr/bin/env bash
set -euo pipefail

# scripts/clean-target.sh: Prunes target directory bloat without forcing a full rebuild of 3rd-party crates.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${ROOT_DIR}/target"

if [ ! -d "${TARGET_DIR}" ]; then
    echo "target directory does not exist: ${TARGET_DIR}"
    exit 0
fi

echo "=== Target Directory Size Before Cleanup ==="
du -sh "${TARGET_DIR}" || true

echo "Pruning stale incremental compilation artifacts..."
rm -rf "${TARGET_DIR}/debug/incremental"/* 2>/dev/null || true
rm -rf "${TARGET_DIR}/release/incremental"/* 2>/dev/null || true

echo "Pruning temporary target files..."
rm -rf "${TARGET_DIR}/tmp"/* 2>/dev/null || true

echo "Pruning macOS dSYM debug bundles..."
find "${TARGET_DIR}" -name "*.dSYM" -type d -exec rm -rf {} + 2>/dev/null || true

echo "Pruning stale test executables and orphaned artifacts..."
find "${TARGET_DIR}/debug/deps" -maxdepth 1 -type f -perm +111 ! -name "*.dylib" ! -name "*.so" -delete 2>/dev/null || true
find "${TARGET_DIR}/release/deps" -maxdepth 1 -type f -perm +111 ! -name "*.dylib" ! -name "*.so" -delete 2>/dev/null || true
find "${TARGET_DIR}/debug" -maxdepth 1 -type f -delete 2>/dev/null || true

echo "Pruning duplicate stale crate hashes in target/*/deps..."
python3 - << 'EOF'
import os, re

for profile in ["debug", "release"]:
    deps_dir = os.path.join("target", profile, "deps")
    if not os.path.isdir(deps_dir):
        continue
    crates = {}
    pattern = re.compile(r"^(lib)?([a-zA-Z0-9_]+)-([0-9a-f]{16})\.([a-z_]+)$")
    for fname in os.listdir(deps_dir):
        m = pattern.match(fname)
        if m:
            prefix, name, hash_val, ext = m.groups()
            key = (prefix or "", name, ext)
            fpath = os.path.join(deps_dir, fname)
            try:
                st = os.stat(fpath)
                crates.setdefault(key, []).append((st.st_mtime, st.st_size, fpath))
            except OSError:
                pass

    reclaimed = 0
    removed_count = 0
    for key, entries in crates.items():
        if len(entries) > 1:
            entries.sort(key=lambda x: x[0], reverse=True)
            for _, sz, fpath in entries[1:]:
                try:
                    os.remove(fpath)
                    reclaimed += sz
                    removed_count += 1
                except OSError:
                    pass

    if removed_count > 0:
        print(f"[{profile}] Pruned {removed_count} stale duplicate artifacts ({reclaimed / (1024*1024):.1f} MB reclaimed)")
EOF

echo "Pruning intermediate codegen unit object files (*.rcgu.o)..."
find "${TARGET_DIR}" -name "*.rcgu.o" -delete 2>/dev/null || true

echo "Pruning duplicate stale build script directories in target/*/build..."
python3 - << 'EOF'
import os, re

for profile in ["debug", "release"]:
    build_dir = os.path.join("target", profile, "build")
    if not os.path.isdir(build_dir):
        continue
    crates = {}
    pattern = re.compile(r"^([a-zA-Z0-9_]+)-([0-9a-f]{16})$")
    for dname in os.listdir(build_dir):
        m = pattern.match(dname)
        if m:
            name, hash_val = m.groups()
            fpath = os.path.join(build_dir, dname)
            try:
                st = os.stat(fpath)
                crates.setdefault(name, []).append((st.st_mtime, fpath))
            except OSError:
                pass

    pruned = 0
    for name, entries in crates.items():
        if len(entries) > 1:
            entries.sort(key=lambda x: x[0], reverse=True)
            for _, fpath in entries[1:]:
                pruned += 1
                os.system(f"rm -rf {fpath}")
    if pruned > 0:
        print(f"[{profile}] Pruned {pruned} duplicate build directories")
EOF

if [[ "${1:-}" == "--ultra" || "${1:-}" == "--check-only" ]]; then
    echo "Ultra mode: pruning heavy binary .rlib archives (leaving fast .rmeta metadata for check workflows)..."
    find "${TARGET_DIR}" -name "*.rlib" -delete 2>/dev/null || true
fi

echo "=== Target Directory Size After Cleanup ==="
du -sh "${TARGET_DIR}" || true
echo "Cleanup completed successfully."
