# TAIM production verification gate — 2026-10-21

**NO-GO — not production-ready.**

This changes the existing local verifier, not the product release status. The runner now requires a real full commit SHA and clean tracked checkout before running, enforces the exact set/order of named checks instead of merely checking PASS count, includes release-contract synthetic tests, CPU goldens, frame-time contract and cargo-deny license/advisory checks, and records SHA-256 hashes of executed log files. Missing tools and any failed gate result in NO_GO.

Run on a real, clean, full-history checkout: `python3 scripts/qa/verify-taim.py --stop-on-failure`. Install rustup toolchain 1.95.0 and cargo-deny. Source-lineage test requires historical fork commit as a reachable ancestor. Logs are created in `artifacts/taim-local`. If logs are tracked, use ignored temporary output or a dedicated clean checkout; do not bypass the tracked clean-tree preflight.

**Unexecuted in this environment:** Rust fmt/check/Clippy/test, CPU goldens, frame-time tests, dependency audit, platform and accessibility acceptance. GitHub Actions runner-allocation blocker remains https://github.com/theaimartco/taim/issues/2. No disabled workflow has been enabled, no tag published and main untouched.

The separate `scripts/release/verify-readiness.py` integrity validator plus manual source audit, GPU-loss recovery, Wayland/VYOM, screen-reader and IME physical testing, reproducibility and independent signoff are still mandatory. Passing this local code verifier is *not* a production certification.
