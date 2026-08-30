# Changelog

All notable changes to this project will be documented in this file.

## [1.0.0] - Unreleased

### Core
- Introduced `taim::Application` trait for robust app lifecycles.
- Zero-idle-work architecture for power efficiency.

### Rendering
- `wgpu` backend with Vulkan support for high performance.
- `tiny-skia` software fallback renderer.

### Layout
- Advanced flexbox-based layout system.

### Input & Motion
- Gesture arena for complex touch disambiguation.
- `taim-motion` tweening engine for fluid animations.

### Design
- `taim-design` crate for VYOM design system tokens.

### Accessibility
- Built-in semantics tree generation.

### Platform
- First-class Wayland support.
- Custom VYOM platform integration (`taim-platform-vyom`).

### Tools & Testing
- `taim_bench` for competitive performance analysis.
- `vyom_shell` reference implementation.
