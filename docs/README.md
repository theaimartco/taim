# Taim Documentation

Welcome to the official documentation for the **Taim UI Framework** — the GPU-first, zero-compromise GUI framework built in Rust.

## 📚 User Guide

Step-by-step guides to learn Taim from scratch:

| Guide | Description |
|---|---|
| [Getting Started](guide/getting-started.md) | Installation, first app, the Elm architecture, feature flags, themes |
| [Layout](guide/layout.md) | Length, columns, rows, containers, scrolling, responsive & adaptive layout |
| [Widgets](guide/widgets.md) | Complete widget catalog — 50+ widgets with examples |
| [Styling & Theming](guide/styling.md) | Built-in themes, custom themes, widget styling, the palette system |
| [Tasks & Subscriptions](guide/tasks-and-subscriptions.md) | Async tasks, time ticks, keyboard events, window management |
| [Animations & Motion](guide/animations.md) | Springs, tweens, decay, page transitions, shared elements |
| [Gestures](guide/gestures.md) | Gesture arena, tap, pan, swipe, pinch, rotate, nested scroll |
| [Canvas Drawing](guide/canvas-drawing.md) | Custom 2D drawing, paths, fills, strokes, geometry caching |
| [Design System](guide/design-system.md) | Color tokens, typography, spacing, radius, elevation, motion tokens |
| [Accessibility](guide/accessibility.md) | Semantic tree, focus management, screen readers, live regions |
| [Scaling Applications](guide/scaling.md) | Component pattern, screen composition, message mapping, multi-window |

## 🏗️ Architecture

Deep dives into how Taim works internally:

| Document | Description |
|---|---|
| [Path Map](architecture/path-map.md) | Crate dependency graph and module map |
| [Widget Lifecycle](architecture/widget-lifecycle.md) | How widgets are created, reconciled, and destroyed |
| [Retained Rendering](architecture/retained-rendering.md) | Unchanged subtrees skip paint — zero idle work |
| [Frame Scheduling](architecture/frame-scheduling.md) | Compositor-aware frame pacing for smooth 120Hz |
| [Layout](architecture/layout.md) | Incremental constraint-based layout engine |
| [Invalidation](architecture/invalidation.md) | How dirty tracking triggers minimal re-computation |
| [Input & Gestures](architecture/input-gestures.md) | Pointer routing and gesture arena internals |
| [Text & IME](architecture/text-ime.md) | Text shaping, input method, and selection |
| [Feature Policy](architecture/feature-policy.md) | When and how to add new features |

## 🎨 Design

The visual language behind Taim:

| Document | Description |
|---|---|
| [Taim Design Language](design/taim-language.md) | Visual principles and component philosophy |
| [Motion Language](design/motion-language.md) | Animation principles and timing semantics |

## ⚡ Performance

Engineering Taim for speed:

| Document | Description |
|---|---|
| [GPU Pipeline](performance/gpu-pipeline.md) | Batching, pipeline cache, atlas management |
| [Effect Budgets](performance/effect-budgets.md) | Blur, shadow, and mask cost accounting |
| [Methodology](performance/methodology.md) | How we measure and compare performance |
| [1.0 Report](performance/taim-1.0-report.md) | Benchmark results and analysis |

## 🧪 Testing

Quality assurance infrastructure:

| Document | Description |
|---|---|
| [Golden Policy](testing/golden-policy.md) | Visual regression test approval process |
| [Release Matrix](testing/release-matrix.md) | Device, language, and platform coverage |
| [Production hardening evidence](testing/production-full-access-20261115.md) | Clean release candidate, runtime probes and remaining acceptance gates |

## 🔒 Security

| Document | Description |
|---|---|
| [Threat Model](security/threat-model.md) | Trust boundaries, protected assets, mitigations |

## 🛠️ Tools

[macOS apps and Mac App Store packaging](platform/macos.md) covers sandboxed
app bundles, distribution signing, installer validation and the runtime fixture.

[iOS Simulator packaging](platform/ios.md) covers native Simulator bundles,
the verified rendering path and the remaining device/accessibility limitations.

| Document | Description |
|---|---|
| [Taim Lab](tools/taim-lab.md) | Widget inspector, layout overlay, frame profiler |

## 🔧 Operations

| Document | Description |
|---|---|
| [Recovery](operations/recovery.md) | GPU loss recovery ladder and crash loop detection |

## ♿ Accessibility

| Document | Description |
|---|---|
| [Contract](accessibility/contract.md) | Accessibility requirements and compliance |
