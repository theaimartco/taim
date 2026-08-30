# Animations in Vyom-Taim

Animations are a critical component of any modern user interface. In Vyom-Taim, the `taim-motion` crate provides a robust, physics-based animation system that makes it easy to build fluid, responsive, and interruptible UI experiences.

## 1. Physics-based vs. Duration-based Animations

### The Problem with Duration

Traditionally, UI animations have been duration-based. You tell an object to move from A to B over 300 milliseconds. While this works for simple transitions, it breaks down quickly in interactive applications:

- **Interruptibility**: If a user interrupts a 300ms animation halfway through (e.g., closing a menu that is currently opening), what happens? A duration-based animation must either snap to the end, instantly reverse (causing a jarring velocity change), or calculate a complex path.
- **Velocity Matching**: When a user flings a scroll view and it hits the boundary, a duration-based "bounce" has no awareness of the scroll's initial velocity. It looks fake.
- **Complexity**: Chaining multiple duration-based animations with different easing curves to simulate real-world motion is tedious and brittle.

### The Physics-based Solution

`taim-motion` prioritizes physics-based animations, specifically using **Springs**. Instead of defining how long an animation takes, you define the physical properties of the motion: its stiffness and damping.

- **Continuous Velocity**: Physics engines maintain velocity across interruptions. If a user grabs a moving element and flings it back, the momentum smoothly transitions.
- **Natural Feel**: Real-world objects don't move with perfectly cubic easing curves over fixed durations. They have mass, and they are affected by friction and tension.
- **Less Math for You**: You don't need to calculate durations based on distance. A spring naturally takes longer to travel a greater distance.

---

## 2. Spring Animations Deep Dive

The `Spring` is the workhorse of `taim-motion`. It is defined by a `SpringConfig`.

### `SpringConfig`

A `SpringConfig` takes three primary parameters:

1.  **`angular_frequency` ($\omega$)**: This determines the speed or stiffness of the spring. A higher value means the spring moves faster toward its target. Think of it as how tight the spring is coiled.
2.  **`damping_ratio` ($\zeta$)**: This controls how the spring settles.
    -   **Underdamped ($\zeta < 1.0$)**: The spring will overshoot its target and oscillate (bounce) before coming to rest. Lower values mean more bouncing.
    -   **Critically Damped ($\zeta = 1.0$)**: The spring will return to its target as fast as possible without overshooting. This is the default and most common setting for UI interactions, providing a snappy but smooth feel.
    -   **Overdamped ($\zeta > 1.0$)**: The spring will slowly creep toward its target without overshooting. It feels sluggish, like moving through molasses.
3.  **`epsilon` ($\epsilon$)**: The resting threshold. Since mathematically a spring never truly stops moving (it just gets infinitely small), `epsilon` defines the minimum velocity and displacement at which the animation is considered "finished."

### Example: Configuring a Spring

```rust
use taim_motion::{Spring, SpringConfig};

// A bouncy spring
let bouncy_spring = Spring::new(
    SpringConfig {
        angular_frequency: 15.0,
        damping_ratio: 0.6, // < 1.0 means it bounces
        epsilon: 0.001,
    },
    0.0, // initial value
    100.0, // target value
);

// A smooth, critically damped spring for UI panels
let panel_spring = Spring::new(
    SpringConfig {
        angular_frequency: 20.0,
        damping_ratio: 1.0, // Critically damped
        epsilon: 0.001,
    },
    0.0,
    1.0,
);
```

### Interruptibility in Action

When a spring animation's target is changed while it is still running, the `Spring` simply updates its target value. The current position and velocity are maintained, and the physics simulation continues seamlessly towards the new target. This is handled automatically by the engine.

---

## 3. Tween Animations

While springs are preferred, there are times when you need a precise, duration-based animation—for example, a pulsing opacity effect or a loading spinner. For these, `taim-motion` provides the `Tween` API.

### Easing Curves

A `Tween` moves from a start value to an end value over a specific duration, modified by an `Easing` function. `taim-motion` provides a comprehensive suite of standard easing curves:

-   **`Linear`**: Constant speed.
-   **Quadratic (`QuadIn`, `QuadOut`, `QuadInOut`)**: Accelerates or decelerates using a power of 2. Good for simple hovers.
-   **Cubic (`CubicIn`, `CubicOut`, `CubicInOut`)**: Accelerates or decelerates using a power of 3. Standard UI easing.
-   **Elastic (`ElasticIn`, `ElasticOut`, `ElasticInOut`)**: Over-shoots the target simulating an elastic band.

### Custom Cubic Bezier

If the standard curves don't meet your needs, you can define a custom cubic bezier curve, familiar to CSS developers.

```rust
use taim_motion::{Tween, Easing, CubicBezier};
use std::time::Duration;

let custom_tween = Tween::new(
    0.0,
    1.0,
    Duration::from_millis(300),
    Easing::Custom(CubicBezier::new(0.25, 0.1, 0.25, 1.0)) // Equivalent to CSS 'ease'
);
```

---

## 4. Decay Animations

A `Decay` animation is used when a user imparts velocity to an object and then lets go. The object continues to move, slowing down due to friction, until it stops or hits a boundary.

### Momentum Scrolling

The primary use case for `Decay` is momentum scrolling. When you flick a list, it doesn't stop immediately; it coasts.

### Properties

-   **`velocity`**: The initial speed imparted by the user's gesture (in logical pixels per second).
-   **`friction`**: How quickly the velocity diminishes. Higher friction means a shorter coasting distance.
-   **`Bounds`**: (Optional) Minimum and maximum values. If the decaying value hits a bound, it can either stop immediately or, if configured with a spring, bounce back.

```rust
use taim_motion::{Decay, DecayConfig};

let scroll_momentum = Decay::new(
    DecayConfig {
        friction: 0.998,
    },
    1500.0, // High initial velocity from a flick gesture
);
```

---

## 5. Timeline Animations

For complex, choreographed sequences, `taim-motion` offers the `Timeline` API. A `Timeline` allows you to chain multiple animations (Springs, Tweens, or pauses) together.

### Segments

A `Timeline` is composed of `Segment`s. Each segment defines an animation and a relative start time or sequence order.

### Direction

Timelines can be played in different directions:

-   **`Forward`**: Plays from start to finish.
-   **`Reverse`**: Plays from finish to start.
-   **`Alternate`**: Plays forward, then reverses, repeating as configured.

```rust
use taim_motion::{Timeline, Tween, Easing};
use std::time::Duration;

let mut timeline = Timeline::new();
timeline.add_segment(Tween::new(0.0, 1.0, Duration::from_millis(200), Easing::Linear));
timeline.add_segment(Tween::new(1.0, 0.5, Duration::from_millis(100), Easing::QuadOut));

timeline.play(Direction::Alternate);
```

---

## 6. PageTransition

Navigating between screens should feel fluid. `taim-motion` includes built-in `PageTransition` types that can be applied to your router or navigation stack.

-   **`FORWARD`**: The standard iOS-style push. The new page slides in from the right while the old page slides out to the left slightly and fades.
-   **`BACK`**: The reverse of `FORWARD`.
-   **`ZOOM`**: The standard Android-style transition. The new page scales up from the center while fading in.
-   **`SpringSlide`**: A physics-based slide transition that uses a critically damped spring for a snappier feel.
-   **`None`**: Instantaneous swap, no animation.

---

## 7. The Transition Widget

The easiest way to use animations in your UI tree is the `Transition` widget. It allows you to declaratively map an animation value to widget properties.

```rust
use taim_ui::widgets::{Transition, Container, Text};
use taim_motion::Spring;

// A simple hover transition
Transition::new(
    is_hovered, // Boolean trigger
    Spring::default(), // The animation to run when the trigger changes
    |anim_value, context| {
        // anim_value interpolates between 0.0 and 1.0 based on is_hovered
        let scale = 1.0 + (anim_value * 0.1); // Scale from 1.0 to 1.1

        Container::new(Text::new("Hover Me"))
            .scale(scale)
    }
)
```

---

## 8. SharedElement Transitions (Hero Animations)

`SharedElement` transitions create a seamless connection between two distinct screens by animating an element that appears on both screens from its starting position and size to its final position and size.

### `SharedElementId`

To use this, you wrap the target widget on both screens with a `SharedElement` widget and give them a matching `SharedElementId`.

```rust
// On Screen A (List View)
SharedElement::new(
    SharedElementId::new("avatar_user_123"),
    Image::new("user_123.png").size(40, 40)
)

// On Screen B (Detail View)
SharedElement::new(
    SharedElementId::new("avatar_user_123"),
    Image::new("user_123.png").size(200, 200)
)
```

When navigating from Screen A to Screen B, `taim-motion` automatically captures the bounding box of the element on Screen A, creates an overlay, and animates it to the bounding box of the element on Screen B.

---

## 9. Reduced Motion and Accessibility

Animations can cause discomfort or nausea for users with vestibular disorders. `taim-motion` respects system-level accessibility settings.

### `MotionPolicy`

The `MotionPolicy` singleton determines whether animations should run.

-   **OS Detection**: On startup, `taim-motion` queries the host OS (macOS, Windows, Linux, iOS, Android) to check if "Prefers Reduced Motion" is enabled.
-   **Automatic Override**: If reduced motion is preferred, all `Spring`, `Tween`, and `Decay` animations will instantly snap to their target values, completing in zero time.
-   **Manual Control**: You can explicitly set the `MotionPolicy` if you need to override the system setting (e.g., for an in-app toggle).

```rust
use taim_motion::MotionPolicy;

// Check current policy
if MotionPolicy::get().allows_motion() {
    // Run complex animation
}
```

---

## 10. Complete Example: Animated Card

Here is a complete example tying together state, springs, and the `Transition` widget to create an expandable card.

```rust
use taim_ui::prelude::*;
use taim_motion::{Spring, SpringConfig};

#[derive(Default, Clone, State)]
struct CardState {
    expanded: bool,
}

fn build_card(state: &CardState, cx: &mut Context) -> impl Widget {
    let expanded = state.expanded;

    // Use a spring with a slight bounce
    let spring_config = SpringConfig {
        angular_frequency: 18.0,
        damping_ratio: 0.8,
        epsilon: 0.001,
    };

    Transition::new(
        expanded,
        Spring::with_config(spring_config),
        move |anim_val, _cx| {
            // anim_val is 0.0 when collapsed, 1.0 when expanded

            // Interpolate height: 100px to 300px
            let current_height = 100.0 + (anim_val * 200.0);

            // Interpolate opacity for the detail text
            let detail_opacity = anim_val;

            Container::new(
                Column::new()
                    .push(Text::new("Tap to Expand").font_size(18.0))
                    .push(
                        Text::new("Here are the expanded details. They fade in smoothly as the card grows.")
                            .opacity(detail_opacity)
                    )
            )
            .height(current_height)
            .padding(16.0)
            .background_color(Color::rgb(240, 240, 240))
            .corner_radius(8.0)
            .on_tap(move |cx| {
                // Toggle state
                cx.update_state(|s: &mut CardState| s.expanded = !s.expanded);
            })
        }
    )
}
```
This comprehensive approach ensures your UI feels alive, responsive, and respectful of user preferences.
