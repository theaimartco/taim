# TAIM 1.0 evidence-based release checklist

**Current decision: NO-GO.** Never treat a green compile or internal frame
benchmark as sufficient evidence of production readiness.

- [ ] Active PR/main CI green: fmt, clippy, workspace tests, goldens,
      accessibility and measurement contract tests.
- [ ] Real Wayland/VYOM presentation verified and supported GPUs/devices
      explicitly documented.
- [ ] Physical pointer/touch, multilingual IME and screen-reader tasks pass.
- [ ] GPU device-loss and software-fallback recovery pass on staging target.
- [ ] Frame p50/p95/p99, real input-to-photon, RSS, GPU memory and idle power
      are measured on actual devices or transparently marked BLOCKED.
- [ ] Same-hardware Iced / Slint / egui measurements are reproducible.
- [ ] Stable public API frozen; clean consumer can build package/examples.
- [ ] Security and source/license attribution audits approved.
- [ ] Supported platform and feature matrix verified and published.
- [ ] Release reproducibility: source SHA, compiler, Cargo.lock, SBOM,
      artifact checksums, independent clean rebuild and signed tag.
- [ ] Release owner and independent reviewer approve all gate evidence.

Required reading:
- docs/testing/wayland-vyom-device-acceptance.md
- docs/performance/four-framework-comparison.md
- docs/release/production-gates.md
- SECURITY.md
