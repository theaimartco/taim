# TAIM on the web

A TAIM application runs in a browser as WebAssembly, drawing into a
`<canvas>` with WebGL2 or WebGPU. The same `taim::app(...).run()` code that
opens a desktop window mounts a canvas in the page instead.

## Build and serve

```sh
cargo install wasm-bindgen-cli --version 0.2.108 --locked   # once; match Cargo.lock
bash scripts/web/build.sh counter                           # or liquid, mobile_shell, your crate
python3 -m http.server -d target/web/counter 8000
```

`scripts/web/build.sh PACKAGE [OUT_DIR] [--webgpu]` builds the package for
`wasm32-unknown-unknown` in release mode, runs `wasm-bindgen` and writes an
`index.html`. The output directory is static files only, so any web host
can serve it.

The canvas replaces an element with `id="taim"` if the page has one, and is
appended to `<body>` otherwise. It fills that element, so size the
placeholder with CSS to embed an application in a larger page.

## Graphics

| Build | Runs in | Notes |
|---|---|---|
| default (`--webgpu`) | Browsers with WebGPU enabled | Full renderer, liquid glass included. |
| `--features taim/webgl` (the script's default) | Every current browser | Glass, backdrop blur and effects are drawn flat: a WebGL canvas cannot be read back, so there is no backdrop to refract. Gradients, shadows and text draw as they do natively. |

The WebGL build is the default because it runs everywhere; pass `--webgpu`
when your audience has WebGPU.

## What works

Verified in headless Chromium (WebGL2 on SwiftShader): `counter`,
`mobile_shell` and `liquid` render with text, icons and gradients, and
clicks update state with no console errors.

- Text: Fira Sans is embedded (a page has no system fonts), as is the TAIM
  icon font. Characters outside Fira Sans (most symbols and emoji, CJK,
  Devanagari) need a font added with `App::font(include_bytes!(...))`.
- Input: mouse and keyboard are verified. Touch arrives through winit but
  is untested in a browser; the canvas sets `touch-action: none` so that
  drags reach the application instead of scrolling the page.
- Time and animation use `web-time`; panics are reported to the console
  with their message.

## Platform services

| Service | On the web |
|---|---|
| Clipboard | Text through `navigator.clipboard`: copy, cut and paste in text fields. Needs a secure context (`https:` or `localhost`); the browser may ask before the first paste. |
| `taim::storage` | Kept in the page's `localStorage`, per origin. |
| `platform::open_url` | Opens a new tab. Browsers allow it only in response to a click or tap. |
| `platform::share_text` | The Web Share API, where the browser offers it (mostly mobile). |
| `platform::vibrate` | The Vibration API (Android browsers), after the user has tapped the page. |

Verified in headless Chromium: typing into a text field, copy and paste
through the system clipboard, storage surviving a reload, `open_url`
opening a tab.

## Not yet

- Screen readers: the semantics tree is not exposed to the page's
  accessibility tree.
- Text input methods (IME composition) are not handled.
- Images and SVG compile for the web but have not been checked in a browser.
- High-DPI rendering follows the browser's `devicePixelRatio` through
  winit, but has not been checked on a real high-DPI screen. DevTools device
  emulation reports CSS pixels as device pixels, so an application looks
  enlarged there; that is the emulator, not the page.
- Not tested in Firefox or Safari.

## Checking a build

`node scripts/web/smoke.js target/web/counter` serves a build, clicks it in
headless Chromium and fails on a blank canvas, a click that changes
nothing, or a console error. The release check runs it.
