#!/usr/bin/env bash
# Validate TAIM font glyph manifest and the 60 full-color root SVG exports.
# Run from repository root. GitHub CI stays commented at owner's request.
set -euo pipefail

python3 - <<'PY'
import tomllib
from pathlib import Path

root = Path("assets/icons")
with (root / "manifest.toml").open("rb") as file:
    data = tomllib.load(file)

icons = data["icon"]
names = [item["name"] for item in icons]
assert len(names) == len(set(names)), "Duplicate icon-font entries"
assert len(icons) >= 36, "Unexpected loss of existing font glyphs"
premium_count = 0
for icon in icons:
    name = icon["name"]
    assert (root / f"{name}.svg").is_file(), f"Missing SVG: {name}"
    assert icon["license"], f"Missing license for: {name}"
    assert icon["codepoint"].startswith("U+"), f"Missing font glyph codepoint: {name}"
    variant = icon.get("asset_variant", "icon-font")
    assert variant in ("premium-svg", "icon-font"), f"Unexpected variant: {variant}"
    expected_grid = 64 if variant == "premium-svg" else 24
    assert icon["grid"] == expected_grid, f"Wrong grid for {name}"
    premium_count += variant == "premium-svg"

assert premium_count == 12, "Expected 12 premium replacements of existing glyph SVGs"
print(f"Font manifest validated: {len(icons)} entries; {premium_count} premium overrides")
PY

python3 scripts/assets/validate-premium-icons.py
