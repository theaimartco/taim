# Changelog

All notable changes to this project will be documented in this file.

## [1.0.0] - Unreleased

### Core
- Introduced `aura::Application` trait for robust app lifecycles.
- Zero-idle-work architecture for power efficiency.

### Rendering
- `wgpu` backend with Vulkan support for high performance.
- `tiny-skia` software fallback renderer.

### Layout
- Advanced flexbox-based layout system.

### Input & Motion
- Gesture arena for complex touch disambiguation.
- `aura-motion` tweening engine for fluid animations.

### Design
- `aura-design` crate for VYOM design system tokens.

### Accessibility
- Built-in semantics tree generation.

### Platform
- First-class Wayland support.
- Custom VYOM platform integration (`aura-platform-vyom`).

### Tools & Testing
- `aura_bench` for competitive performance analysis.
- `vyom_shell` reference implementation.
