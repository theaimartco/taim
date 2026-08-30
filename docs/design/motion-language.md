# Taim motion language

Taim motion is driven only by the runtime's per-window monotonic frame clock.
Widgets never create timers or render loops. A transition registers work while
it is active and unregisters immediately when settled, preserving Taim's idle
zero-submission rule.

Use 100–200 ms for direct feedback, 200–400 ms for hierarchy changes, and a
spring for gesture handoff. Retarget an in-progress transition instead of
restarting it: its sampled position, and spring velocity where applicable,
must be preserved.

Reduced motion changes spatial transforms to short fades (at most 150 ms) or
instant semantic state changes. It is not a blanket zero-duration setting:
users still need an understandable indication that navigation or selection
changed.

Shared-element snapshots are renderer-owned retained layers. They must be
released on completion, cancellation, and navigation; the bounded
`SharedElements` registry returns every layer to the renderer on cleanup.
