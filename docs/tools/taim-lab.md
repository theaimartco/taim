# Taim Lab and the profiler

> Status: Phase 15, partial. The profiler — schema, collection, export,
> and the runtime bridge — is built and tested. **Taim Lab itself, the
> standalone inspector application, is not.** See
> [What is not built](#what-is-not-built) for exactly what that means and
> what it costs you today.

## What this is for

Phase 15's objective is to make performance and correctness observable
*without attaching a general-purpose debugger*. A profiler is not a
stopwatch: `22ms` is not a finding. "22ms, of which 14 was `Paint`,
because the text cache missed 40 times" is a finding, because it tells you
where to go.

That is why the exit gate is "every global metric can be traced to a
phase/cause" rather than "we can measure frame time".

## The two rules that shape the design

### It must be free when off

`Collector::record` takes a **closure**, not an `Event`:

```rust
collector.record(|| Event::Semantics {
    nodes: tree.count(),   // never runs when profiling is off
    rebuilt,
    at: collector.now(),   // nor this
});
```

With collection disabled, the closure is never called. A recording site
costs one relaxed atomic load and a not-taken branch — no clock read, no
tree walk, no lock. That is what makes the "below 1% disabled overhead"
budget achievable rather than aspirational, and it is why `record` has the
signature it does.

If you add instrumentation, **put the work inside the closure.** A
`let nodes = tree.count();` on the line above defeats the whole design and
nothing will warn you.

### It must be unable to leak

`phase.md` requires that secure text, private notifications, protected
surfaces, clipboard contents and raw images are redacted by default.

The obvious way to do that is a redaction pass before export. It is the
wrong way, because it **fails open**: every new event kind is a leak until
someone remembers to teach the pass about it, and nothing fails when they
forget.

So an `Event` *cannot hold user content at all*:

- The only free-form string it carries is a `Label`, which is
  `&'static str` — it can only come from the program's own source. A
  keystroke cannot become one.
- Where an event must refer to user data, it carries a `Redacted`, which
  stores a length and a classification and never the bytes. `Redacted::new`
  takes a **length**, not the content — there is no parameter to pass the
  secret into by mistake.

Redaction is the shape of the type. Forgetting it is a compile error.

## Vocabulary

| Type | Question it answers |
|---|---|
| `Phase` | *Where* in the frame did the time go? (`Layout`, `Paint`, …) |
| `Cause` | *Why* was a frame late? (a phase, a cache miss, a GPU stall) |
| `Resource` | *What kind* of thing did a cache hold? |
| `Sensitivity` | *How* sensitive was content that a tool must not show? |
| `Priority` | *What survives* when the buffer is full? |

## Turning it on

The trace goes on the `Cache`, not the `UserInterface` — `Diff` and
`Layout` run inside `UserInterface::build`, before there is a
`UserInterface` to attach anything to, and a profiler that could see every
phase *except* the two a slow first frame is made of would be missing the
point:

```rust
let collector = Collector::enabled();

let mut ui = UserInterface::build(
    view(),
    bounds,
    cache.with_trace(Trace::new(collector.clone())),
    &mut renderer,
);

// ... drive a frame ...

std::fs::write("trace.json", taim_profiler::export::taim(&collector))?;
```

It survives `relayout`, which rebuilds: a trace that switched itself off
the first time the window resized would be a puzzle rather than a tool.

## Reading a trace

Two formats:

- `export::chrome` — the Trace Event Format. Open it in `chrome://tracing`
  or Perfetto; no tooling to install.
- `export::taim` — the above, plus phase attribution and drop accounting.

**Timestamps in the Chrome format are microseconds.** They are nanoseconds
everywhere inside the profiler. A trace that confuses the two renders a
16ms frame as 16 seconds, which reads as a catastrophic hang rather than a
unit bug.

### The residue is the point

`export::phase_totals` gives you time per phase. The frame's total minus
that sum is **unattributed time**, and it is deliberately visible rather
than distributed. If a frame is 10ms and the phases account for 9ms, the
missing millisecond is a finding — an uninstrumented path — not a rounding
error to hide.

### Holes are declared

`Dropped` reports what the collector lost: `overflow`, `sampled`,
`contended`. A trace with a hole in it that does not say so is a trace
that lies, and a suspiciously clean timeline is exactly what a full buffer
looks like. Check it before concluding anything.

## Under pressure

The buffer is **bounded** and never grows. A profiler that grows without
bound under load kills the application it was meant to diagnose, precisely
when things are already going wrong.

When it is full, it drops **telemetry** and keeps **diagnostics**:

| Kept | Dropped first |
|---|---|
| `Missed` (a frame's cause) | `CacheHit` / `CacheMiss` |
| `Evicted` | `PhaseBegin` / `PhaseEnd` |
| `Withheld` | `Input`, `Semantics` |

Losing some cache-hit marks costs you resolution. Losing the
missed-frame cause costs you the answer.

Sampling (`Sampling::every(n)`) thins telemetry and **never** thins a
diagnostic — there is no such thing as every tenth missed frame. It is
deterministic, not random: a trace you cannot reproduce is a trace you
cannot bisect.

## Opening a span

Use `Trace::span`, not a `begin`/`end` pair, anywhere the scope has more
than one exit:

```rust
let _span = self.trace.span(Phase::PaintPreparation);
```

`UserInterface::draw` returns early when there is no overlay. A
hand-written `end` after that point is simply not reached, and the span
stays open — silently charging the rest of the frame, *and every frame
after it*, to `Paint`. **A leaked span does not fail, it lies**, which is
why `tests/profiler/pipeline.rs` checks the begin/end balance explicitly:
with the leak reintroduced, every other test in that file still passes.

## Recording never blocks

`record` uses `try_lock` and drops the event rather than wait. A profiler
must never be the reason a frame is late, so contention costs events
(reported as `Dropped::contended`), not milliseconds.

## Counters and traces

`taim_runtime::diagnostics::Counters` answers *how many times*;
the profiler answers *how long, and why*. They are separate because a
counter is an atomic add with no timestamp and no buffer — cheap enough to
leave on in a benchmark — while a trace costs a clock read and a ring slot.

`diagnostics::Trace` bridges them, mapping the runtime's `Phase` onto the
profiler's so an overlay and a benchmark describe the same frame in the
same words. Two mappings are not one-to-one, on purpose:

- `PaintPreparation` → `Paint`: a developer reading a flame chart cannot
  act on the difference between preparing a draw and issuing it.
- `Upload` → `Submit`: an atlas write is work handed to the GPU.
- `DamageFallback` → *nothing*. It is not a phase; it is the record of the
  damage tracker giving up, which is a **cause**. It records as a
  diagnostic, so it survives the pressure that made it happen.

## Privacy restrictions on tooling

Any tool built on this — Taim Lab included — inherits these, because they
are enforced below it:

- It cannot display secure text. The bytes never enter the profiler.
- It cannot display a password's **length** either. That is worth real
  bits to an attacker and no profiling question needs it. An ordinary
  redaction keeps its length; a secure one does not.
- A protected surface's *size* is available (a developer chasing a memory
  question needs it); its pixels are not.
- "Nothing happened" and "something happened that you may not see" are
  distinguishable: the latter records a `Withheld` marker. An absent event
  would look like a bug in the profiler and send someone hunting for it.

## What is not built

Honestly, and in full:

- **Taim Lab itself.** The plan calls for a standalone inspector with nine
  panels — widget tree, layout overlay, damage overlay, overdraw heat map,
  frame graph, GPU resources, semantics inspector, animation inspector, and
  device-profile simulation. None of it exists. What exists is the data it
  would read.
- **Only five phases are instrumented.** `UserInterface` opens spans for
  `Diff`, `Layout`, `Input`, `Paint` and `Semantics`. `View`, `Damage`,
  `Submit`, `Present` and `Animation` exist in the schema and nothing
  records them, so their time lands in the unattributed residue — which is
  at least *visible*, rather than misfiled.
- **GPU events.** `Resource::{Atlas, Buffer, Pipeline}` exist in the
  schema; nothing in `wgpu` records them.
- **The overhead budgets are unproven.** The tests here pin the
  *structural* property — a disabled collector never builds the event —
  and smoke-test the timing with bounds loose enough to catch a category
  error (a lock, an allocation) rather than a percentage. The 1%/3%
  numbers are budgets for `taim_bench` on a quiet reference machine. A
  unit test on a shared box cannot honestly assert them, and one that
  claimed to would be a test that gets loosened until it stops failing.
