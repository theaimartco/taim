# TAIM independent identity: engineering acceptance contract

**Goal:** make TAIM's public programming model, application lifecycle,
renderer and widgets distinct through actual native implementation, not a
rename or replacement of attribution. Work in this branch is incremental and
**does not yet establish 100% original code**.

## Starting facts (September 19, 2026 inherited code audit)

- TAIM began as an Iced fork at 81afda72. Preserve LICENSE, CREDITS.md
  and complete Git history. Any public representation must acknowledge the
  actual code lineage when relevant.
- Existing source audit estimated 97.39% normalized-token independence,
  with approximately 5,152 inherited-equivalent LOC, 68 byte-identical
  files and 32 high-similarity files. These are older unverified-for-this-branch
  estimates, not an updated measurement and not a legal ownership test.
- Existing direct iced-rs Git / Cargo dependencies are reported absent.
  This is DIFFERENT from source ownership.
- The native AppRunner, task scheduler and redraw gate are already TAIM
  work. Existing Program, Widget, Element and familiar construction
  helpers still retain recognizably Iced-derived conceptual and source APIs.

## Newly implemented: typed intent-and-scene application contract

TAIM native app lets a reducer return an explicit Dispatch:
- idle: no new frame, even if an unrelated message was processed
- paint: redraw without declaring relayout
- layout: invalidate layout and painting
- animate: request a continuing animation frame
- each may schedule asynchronous tasks independently

The native app implements App directly and uses AppRunner without the
legacy ProgramAdapter route. See program/src/native.rs and the tests.
No changes to historical copyright records or public API compatibility
were needed for this first independent application contract.

## Completion gates for a truly independent TAIM architecture

1. **Source provenance, not file names.** Run scripts/audit/check-independence.py
   at the proposed commit. Investigate every byte-identical and high-similarity
   code file against the fork baseline, with an independently reviewed manual
   classification. Cosmetic renaming does not satisfy this gate.
2. **Public ergonomics.** Make the native intent/context contract the documented
   default. Build clean native sample apps and benchmark developer-experience
   overhead. Legacy Program compatibility may be deprecated with a documented
   migration period, not abruptly removed from consumers.
3. **Distinct widget stack.** Replace remaining inherited implementation of
   core input, editing, scrolling, focus, layout and accessibility with
   original code and contract tests while preserving usability.
4. **Rendering and platform.** Finish original render/text/image paths,
   native VYOM integration, Wayland/IME and GPU-loss acceptance. Keep WGPU
   and neutral crates as clearly disclosed third-party dependencies.
5. **Objective performance.** All replacements must pass functional, visual
   golden, hardware and accessibility regression suites before merge.
   Regression thresholds must be justified, not waived to improve a score.
6. **Licensed origin.** Keep proper attribution even if 100% of present-day
   executable source is eventually rewritten. A historic fork cannot
   truthfully be described as never having been based on Iced.
7. **Publication gate.** Rerun all CI after runner allocation issue #2 is
   resolved; require independent manual source review before a stable tag.

## Automation boundary

The source-lineage scanner measures similarity against historical files.
The new scripts/audit/check-upstream-boundary.py disallows reintroducing
Iced Cargo and Git dependencies, including aliased or target-specific
manifests. They cover different properties. Neither turns a subjective
"looks distinct" target into a mathematically provable 100% originality
certificate.

**No-go for claiming all source is original until the source audit,
manual verification, release CI and licensing review actually complete.**
