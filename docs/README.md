# Taim Documentation

Documentation for **Taim**, the GPU-native, fine-grained reactive GUI framework for Rust.

## 📚 User Guide

Start here. [Getting Started](guide/getting-started.md) covers installation,
state, layout, lists, navigation, background work, storage, liquid glass and
running on every platform; its examples are compiled by the test suite.

| Guide | Description |
|---|---|
| [Getting Started](guide/getting-started.md) | From install to a running app, on the view API |
| [Animations & Motion](guide/animations.md) | Springs, tweens, decay, page transitions, shared elements |
| [Gestures](guide/gestures.md) | Gesture arena, tap, pan, swipe, pinch, rotate, nested scroll |
| [Design System](guide/design-system.md) | Color tokens, typography, spacing, radius, elevation, motion tokens |
| [Accessibility](guide/accessibility.md) | Semantic tree, focus, screen readers per platform, live regions |

Being rewritten for the view API (their code targets the pre-0.15 API):
[Widgets](guide/widgets.md), [Layout](guide/layout.md),
[Styling](guide/styling.md), [Tasks & Subscriptions](guide/tasks-and-subscriptions.md),
[Canvas Drawing](guide/canvas-drawing.md), [Scaling](guide/scaling.md).

## 📦 Platforms and release

| Document | Description |
|---|---|
| [Android](platform/android.md) | Building, running and the Play Store bundle |
| [iOS](platform/ios.md) | Simulator and device bundles, App Store packaging |
| [macOS](platform/macos.md) | Sandboxed apps and Mac App Store packaging |
| [Web](platform/web.md) | WebAssembly with WebGL2 |
| [Store Submission](release/store-submission.md) | Google Play, App Store, Mac App Store, Microsoft Store, Snap Store |
| [Production Gates](release/production-gates.md) | What every release must pass |

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
| [Numbers](performance/numbers.md) | Every published measurement, with hardware and method |
| [1.0 Report](performance/taim-1.0-report.md) | What is and is not yet measured for 1.0 |

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
