#!/usr/bin/env bash
# Reproducible LOC accounting for phase budgets (see phase.md section 2).
#
# Counts non-blank source/test/config/documentation lines using `tokei`,
# pinned to a single version so local and CI runs agree. Excludes:
#   - generated code (target/, *.lock)
#   - vendored code (none tracked in this workspace)
#   - shader compiler output (*.spv)
#   - golden PNG/image bytes (tests/golden/**/*.png, *.jpg, *.gif)
#
# Usage:
#   scripts/loc.sh                 # total LOC across the whole workspace
#   scripts/loc.sh <path> [<path>...]   # LOC for specific files/directories
#   scripts/loc.sh --diff <base-ref>    # net new LOC vs. a git ref (for phase ceilings)

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

REQUIRED_TOKEI_VERSION="12"

require_tokei() {
    if ! command -v tokei >/dev/null 2>&1; then
        echo "error: tokei is required (cargo install tokei --version ^${REQUIRED_TOKEI_VERSION})" >&2
        exit 1
    fi

    local version
    version=$(tokei --version | awk '{print $2}')

    if [[ "${version%%.*}" != "$REQUIRED_TOKEI_VERSION" ]]; then
        echo "error: tokei v${REQUIRED_TOKEI_VERSION}.x required, found v${version}" >&2
        exit 1
    fi
}

EXCLUDES=(
    --exclude "target"
    --exclude "*.lock"
    --exclude "*.spv"
    --exclude "tests/golden/**/*.png"
    --exclude "tests/golden/**/*.jpg"
    --exclude "tests/golden/**/*.gif"
    --exclude "assets/fonts/**"
    --exclude "assets/icons/**"
)

require_tokei

if [[ "${1:-}" == "--diff" ]]; then
    base_ref="${2:?usage: scripts/loc.sh --diff <base-ref>}"

    added_removed=$(git diff --numstat "${base_ref}"...HEAD -- . \
        ':(exclude)Cargo.lock' \
        ':(exclude)*.spv' \
        ':(exclude)tests/golden/**/*.png' \
        ':(exclude)tests/golden/**/*.jpg' \
        ':(exclude)tests/golden/**/*.gif' \
        ':(exclude)assets/fonts/**' \
        ':(exclude)assets/icons/**' \
        | awk '{added += $1; removed += $2} END {printf "added=%d removed=%d net=%d\n", added, removed, added - removed}')

    echo "${added_removed}"
    exit 0
fi

if [[ $# -gt 0 ]]; then
    tokei "${EXCLUDES[@]}" --output json "$@"
else
    tokei "${EXCLUDES[@]}" --output json .
fi
