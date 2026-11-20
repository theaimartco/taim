# TAIM source independence promotion policy

TAIM began as an Iced fork at 81afda72. Preserve its complete MIT license,
CREDITS.md and Git history, even after all inherited active source is rewritten.
A 100%-TAIM current implementation is an engineering objective, not permission
to conceal historical authorship.

## Verifiable release gate

Run from a complete and clean Git checkout:

    python3 scripts/audit/source-lineage-gate.py --strict --output /tmp/taim-audit.json

The strict command blocks promotion whenever the scanner detects byte-identical
code, token-identical or high-similarity code, disguised copies, or
inherited-equivalent lines. It checks fork ancestry and retains attribution.
Even a zero automated count cannot prove that migrated/renamed files were
authored independently: the same work needs a manual code-provenance audit.

## Current migration record

- Native intent-driven app and real Winit FrameGate dispatch added in PR #3.
- TAIM-owned subscription reconciliation replaces the inherited tracker module.
  It deduplicates recipes, cancels retired streams during output backpressure,
  resets identity epochs safely and bounds broadcast queues.
- CI checks direct and aliased Iced Cargo packages, patches, workspace inputs,
  and git sources. This is dependency independence, not source independence.
- Core widgets, renderer and portions of platform code still require source
  review, substantive replacements, functional parity and physical testing.
- Hosted CI is blocked at GitHub runner allocation issue #2. Do not claim
  that Rust compile, Clippy, tests or hardware gates passed.

Completion requires strict audit, independently reviewed source origin,
green executable CI, real desktop/VYOM/IME/AT-SPI/GPU acceptance,
license/security assessment and signed release owner approval.
