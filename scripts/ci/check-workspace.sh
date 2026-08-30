#!/usr/bin/env bash
# Detects the class of breakage phase.md Phase 0 exists to fix: workspace
# members or path dependencies pointing at directories that do not exist,
# and forbidden features (X11/web/software-only extras) leaking into the
# VYOM default profile.
#
# Exit code 0: workspace is internally consistent.
# Exit code 1: at least one problem was found; details on stdout.

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

failures=0

fail() {
    echo "FAIL: $1"
    failures=$((failures + 1))
}

echo "== Checking workspace members resolve to real directories =="

members=$(cargo metadata --format-version 1 --no-deps 2>/dev/null | \
    python3 -c '
import json, sys
data = json.load(sys.stdin)
for pkg in data["packages"]:
    manifest = pkg["manifest_path"]
    print(manifest)
' 2>/dev/null || true)

if [[ -z "${members}" ]]; then
    fail "cargo metadata failed to run; workspace manifest is likely broken (dangling member/path dependency)."
    cargo metadata --format-version 1 --no-deps 2>&1 | tail -20
else
    while IFS= read -r manifest; do
        if [[ ! -f "${manifest}" ]]; then
            fail "workspace member manifest does not exist: ${manifest}"
        fi
    done <<< "${members}"
    echo "  ok: $(echo "${members}" | wc -l) member manifest(s) resolved."
fi

echo "== Checking for path dependencies pointing at missing directories =="

while IFS=: read -r file line_no line; do
    # Only match Cargo's dependency-table `path = "..."` key, not arbitrary
    # strings that happen to contain "path" (e.g. version requirements).
    path=$(echo "${line}" | sed -n 's/^[^#]*\bpath *= *"\([^"]*\)".*/\1/p')
    [[ -z "${path}" ]] && continue

    dir=$(dirname "${file}")
    target="${dir}/${path}"

    if [[ ! -e "${target}" ]]; then
        fail "${file}:${line_no}: path dependency '${path}' does not resolve (looked for ${target})"
    fi
done < <(grep -rn '^\s*path\s*=\s*"' --include='Cargo.toml' . 2>/dev/null || true)

echo "== Checking no manifest still depends on known-removed crates =="

# Source files may still mention these names inside code gated by a
# feature that no longer exists anywhere -- that is dead-but-harmless
# code, not a workspace break. What must never happen is a *manifest*
# still declaring the dependency, since that is what makes
# `cargo metadata` fail outright. See docs/architecture/path-map.md.
#
# `taim_highlighter` is deliberately absent from this list: it is now a
# real workspace member (`taim-highlighter`), not a removed crate.
removed_crates=(taim_beacon taim_devtools taim_selector taim_tester taim_test)

for crate in "${removed_crates[@]}"; do
    hits=$(grep -rn "^\s*${crate}\s*\.\?\s*\(=\|workspace\)" --include='Cargo.toml' \
        --exclude-dir=target --exclude-dir=.git . 2>/dev/null || true)

    if [[ -n "${hits}" ]]; then
        fail "manifest still depends on removed crate '${crate}':"
        echo "${hits}"
    fi
done

echo "== Checking forbidden features are absent from the default profile =="

forbidden_default_features=(x11-only web webgl qr_code markdown svg)
default_line=$(grep -m1 '^default = \[' Cargo.toml || true)

for feature in "${forbidden_default_features[@]}"; do
    if echo "${default_line}" | grep -q "\"${feature}\""; then
        fail "forbidden feature '${feature}' is present in the default profile: ${default_line}"
    fi
done

echo "== Checking required VYOM-profile features remain in the default profile =="

required_default_features=(wgpu tiny-skia wayland)

for feature in "${required_default_features[@]}"; do
    if ! echo "${default_line}" | grep -q "\"${feature}\""; then
        fail "required feature '${feature}' is missing from the default profile: ${default_line}"
    fi
done

echo
if [[ "${failures}" -gt 0 ]]; then
    echo "check-workspace: ${failures} problem(s) found."
    exit 1
fi

echo "check-workspace: workspace is consistent."
