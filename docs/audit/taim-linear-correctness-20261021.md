# TAIM linear sizing + baseline robustness, October 21 2026

Development state, NOT VERIFIED: PR #19 has a new native sizing-intent module for Row and Column which removes stale inferred Fill sizing after child removal. This follow-on patch also normalizes untrusted padding before coordinate calculations, skips unsafe alignment when child cross extent is NaN/Infinity, and corrects a top-padding baseline double-count in the TAIM-owned prefix placement pass.

Three additional source regression tests were added for baseline/padding, invalid cross dimension and malformed padding; all are **unexecuted** and not production evidence. This makes 20 newly committed source tests across the PR #19 layout/sizing refactors (5 schedule + 6 placement + 6 sizing + 3 robustness), not 20 passing tests.

Remaining acceptance blockers: compile/Clippy/full tests, layout and RTL goldens, source-lineage manual audit, VYOM/Wayland/IME/accessibility/GPU device QA, cross-platform matrix, real comparative p50/p95/p99 and performance/energy traces, independent security and release signoff. Keep original upstream credits and licenses. Do not call this 100% independently authored or production-ready.
