# TAIM asset icons

This directory contains **54 named SVG assets** after Edition 01 is applied:
36 pre-existing monochrome UI icons, with **12 premium replacements**,
plus 18 newly named premium SVG icons.

## Premium replacements at the root

The 30 full-color, 64×64, editable premium SVGs in
[assets/icons/premium](./premium/) are also exposed directly as
`assets/icons/<name>.svg`. For example:
`assets/icons/home.svg`, `assets/icons/terminal.svg` and
`assets/icons/settings.svg`.

**Permanent source of truth:** `assets/icons/premium/*.svg`.
The repository's `scripts/assets/build-icon-font.py` has been updated
to restore these 30 root SVGs **after** regenerating monochrome icon-font
artwork. Do not hand-edit the root premium copies; edit the premium source.

The existing icon font (`graphics/fonts/Taim-Icons.ttf`) and
its icon codepoint APIs are intentionally **not** replaced by multicolor
SVGs. The `assets/icons/manifest.toml` manifest covers icon-font
supported glyphs only; the premium-only icons are cataloged in
`premium/manifest.json`.

The remaining root-only monochrome SVGs continue to follow
`scripts/assets/icon_geometry.py`.

## Checks (offline)

```bash
python3 scripts/assets/validate-premium-icons.py
bash scripts/design/check-icons.sh
```

Note: replacing file assets on a feature branch does not automatically
switch all runtime controls to render them. Check SVG renderer support,
scaling, accessibility and UI fallback before merging to `main`.
