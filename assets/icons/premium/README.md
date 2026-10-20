# TAIM Premium Icons — Editions 01 + 02

**60 native, standalone, editable vector SVG icons** — no raster image wrappers. Each uses a 64×64 transparent coordinate grid with polished graphite/chrome material and TAIM crimson gradients. Batch 02 UI-action icons have independently drawn paths and shapes; PNG previews are not embedded.

## Gallery

Browse all 60 in [gallery.html](./gallery.html), or see the actual SVG previews below.

### Edition 01 — System & application (30)

| Icon | Icon | Icon | Icon | Icon |
|:---:|:---:|:---:|:---:|:---:|
|![home](./home.svg)<br>home|![settings](./settings.svg)<br>settings|![folder](./folder.svg)<br>folder|![search](./search.svg)<br>search|![notifications](./notifications.svg)<br>notifications|
|![user](./user.svg)<br>user|![chat](./chat.svg)<br>chat|![terminal](./terminal.svg)<br>terminal|![store](./store.svg)<br>store|![power](./power.svg)<br>power|
|![lock](./lock.svg)<br>lock|![shield](./shield.svg)<br>shield|![calendar](./calendar.svg)<br>calendar|![clock](./clock.svg)<br>clock|![camera](./camera.svg)<br>camera|
|![image](./image.svg)<br>image|![music](./music.svg)<br>music|![download](./download.svg)<br>download|![upload](./upload.svg)<br>upload|![cloud](./cloud.svg)<br>cloud|
|![database](./database.svg)<br>database|![cpu](./cpu.svg)<br>cpu|![code](./code.svg)<br>code|![rocket](./rocket.svg)<br>rocket|![chart](./chart.svg)<br>chart|
|![globe](./globe.svg)<br>globe|![heart](./heart.svg)<br>heart|![star](./star.svg)<br>star|![trash](./trash.svg)<br>trash|![edit](./edit.svg)<br>edit|

### Edition 02 — UI actions (30)

| Icon | Icon | Icon | Icon | Icon |
|:---:|:---:|:---:|:---:|:---:|
|![save](./save.svg)<br>save|![copy](./copy.svg)<br>copy|![paste](./paste.svg)<br>paste|![cut](./cut.svg)<br>cut|![undo](./undo.svg)<br>undo|
|![redo](./redo.svg)<br>redo|![refresh](./refresh.svg)<br>refresh|![sync](./sync.svg)<br>sync|![share](./share.svg)<br>share|![link](./link.svg)<br>link|
|![unlink](./unlink.svg)<br>unlink|![attachment](./attachment.svg)<br>attachment|![bookmark](./bookmark.svg)<br>bookmark|![pin](./pin.svg)<br>pin|![sort](./sort.svg)<br>sort|
|![sort_ascending](./sort_ascending.svg)<br>sort_ascending|![sort_descending](./sort_descending.svg)<br>sort_descending|![list](./list.svg)<br>list|![grid](./grid.svg)<br>grid|![expand](./expand.svg)<br>expand|
|![collapse](./collapse.svg)<br>collapse|![fullscreen](./fullscreen.svg)<br>fullscreen|![exit_fullscreen](./exit_fullscreen.svg)<br>exit_fullscreen|![maximize](./maximize.svg)<br>maximize|![minimize](./minimize.svg)<br>minimize|
|![backspace](./backspace.svg)<br>backspace|![clear](./clear.svg)<br>clear|![help](./help.svg)<br>help|![external_link](./external_link.svg)<br>external_link|![history](./history.svg)<br>history|

## Usage and source of truth

```html
<img src="/assets/icons/save.svg" width="48" height="48" alt="Save">
<img src="/assets/icons/premium/fullscreen.svg" width="48" height="48" alt="Fullscreen">
```

Both `assets/icons/<name>.svg` and `assets/icons/premium/<name>.svg` have identical source bytes. Premium files are authoritative; the font builder copies them to root after monochrome glyph regeneration.

The existing `graphics/fonts/Taim-Icons.ttf` and its 36 font codepoints are unchanged. All 30 Batch 02 additions are **SVG-only**, not automatically available as Rust `Icon` glyph constants. Widget integration requires additional work.

At 16px use simpler monochrome alternatives where these detailed bevels lose clarity; manually check visual contrast and accessibility.

## Verify

```bash
python3 scripts/assets/validate-premium-icons.py
bash scripts/design/check-icons.sh
```

MIT project licensing and historical third-party attribution are unchanged.
