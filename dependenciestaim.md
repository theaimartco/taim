# TAIM Iced-Origin Dependency Ledger

Last audited: 2026-09-19

## Non-negotiable rule

TAIM ka final target **0% Iced runtime/code dependency** hai. Aage se koi bhi
dependency, fork, patch, container image, generated asset, ya transitive crate
agar `iced-rs`, `iced_*`, ya Iced-owned source se aati hai, to use code change ke
saath isi file me record karna mandatory hai.

Entry me source URL, pinned version/revision, use-site, reason, TAIM replacement,
performance hypothesis, removal gate, aur status hona chahiye. Sirf rename ya
wrapper banana replacement nahi maana jayega; implementation aur hot path TAIM
owned hone chahiye.

Status meanings:

- **ACTIVE BLOCKER**: abhi build/runtime me Iced-owned dependency present hai.
- **REMOVED**: active dependency se hata di gayi hai.
- **HISTORICAL**: repository history me thi, current dependency graph me nahi hai.
- **SOURCE LINEAGE**: vendored/inherited source hai, Cargo dependency nahi.
- **ASSET**: Iced-origin visual/font/generated asset hai.

## Current snapshot

| Category | Count | State |
| --- | ---: | --- |
| Active direct `iced-rs` Git dependencies | 0 | none |
| Active Iced crates in the resolved Cargo graph | 0 | none |
| Active Iced-owned build images/assets | 0 | inherited images and documentation artwork removed/replaced |
| Removed direct `iced-rs` Git dependencies in current TAIM work | 2 | `winit` and `cryoglyph` replaced |
| Historical Iced-owned build/runtime dependencies | 2 groups | `glutin` and cross-build images |
| Source lineage baseline | 1 | `iced-rs/iced` at `56b706fc` |
| Normalized token independence | 97.39% | 5,152 estimated inherited LOC remain across 197,679 audited LOC; **not** 100% source ownership |

### Latest source-ownership checkpoint (2026-09-19)

- **Active Iced dependency/build/asset count:** zero in manifests, resolved Cargo
  graph, and current build infrastructure. This is separate from source lineage.
- **Measured source lineage:** 97.39% normalized token independence; 5,152
  estimated inherited-equivalent LOC remain. The audit still finds 68
  byte-identical files, 77 token-identical files, and 32 high-similarity files.
  Three disguised-copy findings are Cargo manifests. These numbers do not
  establish legal ownership, and required attribution stays in place.
- **TAIM-owned hot-path changes:** keyed/unkeyed geometry cache entries have
  collision-free namespaces; window effective-size calculation no longer
  clones its icon; viewport scale changes retain the physical projection;
  screenshot conversion uses one oversized triangle; synchronous image load
  uses one resident-map probe; text-input edits retain grapheme-index capacity
  and word movement searches that index instead of recounting graphemes;
  damage ordering and pane drag thresholds compare squared distances; text
  highlights reject invalid UTF-8 spans before layout.
- **Dependency discovery:** no new Iced-owned dependency arrived. `smallvec`,
  used by the retained mouse-area trigger storage in this migration, was
  already a neutral workspace dependency; it is not an Iced source.
- **Removal gate:** still open for inherited source. The goal is not completed
  by a green build, a rename, or a percentage rounded to 100.
- **Verification:** `cargo fmt --all` completed;
  `cargo check --workspace --all-targets --all-features --offline` and
  `cargo test --workspace --all-features --offline` pass. Manifest/lockfile and
  resolved-Cargo-tree scans find no active Iced package. Comparative
  Iced-versus-TAIM performance and visual GPU measurements are not yet proven.

## Dependency inventory

### 1. `iced-rs/cryoglyph`

- **Status:** REMOVED in the current working tree
- **Source:** `https://github.com/iced-rs/cryoglyph.git`
- **Pinned revision:** `53ba3e879539d19ed8162942126a977ec896cc3b`
- **Resolved version:** `0.1.0`
- **Declared in:** `Cargo.toml`; Git source allowed in `deny.toml`
- **Use-site:** GPU text rendering in `wgpu/src/text.rs`
- **Why it exists:** glyph preparation, atlas/cache management, text rendering,
  viewport handling, and staging-belt integration for the WGPU renderer.
- **TAIM replacement:** local `taim_text_gpu`, compatible with the repository's
  current `wgpu 29` and `cosmic-text 0.19` stack.
- **Implemented performance design:** one compact instance-buffer upload per text
  layer, exact `textureLoad` sampling without sampler state, separate 1-byte mask
  and RGBA color planes, geometric growth capped at 2048x2048, warm-glyph
  retention, and inactive-LRU eviction under pressure. Text areas stream directly
  into the batch without a temporary ownership `Vec`, and a retained Swash
  raster context is reused across layers and frames.
- **Storage/allocator design:** local `taim_atlas` is shared by image and text
  paths. Text uses its zero-padding mode because exact texel loads cannot bleed;
  this removes Cryoglyph's `etagere`/`lru` dependency subtree and avoids
  reintroducing the heap-heavy `guillotiere` allocator.
- **Removal gate:** dependency, Git source, and `deny.toml` exception are removed.
  Correctness and performance verification remains scheduled for the final test
  stage.

Important: available neutral `glyphon` releases do not currently line up cleanly
with both TAIM's WGPU and cosmic-text versions. A blind package swap is therefore
not accepted as the final solution.

### TAIM-owned replacements created during removal

| Crate | Role | Ownership/performance property |
| --- | --- | --- |
| `taim_text_gpu` | GPU glyph raster cache, clipping, batching, and draw pipeline | Single upload per layer, sampler-free exact texel reads, bounded dual-plane storage, retained raster context. |
| `taim_atlas` | Shared image/text atlas allocation | Generational cache-local skyline, recyclable/coalesced free rectangles, retained scratch buffers, optional zero padding for exact texel consumers. |

### TAIM image pipeline migration

- **Status:** active source-lineage replacement; no new Iced or external package
  introduced.
- **Use-site:** `wgpu/src/image/cache.rs` and the TAIM-owned
  `wgpu/src/image/cache/worker.rs`.
- **Implemented performance design:** the UI/render thread uses bounded
  `try_send` dispatch instead of blocking on a 1,000-job queue; duplicate work
  is coalesced by image ID; one inline callback slot avoids a heap allocation in
  the common case; stale worker results are rejected with monotonic tickets;
  small images upload inline while large decode/upload work stays off the UI
  thread; shutdown cannot block trying to enqueue a stop command. Ordered draw
  groups preserve transparent UI overlap while merging only consecutive
  atlas/sampler-compatible images. Rotation sine/cosine is computed once per
  image instead of six times in the vertex shader.
- **Latest queue hardening:** pending decode/upload work now uses an explicit
  FIFO beside the deduplication map. Dispatch cost is proportional to work
  admitted by the bounded worker instead of rescanning every in-flight request
  after each completion. Queue pressure stops at the first failed probe,
  callbacks coalesce onto one monotonic ticket, and stale FIFO records are
  removed during bounded frame trimming.
- **Storage design:** at most eight decode/upload jobs are admitted to the worker
  queue, pixel payloads use cheap shared `Bytes` clones, and completed, failed,
  or off-screen callback-free requests leave the pending map. GPU atlas growth
  is geometric instead of per-layer, live layers copy in contiguous runs, large
  allocations roll back transactionally, and every atlas has a 256 MiB ceiling.
  Frame epochs now evict unused raster/SVG entries even when no new entry was
  inserted. Large SVG resize rasters reuse adaptive 4/8-pixel size buckets;
  icons up to 128px remain exact.
- **Dependency note:** this pass introduced no external dependency. `smallvec`
  was already a neutral crates.io workspace dependency and is reused only for
  the common one-callback inline case; it is not Iced-owned.
- **Current ownership measurement:** `image/cache.rs` is now a 107-line facade
  at 27.95% normalized similarity. Its FIFO request scheduler, raster
  allocation/upload implementation, and worker are new 0%-similarity modules.
  `image/atlas.rs` is 9.52%, `image/mod.rs` is 2.35%, `image/raster.rs` is
  66.32%, and `image/vector.rs` is 27.23%; the ordered batcher, pipeline,
  packing, storage, and upload modules remain 0%.
- **Verification state:** native and `wasm32-unknown-unknown` WGPU compile gates
  pass, along with the full `taim --all-features` compile gate. Runtime,
  visual-fidelity, and performance tests remain deferred to the agreed final
  test stage.

### TAIM triangle/MSAA pipeline migration

- **Status:** active source-lineage replacement; no new external dependency
  introduced.
- **Use-site:** `wgpu/src/triangle.rs`, `wgpu/src/triangle/*`, and
  `wgpu/src/triangle/msaa/*`.
- **Implemented performance design:** retained meshes upload vertices and indices
  only when geometry changes; transform-only changes update the small uniform
  stream. Index, solid-vertex, gradient-vertex, and uniform slices are grouped
  into bounded staging writes instead of mapping once per mesh. Draw ranges are
  precomputed, empty draws are skipped, and uniform scratch storage persists
  across frames.
- **MSAA storage/lifecycle:** render targets are renderer-local instead of shared
  through a cloned pipeline, eliminating cross-window size contention. Targets
  grow geometrically and use a 120-frame underuse hysteresis before shrinking,
  reducing GPU texture churn during incremental resizes while still reclaiming
  sustained excess capacity.
- **Current ownership measurement:** the thin `triangle.rs` facade is 1.36%
  similar and `triangle/msaa.rs` is 1.10%; the batch, draw, pipeline, storage,
  uniforms, MSAA pipeline, state, and target implementation modules are all 0%.
- **Verification state:** native and `wasm32-unknown-unknown` compile gates pass.
  Runtime fidelity and performance benchmarks remain deferred to the final test
  stage.

### TAIM geometry pipeline migration

- **Status:** active source-lineage replacement; no new external dependency
  introduced.
- **Use-site:** `wgpu/src/geometry.rs` and `wgpu/src/geometry/*`.
- **Implemented performance design:** fill/stroke tessellation now uses concrete
  Lyon builders instead of allocating a boxed dynamic geometry builder per draw.
  Style buffers and transform stacks retain small common-case capacity, while
  dash normalization keeps its usual data inline. Positive axis-aligned
  rectangles retain a direct tessellation fast path; rotated and dashed
  rectangles use the correctness-preserving transformed-path route.
- **UI/correctness design:** tessellation failures and unbalanced transform pops
  are logged and skipped instead of crashing the UI. Rotated rectangle geometry,
  dashed rectangle strokes, scaled cached-text width, and nonuniformly scaled
  image corner radii are handled explicitly. Identical cached mesh/image content
  is retained without advancing the GPU upload version.
- **Current ownership measurement:** the thin `geometry.rs` facade is 0.82%
  similar; the buffers, cache, dash, frame, and transform implementation modules
  are all new files measured at 0% similarity.
- **Verification state:** native full-feature and `wasm32-unknown-unknown` WebGL
  compile gates pass. Runtime, visual-fidelity, and performance tests remain
  deferred to the agreed final test stage.

### TAIM text orchestration migration

- **Status:** active source-lineage replacement on top of the local
  `taim_text_gpu`; no new external dependency introduced.
- **Use-site:** `wgpu/src/text.rs`, `wgpu/src/text/*`, and
  `taim-text-gpu/src/lib.rs`.
- **Implemented performance design:** identical cached text no longer advances
  its content version or rebuilds its GPU instance stream. Retained signatures
  include transformed clipping bounds, preventing stale clips while preserving
  true steady-state reuse. Renderer/cache state is stored per text-pipeline
  identity, so switching SDR/HDR target formats reuses compatible state instead
  of drawing an old-format pipeline or rebuilding every format on each switch.
- **Atlas/UI correctness:** retained atlas membership changes start a coherent
  allocation epoch. Existing renderer glyphs are explicitly pinned before
  recycling is allowed, preventing a later layer from replacing texels still
  referenced by an earlier retained layer. Failed layers keep their renderer
  slot, so subsequent text cannot shift into the wrong z-order; failed retained
  uploads remain retryable instead of being marked successfully prepared.
- **Storage design:** dead per-format states disappear through weak pipeline
  lifetimes, text groups disappear when their uploads are gone, and the shared
  shaping/raster caches remain reusable across active layers.
- **Current ownership measurement:** `wgpu/src/text.rs` fell from 82.53% to
  5.88% similarity; its batch, cache, pipeline, preparation, state, and storage
  modules are all new files measured at 0%. `taim-text-gpu` remains 0%.
- **Verification state:** full native, no-default WGPU, and
  `wasm32-unknown-unknown` WebGL compile gates pass. Runtime/visual and benchmark
  testing remains deferred to the agreed final test stage.

### TAIM CPU text layout and editor migration

- **Status:** active source-lineage replacement; no dependency introduced.
- **Use-site:** `graphics/src/text.rs`, `graphics/src/text/paragraph.rs`,
  `graphics/src/text/paragraph/*`, `graphics/src/text/editor.rs`, and
  `graphics/src/text/editor/*`.
- **Paragraph performance/UI:** steady-state comparison reads an atomic font
  generation instead of taking the global font-system lock, and an unchanged
  resize returns before copy-on-write or reshaping. Plain and attributed text
  share one construction pipeline; rich paragraphs now apply ellipsis, resize
  refreshes its height limit, and poisoned font locks recover safely.
- **Paragraph correctness/storage:** span bounds use actual glyph rectangles
  across every visual run instead of assuming contiguous metadata and adding
  widths blindly. Shared paragraph/weak-reference equality has an O(1) pointer
  path, while independently built paragraphs compare content and complete
  layout state so stale text cannot be treated as unchanged.
- **Editor performance/storage:** unchanged editor configuration returns before
  global font locking, `Arc` teardown/reallocation, and shaping. Changed frames
  perform one editor-aware shape pass instead of shaping the buffer twice;
  scaled metrics are compared in the same coordinate space and identical font
  attributes are built once per update. Selection cache locks recover from
  poisoning and caret geometry avoids index underflow on unusual wrapped runs.
- **Current ownership measurement:** `paragraph.rs` is down from 100% to 45.98%
  similarity and `editor.rs` from 100% to 14.17%. Their build, query, state,
  selection, and update modules are new files measured at 0%. The former shared
  `graphics/src/text.rs` seam is now an 8.29% facade over TAIM-owned conversion,
  font-system, layout, and primitive modules.
- **Verification state:** graphics and dual-renderer all-feature compile gates
  pass. Text interaction, visual fidelity, and performance tests remain deferred
  to the agreed final test stage.

### TAIM UI interaction and shell migration

- **Status:** active source-lineage replacement; no new dependency introduced.
- **Use-site:** keyboard keys, combo/pick lists, widget helpers, Markdown,
  scrollables, text editor/input, pane grid, Winit shell/conversions, and WGPU
  renderer orchestration.
- **UI/state correctness:** transient hover/focus/interaction caches in text
  editor, text input, pick list, and pane grid now live in persistent widget-tree
  state, so view reconstruction cannot silently lose redraw or cursor state.
  Markdown parsing/model/view, scroll configuration/state/style, pane-grid
  interactions, Winit lifecycle/actions, and platform conversions have explicit
  TAIM-owned module boundaries instead of shared inherited monoliths.
- **CPU/allocation performance:** combo filtering reuses event scratch storage
  and avoids per-token lowercase allocations; Markdown private reference maps
  use fast deterministic hashing; scroll notifications avoid duplicate derived
  offset calculations; resize/input event vectors retain capacity across Winit
  turns; text-editor export uses one editor borrow without cloning every line.
  Software retained-layer selection is `O(n log n)` instead of quadratic.
  Runtime task batching keeps zero/one-stream batches direct and creates a
  `SelectAll` merger only for genuinely concurrent streams; work-unit arithmetic
  saturates instead of wrapping. Task output mapping no longer allocates and
  boxes a one-item stream for every message; empty batches remain truly empty,
  collection preallocates from the stream lower bound, and abort-on-drop handles
  no longer allocate a replacement `Arc` during destruction. `Element::map`
  stores its mapper in the element's existing allocation instead of a second box.
- **Storage performance:** text input values moved from one heap `String` per
  grapheme to one contiguous UTF-8 buffer with a compact grapheme-boundary
  index. Normal layout/hit-testing now borrows this buffer instead of rebuilding
  the full string. Borrowed fonts are deduplicated by identity without copying.
  Pane-grid split/close transfers subtree ownership instead of deep-cloning it,
  while layout counts use one contiguous scratch arena instead of two heap boxes
  per split.
- **UI/render hot paths:** radio, rich text, checkbox, sliders, text input/editor,
  pick lists, and pane-grid interactions retain hover/pressed/status state in the
  widget tree across view reconstruction. Image Viewer caches natural dimensions
  once per layout, applies its padding, culls offscreen draws, clamps pan to the
  actual hidden half-extent, and stops capturing no-op wheel events. Table metrics
  reuse capacity, mixed fixed/fluid rows use per-row factors, empty tables avoid
  division by zero, row construction saves one clone per populated row, and
  offscreen cells skip draw work. QR geometry merges horizontal dark-cell runs
  and retains a bounded two-style cache, avoiding one quad per dark cell and
  cross-widget cache thrashing. Float overlays support scale-down correctly and
  cache their inverse transform plus transformed viewport instead of recomputing
  matrix inversion throughout event, draw, and nested-overlay traversal.
  Tooltip deadlines are stored directly, no-op immediate redraw scheduling is
  removed, and the common single-overlay path avoids allocating a group vector.
- **Latest layered/component path:** Stack skips offscreen layers using each
  child's overflow-aware paint bounds before opening renderer layers. Deprecated
  lazy components reuse bounded event buffers across both base and overlay
  dispatch instead of allocating on every interaction. Pane-grid sizing rejects
  non-finite/negative geometry, computes large minimum extents without `f32`
  overflow, hashes exact normalized split ratios, and culls offscreen pane bodies.
  Legacy program decorators preserve preset lists, while default title creation
  uses one pre-sized output buffer instead of one allocation per word.
- **Latest container/menu path:** Grid cache identity now includes column and
  sizing strategy, visible-row lookup is O(1), invalid geometry is normalized,
  and void children never enter layout. Row/Column cache keys include padding
  and alignment. Keyed reconciliation reuses parallel buffers instead of three
  temporary vectors. Dropdown labels are retained shaped paragraphs, removing
  per-visible-option String allocation and shaping from every draw; its prior
  tag/state mismatch and stale hover-on-exit behavior are fixed.
- **Latest storage/cache path:** the generic retained-resource cache enforces a
  hard byte ceiling, rejects individually oversized entries, purges stale
  generations without a temporary key vector, and performs one ordered
  eviction pass instead of repeated O(n²) LRU scans. Tree layout caches use
  normalized limit keys, so NaN payloads and signed zero cannot destabilize
  cache identity.
- **Image/UI storage:** encoded and decoded raster handles use stable
  content-addressed IDs, allowing repeated identical handles to reuse decode and
  GPU cache entries instead of forcing fresh uploads. Raster and SVG widgets
  retain natural dimensions from layout, eliminating renderer re-measurement in
  every draw; transparent, zero-size, and offscreen content is culled early.
  Raster crop extents use checked remaining dimensions, while opacity and scale
  inputs are normalized before they can introduce invalid draw geometry. SVG
  hover state now lives in the retained tree instead of a reconstructed widget.
  File images use one buffered open for EXIF and decode instead of opening the
  same file twice. Encoded dimensions are capped at 16,384 per axis and decoded
  RGBA storage at 256 MiB; raw handles are checked before their shared pixel
  storage is cloned, so malformed input has a bounded error path.
- **Core/UI correctness:** the color parser now decodes ASCII nibbles directly
  without substring/radix overhead or non-ASCII boundary panics, and endpoint
  color mixes avoid gamma conversion. Layout length fingerprints encode every
  bounds, sizing, and fluid-constraint variant, preventing deterministic cache
  collisions between different layouts. Mouse-area routing computes hover once
  and captures every handled release so parent controls cannot process the same
  input accidentally.
- **Dependency note:** these migrations added no package. The current manifest
  and lockfile scan contains zero active `iced`, `iced_*`, `iced-rs` Git, or
  Iced package entries.
- **Current ownership measurement:** the hardened whole-workspace audit reports
  95.24% normalized token independence with 9,331 estimated inherited LOC
  across 196,029 audited LOC.
  Three remaining disguised-copy findings are Cargo manifests, not runtime
  source; they remain explicitly visible until their package metadata is
  genuinely redesigned.
- **Verification state:** affected all-feature crate compile gates pass. Runtime,
  visual-fidelity, unit, and benchmark testing remains deferred to the agreed
  final test stage.

### TAIM WGPU layer batching migration

- **Status:** active source-lineage replacement; no new dependency introduced.
- **Use-site:** `wgpu/src/layer.rs` and `wgpu/src/layer/*`.
- **Implemented performance/storage design:** transient mesh and text group
  vectors are recycled across resets instead of restarting allocation after
  every flush. Each pool is bounded to eight groups and rejects a group above
  256 KiB, while retained-byte accounting includes both outer pools and inner
  capacities so the display-list budget remains honest.
- **UI transform fix:** the framework now measures transformed axis magnitudes
  instead of reading one matrix diagonal. Borders, shadows, and corner radii no
  longer collapse toward zero under rotation; nonuniform radius scaling uses
  the smaller axis. The result is computed once per quad instead of repeatedly.
- **Current ownership measurement:** the thin `wgpu/src/layer.rs` facade is
  8.04% similar; draw, lifecycle, and memory modules are new files measured at
  0% similarity.
- **Verification state:** full native, WGPU no-default, and WebAssembly WebGL
  compile gates pass. Runtime and performance tests remain deferred to the final
  test stage.

### TAIM software-renderer hot-path migration

- **Status:** active source-lineage replacement; no new dependency introduced.
- **Use-site:** `tiny_skia/src/engine.rs` and `tiny_skia/src/engine/*`.
- **Shadow performance/storage:** repeated software shadows are cached by their
  physical shape, blur, radii, and working-space color. The cache has a hard
  16 MiB byte budget, LRU pressure eviction, and 120-frame idle expiry; a single
  shadow larger than the ceiling is skipped instead of risking an unbounded UI
  allocation. Cached bitmaps are position-independent and can be reused under
  translation, rotation, and scale.
- **Border/UI design:** rounded borders render as one even-odd outer/inner ring.
  The former small-radius fallback allocated a full temporary pixmap and mask
  for every draw; that allocation path is removed. Invalid gradients fall back
  safely instead of panicking. Cached/raw/editor text and raster/vector images
  now respect the intersection of local and layer clip bounds, and off-screen
  editor/image work is culled before rasterization.
- **Transform/color performance:** the Tiny-Skia bridge now forwards the full
  2D affine matrix instead of discarding rotation, shear, and nonuniform scale.
  Premultiplied SVG color conversion uses a bounded 64 KiB lookup table instead
  of three transfer-function `powf` calls per pixel.
- **Clip/culling performance:** clip tracking resets once at frame entry, reuses
  identical rectangles without touching memory, and clears only the previous
  span when a nested clip changes instead of zeroing the entire surface for
  every clipped item. Fully transparent text/images skip shaping and decode;
  rotated image culling uses rotated bounds, raw-text fallback dimensions avoid
  double scaling, and visible shadows are retained even when their source quad
  itself is off-screen.
- **Latest frame orchestration:** scale transforms and the source background
  paint are built once per frame instead of per primitive/damage rectangle;
  invalid damage rectangles are skipped without panicking. Retained-frame
  containment uses a sorted suffix-minimum pass (`O(n log n)`) instead of an
  all-pairs scan, and renderer responsibilities are separated into draw,
  retained, text, geometry, asset, and headless modules.
- **Dependency removal:** the direct `kurbo` dependency was removed from
  `taim_tiny_skia`; quarter-circle corners use a fixed TAIM cubic construction.
  `kurbo` remains only transitively through the optional neutral `resvg` stack.
- **Current ownership measurement:** the `tiny_skia/src/engine.rs` orchestrator
  is down to 10.78% similarity and the 75-line renderer facade is 23.97%.
  Its TAIM backend, clip, image, path, primitive, quad, text, bounded-shadow,
  and color modules are new files measured at 0%. The engine no longer appears
  in the audit's disguised-copy list.
- **Verification state:** Tiny-Skia all-feature and no-default compile gates and
  the full native TAIM gate pass. Runtime fidelity and benchmarks remain
  deferred to the final test stage.

### TAIM software geometry migration

- **Status:** active source-lineage replacement; no new dependency introduced.
- **Use-site:** `tiny_skia/src/geometry.rs` and `tiny_skia/src/geometry/*`.
- **Implemented performance design:** identical primitive, image, and text cache
  arrays retain their previous `Arc` storage. Axis-aligned rectangle fills use a
  direct Tiny-Skia path instead of passing through the general path-conversion
  route. Frame vectors and transform stacks keep small common-case capacities,
  child-frame storage is moved into its parent, and opacity-one cached geometry
  replays shared primitives and text without clone/map/collect work.
- **UI/correctness design:** full affine transforms are preserved, scaled cached
  text width and clip bounds are explicit, nonuniform image corner radii use the
  smaller transformed axis, and invalid paths/gradients or stack underflow are
  skipped with diagnostics instead of crashing the UI.
- **Current ownership measurement:** the thin `tiny_skia/src/geometry.rs` facade
  is 1.01% similar; cache, conversion, and frame implementation modules are new
  files measured at 0% similarity.
- **Verification state:** Tiny-Skia all-feature compile gate passes. Runtime
  fidelity and performance benchmarks remain deferred to the final test stage.

### TAIM software text migration

- **Status:** active source-lineage replacement; no new dependency introduced.
- **Use-site:** `tiny_skia/src/text.rs` and `tiny_skia/src/text/*`.
- **Implemented performance/storage design:** one retained Swash raster context
  is reused instead of allocating it for every text draw. Rasterized glyphs have
  a hard 16 MiB byte ceiling, inactive-LRU pressure eviction, and 120-frame idle
  expiry. Color glyphs are keyed independently of requested text RGB, preventing
  duplicate emoji bitmaps, while mask glyphs retain correctly colored coverage.
- **UI design:** monochrome, color, and subpixel masks render through the same
  clipped raster loop. Invalid scales and malformed glyph pixels are skipped
  safely. Font reload clears shaping, raster, and Swash state coherently, and a
  poisoned global font lock can recover without taking down the UI.
- **Current ownership measurement:** the thin `tiny_skia/src/text.rs` facade is
  0.99% similar; draw, glyph-cache, and pipeline implementation modules are new
  files measured at 0% similarity.
- **Verification state:** Tiny-Skia all-feature and no-default compile gates pass.
  Runtime text fidelity and benchmarks remain deferred to the final test stage.

### TAIM software display-layer migration

- **Status:** active source-lineage replacement; no new dependency introduced.
- **Use-site:** `tiny_skia/src/layer.rs`, `tiny_skia/src/layer/*`, and the software
  renderer orchestration in `tiny_skia/src/lib.rs`.
- **Implemented performance design:** primitives, images, and text use explicit
  one-value, owned-group, and shared-group display batches. Opacity-one cached
  geometry images now replay their `Arc<[Image]>` directly instead of cloning
  every image every frame. Empty groups are not recorded and all group types are
  culled once against their transformed clip before per-item rendering.
- **Damage/storage design:** damage comparison writes into one retained-capacity
  output vector instead of constructing a temporary `Vec` for every item's
  bounds. Shared batches compare by `Arc` identity, strokes include their width,
  and transformed quad shadows include their actual old/new extents. Reset keeps
  normal stream capacity but releases any outer stream above 256 KiB so one
  pathological frame cannot pin unbounded display-list storage.
- **UI transform design:** quads, direct text, raster images, and SVGs preserve
  the complete display-stack transformation until render time. Rotation, scale,
  shadow geometry, font sizing, and clipping therefore remain coherent instead
  of partially baking only bounds.
- **Current ownership measurement:** the thin `tiny_skia/src/layer.rs` facade is
  7.75% similar; damage, item, lifecycle, and recording modules are new files
  measured at 0% similarity.
- **Verification state:** Tiny-Skia all-feature/no-default and full native TAIM
  compile gates pass. Runtime visuals and performance benchmarks remain deferred
  to the agreed final test stage.

### TAIM software raster and vector migration

- **Status:** active source-lineage replacement; no new dependency introduced.
- **Use-site:** `tiny_skia/src/raster.rs`, `tiny_skia/src/raster/*`,
  `tiny_skia/src/vector.rs`, `tiny_skia/src/vector/*`, and vector composition in
  `tiny_skia/src/engine.rs`.
- **Raster performance/storage:** decoded raster pixels now live in a checked
  128 MiB cache with inactive-LRU pressure eviction and 120-frame idle expiry.
  Pixel counts and byte sizes use checked arithmetic, allocation uses
  `try_reserve_exact`, oversized entries return `OutOfMemory`, and failed image
  handles are negatively cached so corrupt input is not decoded every frame.
- **Vector performance/storage:** parsed SVG trees are capped at 256 entries;
  rendered vectors use a 64 MiB byte-bounded LRU with 120-frame expiry and a
  bounded failure cache. Icons through 128 px keep exact dimensions, medium and
  large results use 4/8-pixel reuse buckets, and a single raster is capped at
  eight million pixels before being proportionally downscaled.
- **UI transform/color design:** target SVG resolution comes from transformed
  axis magnitudes instead of matrix diagonals, while final composition preserves
  the full affine transform and rotates about the correct local center. SVG path
  input supports binary/gzip data consistently. Color-filter alpha now combines
  with vector coverage instead of being discarded.
- **Current ownership measurement:** the thin `raster.rs` and `vector.rs`
  facades are 1.78% and 1.47% similar respectively; their cache and pipeline
  implementation modules are new files measured at 0% similarity.
- **Verification state:** Tiny-Skia all-feature/no-default and full native TAIM
  compile gates pass. Decode/raster fidelity and performance benchmarks remain
  deferred to the final test stage.

### TAIM software compositor migration

- **Status:** active source-lineage replacement; no new dependency introduced.
- **Use-site:** `tiny_skia/src/window/compositor.rs` and
  `tiny_skia/src/window/compositor/*`.
- **Frame/presentation performance:** buffer-age history stores shared `Arc`
  frame snapshots, so an unchanged present no longer deep-clones nested layer,
  text, image, and geometry storage. History has a hard eight-frame ceiling and
  damaged working-space pixels alone are encoded for presentation.
- **Storage/resilience:** resize validates nonzero dimensions and checked pixel
  counts before mutating the surface; clip-mask and working-buffer allocation
  failures are handled without a UI panic. Screenshot buffers also use checked
  allocation, return safely for zero/oversized surfaces, and issue renderer
  damage in logical rather than physical coordinates.
- **Current ownership measurement:** `tiny_skia/src/window/compositor.rs` is down
  to 45.28% similarity; the state, present, and screenshot implementation
  modules are new files measured at 0% similarity.
- **Verification state:** Tiny-Skia all-feature/no-default and full native TAIM
  compile gates pass. Window-system runtime behavior and frame-time benchmarking
  remain deferred to the final test stage.

### TAIM core rectangle hot-path migration

- **Status:** active source-lineage replacement; no new dependency introduced.
- **Use-site:** `core/src/rectangle.rs` and `core/src/rectangle/*`; shared by
  layout, hit testing, clipping, renderer culling, damage, and gradients.
- **Implemented performance design:** positive-overlap checks use a direct
  boolean edge route instead of constructing an intersection; vertex recovery
  computes one `sin_cos` pair; common center/size/area queries are inline; and
  query, transform, and generic arithmetic responsibilities are independently
  owned modules.
- **UI/correctness design:** the unbounded clip sentinel handles its
  `-infinity + infinity` far edge without producing `NaN`, so containment and
  intersections stay valid. Diagonal gradient chords now reach the actual
  rectangle boundary instead of stopping inside it, while stroke/damage users
  receive stable positive-overlap semantics.
- **Current ownership measurement:** the `core/src/rectangle.rs` facade is 9.01%
  similar; bounds, transform, and operator implementation modules are new files
  measured at 0%. This migration does not appear in the disguised-copy audit.
- **Verification state:** core-only and full native all-feature compile gates
  pass. Behavioral and performance tests remain deferred to the final stage.

### TAIM core affine-transform migration

- **Status:** active source-lineage replacement; no new dependency introduced.
- **Use-site:** `core/src/transformation.rs` and
  `core/src/transformation/ops.rs`; consumed by both renderers, layout, damage,
  retained layers, input coordinates, and window scaling.
- **Implemented performance design:** point, vector, and size transforms use
  direct 2D matrix components instead of constructing and multiplying generic
  four-dimensional vectors. Axis magnitudes remain rotation-safe, and the
  small public transform API is separated from TAIM-owned arithmetic operators.
- **UI/correctness design:** transformed rectangles now calculate the
  conservative envelope of all four affine corners. Rotation and shear can no
  longer produce negative/undersized clip or damage rectangles; non-finite clip
  sentinels stay explicitly unbounded instead of leaking `NaN` coordinates.
- **Current ownership measurement:** `core/src/transformation.rs` is down to
  47.19% similarity and the operator implementation is a new file measured at
  0%; neither appears in the disguised-copy audit.
- **Verification state:** core-only, full native all-feature, and WebAssembly
  WebGL compile gates pass. Visual and performance tests remain deferred to the
  final stage.

### TAIM renderer selection and recovery migration

- **Status:** active source-lineage replacement; no new dependency introduced.
- **Use-site:** `renderer/src/lib.rs`, `renderer/src/selection.rs`, and
  `renderer/src/route.rs`.
- **Performance design:** single-backend builds resolve directly to the enabled
  renderer and compositor, so they pay no recovery-route branch. Dual-backend
  builds use a fixed enum selected during initialization with no trait object or
  heap allocation, while preserving a software recovery path if GPU startup
  fails.
- **Dependency/storage cleanup:** the renderer crate's unused direct
  `thiserror` dependency is removed. Recovery error storage is allocated only
  when both backend initializations actually fail.
- **Current ownership measurement:** the public selector facade fell from
  84.23% to 28.26% normalized similarity and no longer appears in the
  disguised-copy audit. The 915-line route implementation and the compile-time
  feature matrix are new files measured at 0%.
- **Verification state:** no-backend debug, GPU-only, software-only, and dual
  all-feature renderer compile gates pass. Runtime visuals and performance
  benchmarks remain deferred to the agreed final test stage.

### TAIM completion, hot-path, and verification pass

- **Status:** active dependency/build/asset surface is 100% free of Iced-owned
  packages and infrastructure. Source-lineage independence is separately and
  honestly measured at 97.39%; this entry does not relabel inherited source as
  original TAIM work.
- **UI/runtime performance:** unchanged lazy dependencies bypass reconciliation
  and hashing. Dropdown shaping is bounded to the visible window plus three
  screens of overscan instead of shaping the full option set. Component event
  buffers are retained, pane content/title layouts are cached, stale pane state
  is released, adjacency lookup is allocation-free, and offscreen component,
  pane, mesh, and retained-layer work is culled before expensive preparation.
  Focus traversal stops descending once the requested transition is complete,
  rotated size bounds reuse one `sin_cos` pair, and middle gradient-stop
  insertion preserves every existing stop instead of overwriting the next one.
- **Renderer performance/storage:** layer extraction is single-pass with nested
  clip preservation. Font registration sorts and deduplicates in one retained
  vector. Custom primitive pipelines are keyed by target format. Cached mesh
  opacity is a small GPU uniform update inside the existing 256-byte dynamic
  stride; opacity no longer clones every vertex or invents a new cache identity.
  Reusing the same `Arc` keeps mesh versions stable, and transparent/invalid
  meshes are rejected before upload. Quad batches retain an O(1) transmissive
  flag and merge only compatible boundary runs, reducing backdrop scans and
  pipeline switches without changing painter order. GPU staging writes reject
  overflow, device-limit, unaligned, empty, and out-of-range operations before
  reaching WGPU. Screenshot format conversion is a lazy retained pipeline: its
  exact `textureLoad` pass removes the prior sampler, ratio uniform, extra bind
  group, per-capture pipeline compilation, and incorrect copy-time blending.
  Default opaque quad fragments now bypass analytic-normal work entirely;
  gradient lookup exits as soon as it finds the active stop interval and uses
  non-transcendental deterministic dithering instead of a sine per fragment.
  Empty solid/gradient instance uploads and draw ranges return before touching
  staging belts, bind groups, or render pipelines. Custom shader widgets reject
  fully clipped draw and pointer work before invoking application shader code.
- **Core/UI storage and correctness:** traversal-critical widget trees keep
  optional layout/paint/semantics metadata behind a lazy cold-cache pointer,
  so stateless nodes pay neither the inline size nor an allocation. Responsive
  widgets reuse their already-diffed child on duplicate same-size layout passes.
  Timed applications now expose the actual state type as their default visible
  name. Dynamic font-family interning is bounded. IME surrounding selections
  are normalized to UTF-8 boundaries, while secure fields disclose neither
  content nor length. Clipboard merging preserves an existing write without a
  take/drop cycle when the incoming batch has no write. Empty-space widgets are
  copyable and their common horizontal/vertical constructors are `const`.
  Screenshot crops validate coordinate arithmetic and backing-buffer length,
  preallocate exactly, and reuse shared full-frame bytes without copying.
  Window settings reject invalid dimensions, resolve conflicting bounds, and
  clamp the initial size before platform-window creation. Variable-font axis
  updates use ordered binary insertion instead of a linear search plus full
  resort, and transparent/non-finite canvas text exits before shaping or
  allocating geometry.
- **Core rendering dispatch:** identity transforms, zero translations, and
  unit opacity bypass redundant renderer push/pop calls. Overlay groups skip
  sorting when already ordered and use total float ordering when sorting is
  required. Cursor transforms share one inlined position mapper, and keyboard
  modifier queries are const/inlined bit tests.
- **Shell/storage:** Winit proxy wake claims, acknowledgements, and released
  capacity now share one atomic `Arc`, reducing clone/refcount/storage traffic.
  The bounded worker exits cleanly when its channel closes and coalesces wakeups
  without losing released slots.
- **Correctness found by final testing:** fresh keyed columns now initialize
  their materialized-key state consistently with an empty child tree; padding
  preserves requested insets under legitimate infinite layout limits; frame
  scheduler tests retain pending work across missing feedback; catalog coverage
  recursively discovers split style modules; and Markdown examples use the
  current theme API.
- **Verification completed:** every workspace unit/integration/doc test passes
  with all features. The long benchmark report-schema gate passed in the full
  run and the remaining workspace suite was rerun after fixes. Workspace-wide
  all-feature native checks, WGPU WebAssembly WebGL checks, formatting, diff
  hygiene, semantic headless interaction, keyed/virtual-list behavior, and SVG
  XML validation all pass. These gates prove correctness and compatibility;
  they do not, by themselves, prove an across-the-board speed win over Iced.
- **Dependency result:** manifest/lock/deny scans return no `iced`, `iced_*`, or
  `iced-rs` entry, and the locked workspace dependency tree contains no active
  Iced crate. No external dependency was added by this pass. `taim_widget` now
  directly uses the already-resolved, neutral workspace `smallvec` package to
  keep the common two MouseArea handlers inline; this adds no package to the
  lock graph and replaces nine always-inline message slots with demand-sized
  storage.

### 2. `iced-rs/winit`

- **Status:** REMOVED in the current working tree
- **Former source:** `https://github.com/iced-rs/winit.git`
- **Former pinned revision:** `05b8ff17a06562f0a10bb46e6eaacbe2a95cb5ed`
- **Replacement:** neutral crates.io `winit 0.30.13` plus TAIM-owned shell/event
  integration.
- **Performance/ownership direction:** keep the platform shell thin, avoid extra
  event allocations, and let TAIM own routing, redraw scheduling, and surface
  lifecycle decisions.
- **Known compatibility seam:** the fork-only macOS `received_url` callback is no
  longer supplied by upstream Winit. If automatic URL-open delivery is required,
  it must be implemented by a TAIM-owned platform adapter instead of restoring
  the Iced fork.
- **Cross-platform clock fix:** TAIM's frame gate now uses the framework clock
  abstraction on every target, keeping Winit `WaitUntil` deadlines type-correct
  on WebAssembly without a target-specific adapter or dependency.
- **TAIM event-routing replacement:** converted window events now enter a
  persistent per-window queue instead of being appended to one global vector
  and repartitioned into a newly allocated hash map on every `AboutToWait`.
  Per-window vector capacity survives between batches, event order within a
  window is preserved, queues for closed windows are discarded after their
  last ignored event is broadcast, and retained capacity is bounded to 256
  events per window so a one-time burst cannot permanently inflate storage.
- **Frame scheduling/performance:** the TAIM-owned `FrameGate` uses the existing
  neutral `rustc-hash` dependency, mutates a tracker with one lookup per event,
  coalesces dirty bits without advancing redundant generations, and performs a
  direct O(1) render eligibility lookup instead of scanning every dirty window.
  Frame deadlines now have one owner, the TAIM scheduler; the unused parallel
  deadline in `FrameGate` and the Winit window's duplicate `redraw_at` field are
  removed. The scheduler consumes the current frame before UI evaluation and
  retains follow-up redraw/animation work across submit/feedback without a
  per-frame allocation. Closed windows unregister their frame trackers.
- **Idle/minimize behavior:** OS occlusion and zero-sized surfaces are separate
  suspension reasons, so unminimizing cannot accidentally override a still-
  occluded window. Hidden/zero-sized dirty windows are idle to the event loop,
  animation deadlines cannot cause a polling loop, and visible scheduled work
  still reaches Winit's `WaitUntil` path.
- **Input/UI correctness:** pixel wheel deltas, cursor positions, touch points,
  resize dimensions, and window positions use a guarded logical-scale
  conversion. Invalid/zero/non-finite scale factors cannot poison window state,
  file-drop paths move without cloning, and keyboard text filters every Unicode
  private-use plane while retaining an ASCII fast path.
- **TAIM event normalization:** the active mouse, touch, keyboard, IME, file,
  resize, and scale event path now lives in `winit/src/conversion/event.rs`, a
  new 0%-similarity module. Window state normalizes modifier bits once when they
  change instead of rebuilding the same bitset on every keypress. Initial,
  minimum, and maximum window dimensions apply application scaling consistently;
  invalid sizes/scales are bounded and monitor-position addition cannot overflow.
- **IME UI path:** preedit spans are assembled in fixed stack storage instead of
  allocating a temporary vector on every composition update. Invalid UTF-8
  selection ranges fall back safely, paragraph reuse includes size/font/hint
  changes, and oversized preedit bounds stay clamped to the viewport.
- **Clipboard performance/storage:** the inherited thread-per-read/write path is
  replaced by one named worker and a bounded eight-command nonblocking queue.
  Queue pressure returns immediately instead of stalling the UI; text/HTML is
  capped at 64 MiB, images at 256 MiB with exact checked RGBA dimensions, and
  file lists at 4096 entries. Callback panics cannot kill the worker. The
  neutral existing `arboard` backend remains unchanged and no package was added.
- **Proxy/backpressure path:** processed-slot notifications now use one wake
  token plus an atomic released-slot accumulator, so a saturated notification
  channel cannot lose capacity and permanently stall the bounded action worker.
  Repeated tick/redraw/relayout/reload wakeups are collapsed with atomic pending
  bits until the event loop acknowledges them.
- **Update/storage reuse:** action and output vectors are retained as per-runner
  scratch storage and drained between updates instead of allocating fresh
  vectors on every message batch. A dead per-window UI-cache map that allocated
  entries but was never read has been removed.
- **Accessibility render path:** focus-node lookup walks the semantic tree
  directly without first allocating and sorting a complete in-order node list
  on every rendered frame.
- **Dependency note:** this pass introduced no dependency. `rustc-hash` was
  already a neutral crates.io workspace dependency used throughout the Winit
  runner and runtime.
- **Viewport/state performance:** OS window aliases now use O(1) `FxHashMap`
  lookup while the ordered window table retains stable oldest/latest semantics.
  Duplicate resize/scale notifications skip viewport/projection reconstruction,
  surface generations wrap safely, and viewport logical dimensions use one
  guarded reciprocal instead of two divisions. Invalid application/platform
  scale products fall back to 1.0 instead of emitting infinite or `NaN` UI
  geometry.
- **Current ownership measurement:** `winit/src/frame_gate.rs` is a new TAIM file
  measured at 0% normalized similarity, as is the TAIM runtime frame scheduler.
  The event-normalization module is also 0%. `winit/src/window/state.rs` is down
  to 25.11%; `clipboard.rs`, `window.rs`, `conversion.rs`, `proxy.rs`, and
  `lib.rs` remain source-lineage migrations at 41.46%, 44.84%, 1.50%, 21.55%,
  and 6.20%, so this shell is improved but is not yet claimed as fully
  TAIM-owned.
- **Verification state:** native all-feature, runtime integration, proxy/frame
  gate unit tests, workspace doctests, and the relevant WebAssembly compile gate
  pass. Comparative latency benchmarking still requires an explicit Iced
  baseline before any numerical superiority claim.
- **Removal gate:** completed for the dependency itself; platform URL behavior
  remains a separately tracked TAIM feature seam.

### 3. `iced-rs/glutin`

- **Status:** HISTORICAL / REMOVED
- **Former source:** `https://github.com/iced-rs/glutin`
- **Observed pinned revisions:**
  - `da8d291486b4c9bec12487a46c119c4b1d386abf` (`glutin 0.29` era)
  - `be6793b5b3defc9452cd1c896cd315ed7442d546` (`glutin 0.27` era)
- **Current state:** no active dependency.
- **Future rule:** it must not be reintroduced from the Iced organization. Any
  required OpenGL/platform path must use a neutral upstream or TAIM-owned native
  implementation with measured benefit.

### 4. `iced-rs/iced`

- **Status:** SOURCE LINEAGE, not a current Cargo dependency
- **Recorded baseline:** commit `56b706fc`
- **Why it is listed:** TAIM began from an Iced source baseline, so removing Git
  packages alone does not achieve the 0% goal.
- **TAIM replacement target:** progressively replace inherited runtime, renderer,
  widget, shell, layout, and public facade implementations with independently
  designed TAIM code.
- **Measurement:** `python3 scripts/audit/check-independence.py`
- **Removal gate:** no shipping implementation remains Iced-derived or materially
  identical. Required license notices and historical attribution remain intact.

### 5. Iced cross-build container images

- **Status:** HISTORICAL / REMOVED
- **Historical images:**
  - `ghcr.io/iced-rs/aarch64:latest`
  - `ghcr.io/iced-rs/armv7:latest`
  - older `icedrs/iced:aarch64` and `icedrs/iced:armv7` images
- **Current replacement:** neutral `ghcr.io/cross-rs/*` images in `Cross.toml`.
- **Future target:** pin reproducible neutral images or publish minimal TAIM-owned
  toolchain images if profiling shows a build-speed or supply-chain benefit.

## Non-Cargo Iced-origin inventory

These are not dependencies in Cargo's graph, but they count toward TAIM's 0%
Iced-origin goal and must not be overlooked:

| Item | Status | TAIM action |
| --- | --- | --- |
| `graphics/fonts/Taim-Icons.ttf` glyph set | REPLACED | Generated from TAIM-owned `scripts/assets/icon_geometry.py`; current icon and codepoint tests pass. |
| `docs/logo.svg` and `docs/logo-no-shadow.svg` | REPLACED | Original TAIM speed-mark vectors now provide the documentation identity. |
| `widget/assets/iced-logo.svg` | REMOVED | No file or active reference remains. |
| inherited `docs/graphs/*.png` diagrams | REPLACED | Removed and replaced by TAIM-owned `foundations.svg`, `native.svg`, and `ecosystem.svg`. |

Attribution must remain while any derived source or asset remains, and historical
license obligations remain even after replacement.

## Mandatory workflow for every new dependency

1. Scan the owner, source URL, package name, patches, feature graph, and transitive
   dependencies before accepting it.
2. If anything is Iced-owned or Iced-derived, add a ledger entry here in the same
   change before implementation continues.
3. Record why the dependency is needed and define a TAIM-owned replacement with a
   measurable speed, memory, UI, storage, or binary-size goal.
4. Keep the replacement API TAIM-owned; do not expose an Iced-shaped public API
   merely behind renamed types.
5. Finish implementation first. Run correctness, compatibility, and benchmark
   testing at the final verification stage, as agreed for the current migration.
6. Mark an entry REMOVED only after the dependency and its exceptions disappear
   from manifests, lockfile, code paths, assets, and build infrastructure.

## Audit commands

Run these whenever a dependency is added or upgraded:

```bash
rg -n -i 'github\.com/iced-rs|git\+https://github\.com/iced-rs|ghcr\.io/iced-rs|icedrs/iced|package\s*=\s*"iced|\biced[-_][a-z]' . --glob 'Cargo.toml' --glob 'Cargo.lock' --glob 'deny.toml' --glob 'Cross.toml'
cargo tree --edges normal,build,dev | rg -i '(^| )iced[-_]'
python3 scripts/audit/check-independence.py
```

Before closing a replacement, also compare release-mode allocation count,
p50/p95/p99 frame time, peak RSS/GPU memory, storage/cache growth, binary size,
startup time, and UI/output fidelity against the dependency it replaces.

## Template for the next discovery

```text
### Package or asset name
- Status: ACTIVE BLOCKER / REMOVED / HISTORICAL / SOURCE LINEAGE / ASSET
- Source/owner:
- Version or revision:
- Declared in and use-sites:
- Why it entered TAIM:
- TAIM-owned replacement:
- Performance hypothesis and metric:
- Compatibility risks:
- Removal gate:
```
