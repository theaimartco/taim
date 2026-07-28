#!/usr/bin/env bash
# Validates the small, reviewable icon manifest before SVG assets are accepted.
set -euo pipefail

manifest="assets/icons/manifest.toml"
test -f "${manifest}"
rg -n '^name = "[a-z0-9-]+"$' "${manifest}"
rg -n '^grid = (16|20|24|32)$' "${manifest}"
rg -n '^license = "[^" ]+[^" ]*"$' "${manifest}"
