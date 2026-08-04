# VYOM Aura GUI Competitive Audit

**Audit date:** 2026-08-01  
**Repository:** `theaimartco/vyom-aura`  
**Audited commit:** `30b5e969f3db0123721597dbbc894aca63a69af9` (`main`)  
**Primary question:** VYOM Aura GUI quality, performance, and developer workflow me Flutter aur React Native ko kaise beat kar sakta hai?

## 1. Executive verdict

VYOM Aura ke paas unusually strong low-level foundation hai: Rust ownership model, `wgpu` renderer, software fallback, incremental layout/semantics primitives, gesture arena, bounded caches, frame scheduler, design tokens, accessibility engine, profiler, and a very large automated test surface. Is codebase ko “just another unfinished UI toolkit” kehna unfair hoga.

Lekin **aaj ki state me Aura Flutter ya React Native ko GUI framework ke roop me beat nahi karta**. Current repository ek strong collection of individually tested engines hai; abhi tak ek fully integrated, visually proven, developer-friendly product nahi hai.

Main reason code volume ya missing algorithms nahi hai. Main reason hai **live-path integration and proof gap**:

- retained layers/display lists implemented hain, but live `UserInterface::draw` unhe consume nahi karta;
- GPU effect graph and budgets implemented hain, but actual render path effect graph execute nahi karta;
- VYOM Wayland backend crate implemented hai, but public `aura::application(...).run()` path abhi `aura_winit` use karta hai;
- semantics and AT-SPI pieces exist, but running application path semantic tree platform adapter ko deliver nahi karta;
- gesture router exists, but standard widgets/backend real gesture arenas start nahi karte;
- design contracts extensive hain, but visual gallery, approved component screenshots, icon pack, and end-to-end pixel goldens absent hain;
- benchmark harness headless hai, saved reports me device/commit/compiler metadata `null` hai, and Flutter/RN comparison harness nahi hai;
- all GitHub Actions workflow files fully commented out hain;
- root `README.md`, examples, starter template, and framework onboarding absent hain.

### Honest current rating

| Area | Score | Meaning |
|---|---:|---|
| Core architecture | 7.5/10 | Strong primitives and separation |
| Live renderer integration | 3.5/10 | Important optimizations are not on the production path |
| Visual design system | 5.0/10 | Good token/contracts, insufficient visual proof |
| Production component library | 4.5/10 | Broad inherited widget set, incomplete Aura-native behavior/semantics |
| Motion/navigation | 3.5/10 | Good motion math, incomplete/duplicated page-transition integration |
| Text/IME/internationalization | 6.5/10 | Strong contracts and security work, limited real-device proof |
| Accessibility | 5.0/10 | Strong retained semantics core, incomplete end-to-end delivery |
| Performance evidence | 2.5/10 | Infrastructure exists; current numbers cannot support competitor claims |
| Developer experience | 2.0/10 | No README/examples/template; hot loop and inspector attachment incomplete |
| Platform/ecosystem reach | 2.0/10 | VYOM-specific opportunity, but no Flutter/RN-style mobile/plugin ecosystem |
| Verification discipline | 7.0/10 | Excellent local test suite, but CI is disabled |
| **Overall GUI competitiveness today** | **4.3/10** | Advanced engine R&D, pre-production framework |

### Best strategic answer

Aura ko Flutter aur React Native dono ki full cross-platform breadth copy nahi karni chahiye. Sabse credible winning strategy hai:

> **“The best GUI runtime for VYOM OS and Wayland-native system applications”** — compositor-aware, Vulkan-first, zero-idle-work, secure by construction, fully deterministic, and deeply profiled.

VYOM hardware/OS par direct system integration is project ka real moat ho sakta hai. “Rust hai” ya “GPU renderer hai” moat nahi hai—Flutter already compiled code and a modern custom renderer use karta hai, while React Native has a mature native-component ecosystem.

---

## 2. Audit scope and verification

### Local verification performed

The following succeeded on the audited checkout:

```text
cargo metadata --format-version 1 --no-deps
cargo check --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Observed facts:

- working tree audit start par clean tha;
- workspace metadata resolves successfully;
- full workspace build succeeds;
- formatting check succeeds;
- Clippy with warnings-as-errors succeeds;
- test inventory contains **1,473 tests/doc-tests**;
- full suite completed with **no failures**; three documentation tests are intentionally ignored;
- benchmark schema integration test alone took about **327 seconds**, so the default test suite is thorough but currently expensive;
- repository contains roughly **136,336 lines of Rust** and **4,242 lines of Markdown** (physical line count, generated/build output excluded).

### What was not verified

- Aura Lab ko visually judge karne ke liye interactive GUI launch/human design review nahi kiya gaya.
- No physical VYOM reference device was available.
- No real Wayland/Smithay compositor integration run was demonstrated.
- No screen-reader session, touch hardware session, thermal signal, suspend/resume, or device-loss recovery was tested end-to-end.
- No Flutter/RN binaries were built in this repository for comparative measurement.

Therefore this audit distinguishes **implemented type/model**, **tested isolated behavior**, and **live production path**. Ye teen cheezein interchangeable nahi hain.

---

## 3. Repository architecture: what is genuinely strong

```text
Application API
  -> aura_winit runtime today
     -> aura_runtime::UserInterface
        -> aura_widget tree
        -> aura_renderer
           -> aura_wgpu / aura_tiny_skia

Parallel capability crates
  -> aura-design
  -> aura-motion
  -> aura-gesture
  -> aura-accessibility
  -> aura-profiler
  -> aura-platform-vyom
```

### 3.1 Type-safe application model

The Elm-style `boot/update/view` API is simple, deterministic, and Rust-native. Message types, renderer/theme types, subscriptions, and tasks are statically checked. For complex system UI, this can be safer than dynamically typed JavaScript application logic.

### 3.2 Rendering breadth

The repo already supports:

- `wgpu` GPU rendering;
- `tiny-skia` software fallback;
- text shaping via `cosmic-text`/`cryoglyph`;
- vector/SVG, images, canvas, custom shader widgets, gradients, meshes, and screenshots;
- configurable frame latency and device-loss detection;
- byte-bounded text/resource/effect caches.

The renderer is a serious base. The missing step is making new retained/effect systems part of its real frame graph.

### 3.3 Layout and large-data work

Strong implemented pieces include:

- constraint-aware row/column/grid/stack layout;
- warm layout caches;
- `SmallVec`-based common flex bookkeeping;
- `VirtualList` with bounded visible slots for 100,000 logical rows;
- sliver protocol foundations;
- cached spatial hit-test index;
- adaptive, responsive, safe-area, constraint, grid, table, stack, and pane widgets.

This is one of Aura's strongest technical areas. Remaining gaps are multi-sliver composition, off-window focus/bring-into-view, and finer invalidation after `diff`.

### 3.4 Input and gesture primitives

`aura-gesture` is not toy code. It contains arena competition, teams/priorities, tap, pan, pinch, rotate, swipe, edge swipe, long press, velocity, and nested-scroll handoff with deterministic replay tests.

The gap is product integration: the default widget/backend flow does not yet turn normal pointer streams into these recognizers automatically.

### 3.5 Design contracts

`aura-design` provides:

- semantic color schemes: light, dark, high contrast;
- typography, spacing, radius, elevation, and motion tokens;
- adaptive viewport and density contracts;
- state matrix for hover/focus/press/selected/disabled/loading/error;
- semantic roles/actions and input-method declarations;
- contrast and touch-target tests;
- component contracts for buttons, cards, dialogs, menus, navigation, notifications, selection controls, text fields, tooltips, bottom sheets, window chrome, and more.

`widget/src/aura.rs` now implements Aura theme catalogs for many real widgets, and real Aura-native widgets exist for `app_bar`, `dialog`, `icon_button`, and `list_tile` in addition to inherited widgets.

This is a strong consistency system. It is not yet the same thing as a visually reviewed component library.

### 3.6 Semantics, privacy, and security thinking

The repository contains unusually thoughtful handling for:

- retained semantic trees and incremental diffs;
- stable semantic identity;
- focus scopes and traps;
- semantic action routing;
- live regions and bounded announcements;
- password/secure-field redaction from debug, IME surrounding text, clipboard, and semantic values;
- protected-content classifications;
- privacy-aware profiler events;
- malformed input and extreme geometry tests.

This can become a real differentiator for a system framework if the final platform bridge is completed.

### 3.7 Observability

`aura-profiler` and Aura Lab have real value:

- bounded event collection;
- phase spans and missed-frame causes;
- widget/layout/damage/semantics/animation inspection;
- privacy-aware export;
- hosted real renderer rather than a null renderer.

The tool honestly marks missing capabilities. Remote attach, overdraw, and complete GPU-resource visibility are not finished.

---

## 4. Critical implementation gaps

This section is the most important part of the audit. Each finding states what exists and what is missing from the live path.

### P0-1: Retained rendering is implemented but not integrated

**Evidence**

- `graphics/src/display_list.rs` implements display lists and builders.
- `graphics/src/retained_layer.rs` implements revision/generation validation, byte budget, LRU eviction, and telemetry.
- Searches show `RetainedLayerCache`, `RetainedLayerKey`, and `DisplayListBuilder` are consumed only inside their own modules and tests.
- `runtime/src/user_interface.rs::draw` still resets the renderer and traverses/draws the complete root plus overlay.

**Impact**

The architecture promises unchanged subtrees with zero widget paint work, but the real frame path does not achieve that. Static/partially changing scenes cannot yet claim the retained-rendering advantage.

**Required outcome**

- Widget/tree revision becomes a stable retained-layer key.
- Draw records display lists per cacheable subtree.
- Unchanged subtree replays without `Widget::draw`.
- Theme, scale, clip, transform, text atlas, and device generation invalidate correctly.
- Aura Lab shows actual retained hit/miss/eviction events.
- An integration test proves a real UI changes one leaf without painting unrelated siblings.

### P0-2: Damage tracking mostly reports reasons; it does not yet deliver partial rendering/present

**Evidence**

- `UserInterface::update` aggregates layout/redraw requests.
- A relayout records full damage with `Cause::Layout`.
- A non-layout redraw commonly records full damage with `Cause::Unknown`.
- `UserInterface::draw` still draws the whole root.
- `wgpu` compositor comments leave backend damage forwarding as future work.

**Impact**

“Zero/precise damage” tests validate region math, not the complete display-to-compositor chain. This is a major gap for power, 120 Hz, and low-end devices.

**Required outcome**

`widget invalidation -> retained paint -> renderer scissor/load -> Wayland surface damage -> presentation feedback` must be one measurable path.

### P0-3: GPU effects are a tested cost model, not visible effects

**Evidence**

- `wgpu/src/effects` implements bounds, blur cost/downsampling policy, graph admission, quality tiers, and texture-pool budgeting.
- The effect graph is referenced only by its own module/tests.
- `docs/performance/effect-budgets.md` explicitly says the graph is not connected to the actual `aura_wgpu` render path.

**Impact**

Blur, backdrop blur, masks, noise, color matrices, and tiered degradation cannot currently be counted as shipped GUI capabilities.

**Required outcome**

- Implement real shaders and pass scheduling.
- Bind effect nodes to retained layers.
- Show real pixels in Aura Gallery.
- Capture GPU time and intermediate texture bytes.
- Verify Recovery tier produces a coherent UI with effects disabled.

### P0-4: Native VYOM backend is a side crate, not the application backend

**Evidence**

- `aura-platform-vyom` has direct Wayland connection, XDG toplevel lifecycle, capability types, input/IME/clipboard/drag-drop/accessibility/power/window-role models.
- Root `aura` includes it only as a dev-dependency.
- No root feature or `Application::run` path selects `aura-platform-vyom`.
- Production application flow still uses `aura_winit`.
- Platform integration tests mostly exercise models/fake globals rather than a running Smithay compositor.

**Impact**

The most important differentiator—deep VYOM compositor integration—is not usable by a normal application yet.

**Required outcome**

- Add an explicit `vyom` backend/profile.
- Connect event loop, renderer surface, frame callback/presentation feedback, seats, keyboard, touch, IME, clipboard, drag-drop, semantics, power, and privileged roles.
- Run reference shell on the actual VYOM compositor.
- Keep `winit` as a compatibility backend, not as the VYOM production path.

### P0-5: Accessibility engine does not reach assistive technology in the normal app path

**Evidence**

- `UserInterface::semantics()` builds a semantic forest.
- `aura-accessibility` implements retained trees, focus, actions, live regions, and AT-SPI code.
- Neither `aura_winit` nor the root application loop calls `UserInterface::semantics()`.
- `aura-platform-vyom::accessibility::Transport` is a bounded in-memory transport, not a connected compositor service.
- Default test run executes `atspi_bus` with zero tests because the optional AT-SPI feature is not active.
- Only about 15 widget implementations currently define a direct `semantics()` method; many complex widgets still expose no semantics.

**Impact**

Accessibility is a strong internal design but not an end-user feature yet.

**Required outcome**

- Every interactive/meaningful production widget emits semantics.
- Application loop diffs semantics each relevant frame.
- AT-SPI/VYOM adapter publishes nodes and accepts actions.
- Hardware keyboard, switch navigation, screen reader, high contrast, large text, RTL, and virtual-list traversal pass on real devices.

### P0-6: Gesture arena is not the default widget interaction path

**Evidence**

- `UserInterface` owns a pointer router and exposes `route_pointer`.
- `aura_winit` does not call that method.
- Standard widgets do not register `aura-gesture` recognizers.
- The API expects a caller to provide the hit path and construct an arena.

**Impact**

Touch feel will not beat Flutter/RN until recognizers, nested scrolling, fling, swipe-back, long press, and cancellation are used by real widgets and navigation.

### P0-7: Motion/navigation has duplicate and incomplete paths

**Evidence**

- `aura-motion` has `Transition`, springs, decay, timelines, keyframes, reduced-motion policy, page-transition math, and shared-element bookkeeping.
- `core::animation` bridges some `aura-motion` functionality into widgets.
- `widget/src/navigator.rs` defines a second self-contained `PageTransition` instead of using `aura-motion::PageTransition`.
- Navigator exposes `push` but no `pop`, back-stack controller, route identity, deep-link state, or gesture-driven progress.
- Navigator computes opacity but draw only applies translation; opacity is used as a binary draw/no-draw threshold.
- “ZoomFade” in the widget navigator does not apply scale.
- The golden transition test named as pixel behavior actually compares a float at fixed time, not renderer output.

**Impact**

This is below the expected motion quality of mature Flutter/RN navigation stacks.

**Required outcome**

- One page-transition type, sourced from `aura-motion`.
- Real opacity/scale/clip/layer transforms.
- push, pop, replace, modal, nested navigator, interrupted reverse, predictive back, shared elements.
- Gesture progress and velocity handoff into spring.
- Pixel/timing goldens at 60/90/120 Hz and reduced motion.

### P0-8: Visual system has contracts, but no visual truth set

**Evidence**

- No `tests/golden/images` assets exist.
- Existing “golden” design/effect tests mostly pin token values, policy, hashes, or region math.
- GPU primitive tests render solid quads; they do not render the component library.
- No real component screenshots are checked in.
- `assets/icons` contains a manifest for two names but no SVG geometry; licenses are still `TBD-before-asset-import`.
- No licensed deterministic test fonts are checked in.
- `docs/design/aura-language.md` explicitly says human visual review is unmet.

**Impact**

There is no evidence that Aura is beautiful, coherent, or visually stable across profiles. Passing contrast and token tests is necessary but insufficient.

**Required outcome**

Create an Aura Gallery and an approved pixel corpus covering every component/state/theme/density/DPI/script. Human review is a required gate.

### P0-9: Benchmark reports cannot support competitive claims

**Evidence**

- `aura_bench` is entirely headless.
- “Present” time is screenshot/readback time, not compositor presentation.
- Pacing feedback is synthesized from CPU frame time.
- Queue depth and several metrics are unmeasured or zero placeholders.
- Cold start, idle wakeups, real input-to-photon, real GPU duration, and real presentation misses are not measured by the saved baseline.
- Saved `benchmarks/baseline/*.json` reports have `environment.device`, `git_commit`, and `rustc_version` as `null`.
- `benchmarks/devices.toml` describes `dev-workstation-01`, but saved reports do not identify that device.
- No target VYOM reference hardware entry has real data.
- No equivalent Flutter or React Native scenes exist.

**Impact**

Current numbers are useful for harness development, not for “faster than Flutter/RN” marketing.

### P0-10: CI is completely disabled

**Evidence**

Every line in these files is commented out:

- `.github/workflows/ci.yml`
- `.github/workflows/perf.yml`
- `.github/workflows/licenses.yml`

**Impact**

Local quality is strong, but nothing prevents a broken main branch, formatting regression, dependency policy violation, or feature-matrix break.

### P0-11: Framework onboarding and packaging are not viable

**Evidence**

- Root `README.md` is absent.
- `examples/` is absent.
- No `cargo aura new`, starter template, gallery, reference shell, or minimal project exists.
- Root crate docs still point to `aura.rs`/book URLs and upstream authorship/repository metadata, while Git remote is the VYOM fork.
- Several docs describe older phase state and contradict newer code. Example: design docs say only six widgets have Aura theme integration, while current `widget/src/aura.rs` has many more catalog implementations; accessibility docs say no platform adapter exists while AT-SPI source now exists.
- `highlighter` is declared as an empty feature which is documented as “must never be enabled”; public feature flags should not contain intentional build traps.

**Impact**

Even a strong Rust developer cannot discover, start, style, debug, package, and ship a VYOM Aura app through a clear supported workflow.

### P1: Reviewability and roadmap discipline

The audited HEAD commit contains roughly **72,422 additions across 454 files** under one commit named `modified`. This conflicts with `phase.md`'s own rule to isolate one architectural risk per commit. The test quality reduces risk, but review, regression bisection, provenance, and future maintenance become much harder.

---

## 5. Component and GUI capability audit

### 5.1 Components

Current widget inventory is broad, but production completeness is uneven.

| Category | Current state | Needed to compete |
|---|---|---|
| Basic layout | Strong | Polish RTL, focus, diagnostics, pixel proof |
| Button/input/selection | Functional and themed | Complete semantics, state/motion goldens, haptics |
| App bar/dialog/icon button/list tile | Real Aura-native widgets | Visual review and device testing |
| Bottom sheet/navigation/notification/toast/window chrome | Design contracts exist | Real widgets, focus/gesture/semantics/platform behavior |
| Menus/pickers/combo boxes | Inherited widget styling exists | Unified Aura behavior, keyboard/typeahead, screen-reader tests |
| Lists/scrolling | Good primitives and VirtualList | Real gesture/fling integration, sliver composition, offscreen focus |
| Rich text/editor | Strong inherited base | Full semantics, IME matrix, undo/redo policy, selection handles |
| Media/platform views | Images/SVG/canvas exist | Video, camera, web/native surface embedding, clipping/compositing rules |
| Navigation | Experimental Navigator | Production router, back handling, restoration, deep links, transitions |
| Icons | Font/inherited assets plus empty Aura SVG set | Licensed, coherent, variable-size icon library |

### 5.2 Touch target policy inconsistency

`phase.md` sets a 48×48 logical-pixel default target. `aura-design::MIN_TOUCH_TARGET` is 44 px, although density helpers often return 48 or more. Choose one policy and enforce it everywhere. For VYOM touch-first system UI, 48 px is the safer default; 44 px can be an explicit platform compatibility mode.

### 5.3 Typography

Strengths:

- grapheme-aware editing;
- deterministic shaping keys;
- script-aware fallback order;
- Devanagari, Gurmukhi, Arabic, Bengali, Tamil, emoji, combining, and BiDi cases;
- secure-field handling;
- byte-bounded text cache.

Gaps:

- fallback lists name fonts that may not exist on the device;
- no licensed deterministic test font corpus;
- no multilingual component pixel goldens;
- no real device proof for selection, cursor, IME candidate windows, virtual keyboard insets, and complex fallback;
- timed password re-mask is application responsibility;
- protected screenshot policy signal is not consumed in the current normal backend.

### 5.4 Responsive/adaptive GUI

Aura has compact/medium/expanded viewport classes, densities, safe area, responsive/adaptive widgets, and a watch profile inside Aura Lab. However, the main design contract exposes only three viewport classes and docs still state watch visual review is absent. The framework needs a single canonical device/profile system used by widgets, gallery, tests, and tooling.

---

## 6. Flutter and React Native baseline

This comparison uses official documentation available at audit time.

### 6.1 Flutter baseline

Flutter already has:

- a custom cross-platform render pipeline and platform embedders for rendering surfaces, input, accessibility, and event loops ([Flutter architectural overview](https://docs.flutter.dev/resources/architectural-overview));
- Impeller, which precompiles shaders, controls caches explicitly, supports modern graphics APIs, and is default on supported iOS/Android configurations ([Impeller](https://docs.flutter.dev/perf/impeller));
- stateful, usually sub-second hot reload and deployment to iOS, Android, web, and desktop ([Flutter FAQ](https://docs.flutter.dev/resources/faq));
- DevTools frame charts, UI/raster timing, widget build/layout/paint tracking, timeline import/export, and jank diagnosis ([Flutter Performance view](https://docs.flutter.dev/tools/devtools/performance));
- unit, widget, and device integration testing workflows ([Flutter testing overview](https://docs.flutter.dev/testing/overview)).

Therefore Aura cannot win merely by having a GPU renderer, declarative widgets, hot reload flag, or frame profiler. Those are table stakes.

### 6.2 React Native baseline

React Native already has:

- a production New Architecture with Fabric, direct native interfaces, concurrent/prioritized work, synchronous layout access, and no legacy asynchronous bridge dependency ([New Architecture](https://reactnative.dev/blog/2024/10/23/the-new-architecture-is-here));
- the New Architecture as the only runtime path in modern releases and Hermes V1 as the default engine in React Native 0.84 ([React Native 0.84](https://reactnative.dev/blog/2026/02/11/react-native-0.84));
- platform-backed native components and a very large mobile package ecosystem;
- near-instant Fast Refresh, usually within one or two seconds, with state preservation where possible ([Fast Refresh](https://reactnative.dev/docs/fast-refresh));
- Android TalkBack and iOS VoiceOver integration ([Accessibility](https://reactnative.dev/docs/accessibility));
- native-thread scrolling/navigation options even when the JavaScript thread is busy ([Performance overview](https://reactnative.dev/docs/performance)).

Aura cannot win on ecosystem size or mobile platform coverage in the short term.

### 6.3 Where Aura can realistically win

| Dimension | Flutter | React Native | Aura winning opportunity |
|---|---|---|---|
| VYOM compositor integration | Generic embedder | Native-module/platform-view integration | Direct Smithay/Wayland/Vulkan contract with privileged VYOM roles |
| Idle power | Mature but general-purpose | Native views plus JS runtime | Prove zero submissions/wakeups and lower memory on VYOM hardware |
| Frame consistency | Impeller is strong | Native/UI-thread paths are strong | Retained/damage pipeline plus compositor feedback tuned to one OS |
| Safety/privacy | Mature APIs | Platform-dependent modules | Secure types and fail-closed system policies end-to-end |
| Determinism | Strong widget tests | Ecosystem-dependent | Reproducible renderer, fixed clock, bounded queues/caches, traceable misses |
| System UI | Possible but generic | Possible through native code | First-class lock/overview/layer/notification/window roles |
| Visual identity | Material/Cupertino/custom | Native/custom/ecosystem | One deeply coherent VYOM visual language with adaptive quality |
| Developer tooling | Very mature | Very mature | System-specific “why this frame missed” and compositor-aware inspection |

---

## 7. Winning product thesis

### 7.1 Do not position Aura as “Flutter but in Rust”

That pitch creates an impossible checklist: iOS, Android, web, desktop, thousands of packages, IDEs, hot reload, add-to-app, platform views, and years of compatibility.

Position it as:

> **Aura is the native application and system-UI framework for VYOM OS: visually coherent, compositor-aware, energy efficient, accessible, and secure by construction.**

### 7.2 Win one reference experience first

Build one public reference app containing:

- launcher/home surface;
- quick settings/control center;
- notification shade;
- settings pages;
- search and command palette;
- file/content list with 100,000 rows;
- dialogs, bottom sheets, menus, toasts, navigation, and window chrome;
- multilingual text-input form;
- media/image grid;
- animations and shared-element transitions;
- light, dark, high contrast, reduced motion, large text, RTL;
- Ultra/Balanced/Efficient/Recovery visual tiers.

This one app should be the visual gallery, benchmark workload, accessibility fixture, profiler target, and release demo. Multiple disconnected demos would dilute proof.

### 7.3 Beat on measured outcomes

Aura should claim leadership only after meeting all of these on named VYOM reference hardware:

| Outcome | Proposed release gate |
|---|---|
| Input-to-photon | P95 at least 15% lower than the best compared Flutter/RN implementation in three representative interactions |
| 120 Hz frame consistency | P99 under 8.33 ms; missed frames below 0.1% over 10 minutes |
| UI-thread work | P95 <= 1.0 ms at 120 Hz for reference shell interactions |
| Idle | 0 GPU submissions, 0 layout/paint passes, <=1 wakeup/s after settling |
| Cold start | First visible simple shell frame <=350 ms; target <=250 ms after optimization |
| Memory | Stable within 5% after 100 navigation loops and cache purge opportunity |
| Virtual list | 100,000 rows with work proportional to visible rows |
| Accessibility | 100% production components named, actionable, focusable where appropriate, and tested with real AT |
| Visual stability | Approved pixel goldens across target themes, DPIs, scripts, quality tiers, and device classes |
| UI iteration | Token/style/content preview <=100 ms; full Rust UI patch target <=1.5 s with state preservation |

The relative comparison must use the same hardware, display mode, scene, assets, fonts, cache state, power state, build mode, and input script.

---

## 8. Prioritized execution plan

Assumption: 3–5 experienced engineers. Solo execution will likely take 2–3× longer.

### Milestone 0 — Restore truth and reviewability (1–2 weeks)

1. Re-enable CI, perf smoke, and license workflows.
2. Add Linux + macOS checks; add a self-hosted VYOM/GPU lane when hardware is ready.
3. Split feature profiles: `default-portable`, `vyom`, `recovery`, `profiling`, `all-widgets`.
4. Remove/repair the intentionally broken `highlighter` feature.
5. Restore root README, contribution guide, architecture index, and current capability matrix.
6. Correct package authors, repository, homepage, book/docs links, and fork attribution.
7. Enforce small, reviewable commits/PRs and maintain a phase-to-live-path status table.

**Exit gate:** clean clone can run documented commands; CI is required; a new developer reaches a window in under 10 minutes.

### Milestone 1 — Build Aura Gallery and visual truth (2–4 weeks)

1. Create `examples/vyom_gallery` as the canonical app.
2. Render every real component in every meaningful state.
3. Add licensed SVG icon set and deterministic test fonts.
4. Add actual PNG pixel goldens for software renderer and selected GPU reference devices.
5. Add phone/tablet/desktop/watch profiles, scale factors, RTL, large text, reduced motion, and quality tiers.
6. Review visuals with a designer; record approved tokens and screenshots.

**Exit gate:** no “contract-only” component is counted as shipped; every shipped component is visible, interactive, semantic, and pixel-reviewed.

### Milestone 2 — Put performance architecture on the live path (4–8 weeks)

1. Integrate tree revisions, display-list recording, and retained-layer replay.
2. Propagate precise damage from widget invalidation through renderer/compositor.
3. Connect the effect graph, texture pool, shaders, and quality tiers.
4. Wire resource metrics into Aura Lab.
5. Complete device-loss rebuild and software fallback escalation.
6. Prove static subtrees do no paint/upload work.

**Exit gate:** Aura Gallery visibly uses these systems; real trace proves their work reduction; no test-only architecture is presented as shipped.

### Milestone 3 — Production motion and touch (4–6 weeks, overlaps Milestone 2 after interfaces settle)

1. Replace Navigator's local transition engine with `aura-motion`.
2. Add pop/replace/modal/nested/deep-link/restoration APIs.
3. Implement actual opacity, scale, transform origin, clip, and retained surface transitions.
4. Wire gesture arenas into buttons, scrolling, sliders, sheets, menus, and navigation.
5. Implement predictive back and velocity-preserving interruption.
6. Add haptic/sonic feedback hooks with platform policy.

**Exit gate:** recorded touch streams and real-device gestures produce identical state transitions; 60/90/120 Hz captures finish at the same semantic time.

### Milestone 4 — VYOM backend end-to-end (6–10 weeks)

1. Make `aura-platform-vyom` selectable by the public app API.
2. Bind required Wayland protocols and actual VYOM authenticated protocols.
3. Connect surface configure/scale/damage/frame feedback.
4. Connect seats, keyboard, multitouch, IME, clipboard, drag-drop.
5. Connect visibility, suspend, thermal, battery, protected-content, session lock, overview, and system gesture edges.
6. Connect semantic diff/action transport.
7. Run reference shell under a real Smithay compositor in CI/hardware lab.

**Exit gate:** `cargo run -p vyom_gallery --features vyom` uses no `winit` application loop and passes lifecycle/accessibility/performance tests on a reference device.

### Milestone 5 — Competitive proof and release experience (4–6 weeks)

1. Create equivalent Flutter and React Native reference scenes.
2. Run release/profile builds on the same hardware.
3. Capture raw CPU/GPU/presentation/input/power/memory samples.
4. Populate every benchmark environment field automatically; reject `null` identity fields.
5. Add windowed compositor benchmark mode and real presentation feedback.
6. Add `cargo aura new`, project template, packaging, deploy, logs, and device selection.
7. Add stateful UI preview/hot patching and error recovery.
8. Add remote Aura Lab attachment, GPU resources, overdraw, and trace export.

**Exit gate:** reproducible third-party-runnable benchmark package supports every public comparison claim.

---

## 9. First 12 implementation tickets

| Priority | Ticket | Primary paths | Definition of done |
|---:|---|---|---|
| 1 | Enable CI | `.github/workflows/*.yml` | Required checks run on every PR |
| 2 | Canonical capability matrix | `README.md`, `docs/status.md` | “model/test/live” state for every subsystem |
| 3 | Aura Gallery | `examples/vyom_gallery/` | Every shipped component/state is visible |
| 4 | Real visual goldens | `tests/golden/images/`, harness | Component screenshots fail on unintended pixel drift |
| 5 | Retained draw integration | `runtime/user_interface`, `graphics/display_list`, `graphics/retained_layer` | Unchanged subtree skips `Widget::draw` |
| 6 | Damage-to-present chain | runtime + `wgpu/window` + VYOM surface | Small leaf update produces bounded compositor damage |
| 7 | Live effects | `wgpu/effects`, layer/engine shaders | Blur/shadow/mask render and obey quality budgets |
| 8 | Navigator unification | `widget/navigator`, `aura-motion` | push/pop/reverse with real opacity/scale and interruption |
| 9 | Gesture integration | widgets + runtime + platform backends | Standard controls automatically use gesture arena |
| 10 | VYOM public backend | root API + `aura-platform-vyom` | Normal app runs directly on VYOM backend |
| 11 | Accessibility delivery | runtime + accessibility + platform | Orca/VYOM AT can enumerate and operate Gallery |
| 12 | Windowed comparative bench | `tools/aura_bench`, competitor fixtures | Real input-to-photon/presentation metrics on named device |

Tickets 5–12 should not be merged as one large “modified” commit. Each needs a focused benchmark, visual check, and rollback boundary.

---

## 10. Benchmark design for a fair Flutter/RN comparison

### Common scenes

1. **Cold shell:** app start to first interactive frame.
2. **Settings scroll:** mixed text, icons, toggles, dividers, sticky app bar.
3. **100k list:** continuous scroll, jump, insert, update, and focus traversal.
4. **Notification shade:** backdrop/effects, nested scroll, gestures, dynamic cards.
5. **Navigation stress:** push/pop/shared element with interruption.
6. **Text editor:** multilingual shaping, selection, IME composition, secure field.
7. **Image grid:** decode/upload/scroll/cache pressure.
8. **Idle:** settled app for 10 minutes.

### Required measurements

- process start to visible and interactive;
- event timestamp to submit and actual compositor presentation;
- P50/P95/P99 CPU frame phases;
- GPU timestamps and presentation lateness;
- missed/dropped frames at 60/90/120 Hz;
- wakeups, CPU residency, GPU submissions, and energy;
- allocations, RSS, GPU bytes, cache hit/miss/eviction;
- nodes visited/diffed/laid out/painted;
- damage area and overdraw;
- APK/bundle/binary size;
- clean build, incremental build, UI preview, hot patch timings.

### Anti-cheating rules

- identical assets, fonts, text, item counts, animation duration, and input trace;
- release-equivalent builds only;
- same device temperature/power mode and randomized run order;
- cold/warm cache states reported separately;
- minimum five runs, raw samples retained;
- device, commit, compiler, OS, compositor, driver, refresh rate, and build flags mandatory;
- a missing metric is `unmeasured`, never zero;
- result must be reproducible outside the development machine.

---

## 11. Developer experience required to compete

Flutter and RN win many teams before runtime performance matters because the first screen, reload loop, documentation, packages, and debugging are easy.

Aura needs this supported path:

```text
cargo install aura-cli
aura new settings_app --platform vyom
cd settings_app
aura run --device vyom-emulator
# edit -> state-preserving refresh
aura inspect
aura test --golden
aura package
```

Minimum DX deliverables:

- project generator and versioned templates;
- device/emulator discovery;
- clear compiler/runtime error overlay;
- state-preserving hot UI patching;
- component explorer with token editing;
- automatic golden capture/update review;
- remote inspector attachment;
- one command for trace + benchmark bundle;
- package/plugin manifest and ABI/version policy;
- cookbook for navigation, async data, forms, lists, media, platform APIs, testing, accessibility, localization, and deployment.

Rust compile time is a real competitive risk. A practical solution is two loops:

1. **Sub-100 ms design loop:** serialize component scene/tokens/content into an already running Gallery process.
2. **Sub-1.5 s code loop:** incremental compile + dynamic patch/restart with state restoration.

Trying to make every Rust code change sub-second before shipping a design preview will delay the product.

---

## 12. What not to do next

- Do not add more standalone policy structs before existing ones are wired into the live app.
- Do not count contract tests as pixel goldens.
- Do not count cost-model effects as rendered effects.
- Do not claim retained rendering while the real draw path traverses the whole tree.
- Do not claim native VYOM integration while normal apps still run through `winit`.
- Do not publish “fastest” from headless screenshot/readback benchmarks.
- Do not chase iOS/Android parity before the VYOM reference shell is excellent.
- Do not expand the component count without finishing semantics, motion, keyboard, touch, visuals, and tests for each component.
- Do not accept another multi-phase, 400+ file commit.

---

## 13. Go/no-go criteria for public “beats Flutter/RN” messaging

Public comparative messaging is a **no-go today**.

It becomes a go only when:

- VYOM backend is the actual app path;
- retained rendering, damage, effects, recovery, gestures, and semantics are live;
- Aura Gallery proves visual quality;
- CI is mandatory;
- named reference hardware produces real windowed/presentation metrics;
- equivalent Flutter/RN implementations and raw results are public;
- claims specify scope: “on VYOM reference hardware for these workloads,” not universal superiority;
- a designer and accessibility tester approve the reference experience.

The first credible headline should be narrow and provable, for example:

> “On VYOM reference hardware, Aura's system-settings workload delivers lower P95 input-to-photon latency, zero idle GPU submissions, and lower steady-state memory than the equivalent Flutter and React Native builds.”

Only publish it if the data actually says so.

---

## 14. Final recommendation

The codebase does not need another giant feature phase. It needs a **product-integration phase**.

Recommended order:

1. enable CI and repair public project truth;
2. ship Aura Gallery with real pixel goldens;
3. wire retained/damage/effects into live rendering;
4. fix navigation/motion and standardize gesture use;
5. make VYOM backend and accessibility end-to-end;
6. build the developer loop;
7. run fair physical-device comparisons.

If this order is followed, Aura can plausibly beat Flutter/RN **for VYOM-native GUI and system UI**. If the project instead keeps adding isolated engines, contracts, and tests without moving them onto the live path, it will remain technically impressive but visually and commercially non-competitive.

**Confidence:** high for repository findings; medium for delivery estimates; competitive claims require the missing physical-device measurements.
