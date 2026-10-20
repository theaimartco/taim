# TAIM Icons — 119 Premium SVGs

The root `assets/icons/` directory now holds **119 unique premium vector SVG files**. All root icons are exact copies of `assets/icons/premium/` source files, with a machine-readable [119-icon manifest](./premium/manifest.json).

### Legacy SVG retirement and Batch 03

- **24 legacy monochrome SVG artworks replaced**, preserving their useful icon filenames (such as `add`, `chevron_down`, `close`, `play`, `pause`, `warning`, `mail`, `filter`).
- **35 new premium SVG concepts**: Wi-Fi, connectivity, battery, audio/video equipment, keyboard, form states, progress, document types, commerce and AI sparkle.
- Previous 60 premium SVGs remain preserved.
- Existing 36 icon-font codepoints remain unchanged; no font binary is modified. The legacy font and Rust glyph API still require a separate audit/migration if the goal is to remove every inherited font implementation.

### Source-of-truth

Edit `premium/<name>.svg`, then use `scripts/assets/build-icon-font.py` to regenerate root exports. The generator no longer writes legacy 24×24 SVG artwork into the root folder. **It still generates the older-compatible icon font**; regenerating the font has independent build and provenance requirements.

Browse [119 previews](./premium/gallery.html) or read [the premium pack guide](./premium/README.md).

### Validation

```bash
python3 scripts/assets/validate-premium-icons.py
bash scripts/design/check-icons.sh
```

The CI YAML files remain disabled at owner request. Do not claim these source changes validate runtime rendering or the entire framework's clean-room status.
