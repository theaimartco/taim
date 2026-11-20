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

## Springs in the views

Every moving part of the views runs on one damped spring, solved in closed
form (`view/src/views/look.rs`). Its value at any instant needs no stepping.
A retarget starts from where the spring is drawn, at the speed it had, so
interrupting a motion never jumps. With Reduce Motion every spring jumps straight
to its target. `appear` keeps a 150 ms fade.

| Tuning | ω | ζ | Used for |
| --- | --- | --- | --- |
| `PRESS` | 26 | 0.62 | sinking under a finger |
| `RELEASE` | 22 | 0.32 | glass springing back |
| `HOVER` | 14 | 1.0 | hover light |
| `SNAP` | 30 | 0.82 | indicators, switch knob |
| `POP` | 24 | 0.45 | ticks, dots, slider thumb |
| `TRAIL` | 17 | 0.9 | the trailing edge of a stretchy indicator |
| `GLIDE` | 18 | 0.86 | `animated` |

### What moves

- **Buttons and chips** shrink by 4% (chips 6%) under the finger and
  overshoot slightly when let go.
- **Switch**: the knob slides and the track colour follows it. Held, the
  knob widens.
- **Checkbox and radio**: the tick scales in and the dot pops.
- **Segmented control**: the indicator's leading edge snaps to the new
  segment and its trailing edge follows more slowly. The indicator stretches,
  catches up and bounces, and the chosen label pops.
- **Slider**: the thumb swells by a third while dragged.
- **Scroll and list**: dragging past an end stretches with growing
  resistance, never more than 140 px. A fling that hits an end carries past
  it and springs back, critically damped.

### Motion for any view

```rust
// Glide to wherever layout moves it, instead of jumping.
animated(card)

// Fade and rise in when first drawn; cascade a list.
appear(row).delay(Duration::from_millis(40 * index)).rise(16.0)
```

Both change only drawing, through `View::visual`, which returns an offset,
a scale about the centre and an opacity. Pointer input still finds a view
at its laid-out place. Content inside `when` replays `appear` each time it
is shown. New items of `each` appear, and items that keep their view keep
their glide.

### Tests

- `view/src/views/motion.rs`: gliding, continuing a glide mid-motion,
  appearing with a delay, and reduced motion.
- `view/src/views/scroll.rs`: stretch resistance, a fling bouncing off the
  top, and dragging back out of a stretch.
- `tests/glass/live.rs`: `appearing_content_is_drawn_faded_and_lowered_then_in_place`
  renders on the GPU.

### Page transitions

```rust
navigator(nav, page).transition(PageTransition::Zoom)
```

| Transition | The new page | The page underneath |
| --- | --- | --- |
| `Slide` (default) | slides in from the right, casting a shadow | drifts left by 30% and darkens |
| `Zoom` | grows from 90% while fading in | swells by 6% |
| `Fade` | fades in | stays |
| `Rise` | rises from the bottom, casting a shadow | recedes to 95% and darkens |
| `None` | appears at once | — |

Going back plays the same thing in reverse. Transitions take 0.3 s on an
ease-out curve. With Reduce Motion, pages change at once.

With `Slide`, a finger that starts within 24 px of the left edge and moves
sideways drags the top page back. A vertical move first leaves the gesture
to the page, for scrolling. Let go past 40% of the width, or faster than
500 px/s, and the page finishes going at the finger's speed (at least
0.12 s). Otherwise it returns.

### Heroes

```rust
// On the list page:
hero(("photo", id), image(thumb))
// On the detail page:
hero(("photo", id), image(full))
```

During a transition, the hero on the page that is on top flies between its
own place and its partner's on the page beneath. The partner hides under it.
The flight:
- follows the transition's curve, or the finger during an edge swipe;
- cancels its page's movement and fade, so it travels in window space at
  full opacity;
- scales uniformly, by the geometric mean of the width and height ratios.

Limits:
- The hero is not laid out again at each size.
- It stays inside the clips of scroll views around it.
- Tags pair one to one per page.
