# TAIM Premium SVG Pack · Edition 03

**119 individually named and editable SVG icons**, transparent 64×64 viewport, actual vector paths and native gradients—not PNG files renamed to SVG. Every premium source is also exposed byte-identically in `assets/icons/<name>.svg`.

This edition converts **all 24 previous monochrome-only root SVG names** to new TAIM-style vector artwork and adds **35 entirely new icons**. It does not delete useful legacy filenames or break consumers of those paths. The 36 icon-font glyph codepoints remain available separately for backward compatibility, and must not be misrepresented as independently re-authored by this asset change.

## Gallery

[Open the gallery](./gallery.html) to browse/filter all 119 symbols.

| Icon | Icon | Icon | Icon | Icon |
|:---:|:---:|:---:|:---:|:---:|
|![add](./add.svg)<br>add|![archive](./archive.svg)<br>archive|![arrow_down](./arrow_down.svg)<br>arrow_down|![arrow_left](./arrow_left.svg)<br>arrow_left|![arrow_right](./arrow_right.svg)<br>arrow_right|
|![arrow_up](./arrow_up.svg)<br>arrow_up|![attachment](./attachment.svg)<br>attachment|![backspace](./backspace.svg)<br>backspace|![battery](./battery.svg)<br>battery|![battery_charging](./battery_charging.svg)<br>battery_charging|
|![bluetooth](./bluetooth.svg)<br>bluetooth|![bookmark](./bookmark.svg)<br>bookmark|![calendar](./calendar.svg)<br>calendar|![camera](./camera.svg)<br>camera|![chart](./chart.svg)<br>chart|
|![chat](./chat.svg)<br>chat|![check](./check.svg)<br>check|![checkbox_checked](./checkbox_checked.svg)<br>checkbox_checked|![checkbox_unchecked](./checkbox_unchecked.svg)<br>checkbox_unchecked|![chevron_down](./chevron_down.svg)<br>chevron_down|
|![chevron_left](./chevron_left.svg)<br>chevron_left|![chevron_right](./chevron_right.svg)<br>chevron_right|![chevron_up](./chevron_up.svg)<br>chevron_up|![clear](./clear.svg)<br>clear|![clock](./clock.svg)<br>clock|
|![close](./close.svg)<br>close|![cloud](./cloud.svg)<br>cloud|![code](./code.svg)<br>code|![collapse](./collapse.svg)<br>collapse|![copy](./copy.svg)<br>copy|
|![cpu](./cpu.svg)<br>cpu|![cut](./cut.svg)<br>cut|![database](./database.svg)<br>database|![delete](./delete.svg)<br>delete|![document](./document.svg)<br>document|
|![download](./download.svg)<br>download|![edit](./edit.svg)<br>edit|![error](./error.svg)<br>error|![exit_fullscreen](./exit_fullscreen.svg)<br>exit_fullscreen|![expand](./expand.svg)<br>expand|
|![external_link](./external_link.svg)<br>external_link|![filter](./filter.svg)<br>filter|![folder](./folder.svg)<br>folder|![folder_add](./folder_add.svg)<br>folder_add|![folder_open](./folder_open.svg)<br>folder_open|
|![fullscreen](./fullscreen.svg)<br>fullscreen|![globe](./globe.svg)<br>globe|![grid](./grid.svg)<br>grid|![heart](./heart.svg)<br>heart|![help](./help.svg)<br>help|
|![history](./history.svg)<br>history|![home](./home.svg)<br>home|![image](./image.svg)<br>image|![info](./info.svg)<br>info|![keyboard](./keyboard.svg)<br>keyboard|
|![link](./link.svg)<br>link|![list](./list.svg)<br>list|![lock](./lock.svg)<br>lock|![mail](./mail.svg)<br>mail|![maximize](./maximize.svg)<br>maximize|
|![menu](./menu.svg)<br>menu|![microphone](./microphone.svg)<br>microphone|![microphone_off](./microphone_off.svg)<br>microphone_off|![minimize](./minimize.svg)<br>minimize|![monitor](./monitor.svg)<br>monitor|
|![more_horiz](./more_horiz.svg)<br>more_horiz|![more_vert](./more_vert.svg)<br>more_vert|![mouse](./mouse.svg)<br>mouse|![music](./music.svg)<br>music|![notifications](./notifications.svg)<br>notifications|
|![paste](./paste.svg)<br>paste|![pause](./pause.svg)<br>pause|![pdf](./pdf.svg)<br>pdf|![person](./person.svg)<br>person|![pin](./pin.svg)<br>pin|
|![play](./play.svg)<br>play|![power](./power.svg)<br>power|![presentation](./presentation.svg)<br>presentation|![printer](./printer.svg)<br>printer|![progress](./progress.svg)<br>progress|
|![qr_code](./qr_code.svg)<br>qr_code|![radio_checked](./radio_checked.svg)<br>radio_checked|![radio_unchecked](./radio_unchecked.svg)<br>radio_unchecked|![redo](./redo.svg)<br>redo|![refresh](./refresh.svg)<br>refresh|
|![remove](./remove.svg)<br>remove|![rocket](./rocket.svg)<br>rocket|![save](./save.svg)<br>save|![search](./search.svg)<br>search|![settings](./settings.svg)<br>settings|
|![share](./share.svg)<br>share|![shield](./shield.svg)<br>shield|![shopping_cart](./shopping_cart.svg)<br>shopping_cart|![slider](./slider.svg)<br>slider|![sort](./sort.svg)<br>sort|
|![sort_ascending](./sort_ascending.svg)<br>sort_ascending|![sort_descending](./sort_descending.svg)<br>sort_descending|![sparkle](./sparkle.svg)<br>sparkle|![spreadsheet](./spreadsheet.svg)<br>spreadsheet|![star](./star.svg)<br>star|
|![store](./store.svg)<br>store|![success](./success.svg)<br>success|![sync](./sync.svg)<br>sync|![terminal](./terminal.svg)<br>terminal|![toggle_off](./toggle_off.svg)<br>toggle_off|
|![toggle_on](./toggle_on.svg)<br>toggle_on|![trash](./trash.svg)<br>trash|![undo](./undo.svg)<br>undo|![unlink](./unlink.svg)<br>unlink|![upload](./upload.svg)<br>upload|
|![usb](./usb.svg)<br>usb|![user](./user.svg)<br>user|![visibility](./visibility.svg)<br>visibility|![volume](./volume.svg)<br>volume|![volume_off](./volume_off.svg)<br>volume_off|
|![wallet](./wallet.svg)<br>wallet|![warning](./warning.svg)<br>warning|![wifi](./wifi.svg)<br>wifi|![wifi_off](./wifi_off.svg)<br>wifi_off|

## Files and integration

- `premium/<name>.svg`: original artwork, the single source of truth.
- `../<name>.svg`: identical root export for existing asset-path consumers.
- `manifest.json`: machine-readable list of 119 unique names.
- `scripts/assets/build-icon-font.py`: regenerates the font codepoints from existing glyph geometry but **no longer generates monochrome root SVGs**; it copies all 119 premium sources to `assets/icons/` instead.
- `scripts/assets/validate-premium-icons.py`: structure, manifest, root parity and security checks.

```bash
python3 scripts/assets/validate-premium-icons.py
bash scripts/design/check-icons.sh
```

**Visual QA outstanding**: review 16/24/32/64px readability, contrast on light/dark themes, keyboard/a11y labels, software/native SVG renderer support and export consistency. The existing `Taim-Icons.ttf` and Rust glyph constants are not replaced by vector-only additions. Preserve historical third-party credits.
