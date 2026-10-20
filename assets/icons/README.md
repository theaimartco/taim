# TAIM `assets/icons` — 84 SVG assets

This directory contains **84 distinct root SVGs**: 36 original icon-font-compatible glyph names (12 also with premium overrides), 18 new SVG-only names from Premium Edition 01, and 30 new SVG-only names from Premium Edition 02.

**60 premium SVGs** appear as identical root exports, with authoritative sources in [premium/](./premium/) and [premium/manifest.json](./premium/manifest.json). Batch 02 adds Save, Copy, Paste, Cut, Undo, Redo, Refresh, Sync, Share, Link, Unlink, Attachment, Bookmark, Pin, Sort, List, Grid, Fullscreen, Maximize, Minimize, Help, History and other UI controls.

Edit the `assets/icons/premium/<name>.svg` source for any change; `assets/icons/<name>.svg` is its copy. `scripts/assets/build-icon-font.py` copies premium files *after* regenerating the 36 monochrome glyphs. Existing `graphics/fonts/Taim-Icons.ttf` codepoints remain unchanged.

The new 30 Batch 02 SVGs are not automatically installed as codepoints or Rust `Icon` constants. Runtime SVG rendering must be integrated separately.

View [premium/README.md](./premium/README.md) or [premium/gallery.html](./premium/gallery.html) to review all 60 icons.

## Manual checks

```bash
python3 scripts/assets/validate-premium-icons.py
bash scripts/design/check-icons.sh
```

CI workflows remain disabled at owner request. Manual rendering, small-size contrast and accessibility checks are still necessary.
