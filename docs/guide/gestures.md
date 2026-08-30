# Gestures in Vyom-Taim

User interaction on modern devices is dominated by touch and precise pointer events. The `taim-gesture` crate provides a sophisticated gesture recognition system built around the concept of a **Gesture Arena**. This ensures that complex, overlapping gestures are resolved predictably and smoothly. Whether you're building for desktop with a mouse, or a mobile device with multi-touch, the gesture system provides a unified abstraction.

## 1. Why a Gesture Arena?

Imagine a complex UI: You have a horizontal carousel of images, and each image can be tapped. This carousel is placed inside a vertical scrolling list. 

When the user puts their finger on an image and moves it diagonally, what should happen?
- Should the image trigger a tap?
- Should the horizontal carousel scroll?
- Should the vertical list scroll?
- What if the user uses two fingers?

This is the **competing recognizers problem**. If every widget independently evaluated pointer events, chaotic behavior would ensue. The vertical list might start scrolling while the horizontal carousel also stutters along, and the tap might fire right as the user lifts their finger, leading to unintended navigations.

### The Arena Solution

The Gesture Arena solves this by acting as a centralized mediator. Instead of widgets instantly acting on pointer events, they submit **Recognizers** to the Arena. The Arena feeds pointer events to all active recognizers simultaneously. The recognizers analyze the data and report back their state. The Arena then decides which single recognizer "wins," ensuring that only one primary gesture is executed at a time. This guarantees that scrolling a list won't accidentally tap a button inside it.

---

## 2. Arena Lifecycle: Add -> Track -> Resolve

The lifecycle of a gesture through the Arena involves several distinct phases:

1.  **Add (Pointer Down)**: When a primary pointer (e.g., finger touches screen, mouse button pressed) hits a widget, the UI framework's event router identifies all widgets under that pointer. These widgets create their respective `Recognizer` instances and add them to the Arena. At this stage, all recognizers are in a `Possible` state.
2.  **Track (Pointer Move)**: As the pointer moves, the Arena distributes these `Move` events to all recognizers. Each recognizer updates its internal state (distance moved, velocity, duration).
3.  **Resolve (The Battle)**:
    -   A recognizer can declare **Defeat** (`Rejected`) if the movement violates its rules (e.g., a `TapRecognizer` rejects the gesture if the finger moves beyond a 10-pixel threshold).
    -   A recognizer can declare **Victory** (`Accepted`) if the movement perfectly matches its criteria (e.g., a `SwipeRecognizer` accepts the gesture once the velocity and distance thresholds are met, before a pan recognizer might claim it).
    -   If all recognizers but one reject the gesture, the final remaining recognizer wins by default, even if it hasn't explicitly accepted yet (this is called "winning by default").
    -   When a winner is declared, all other recognizers are forced to `reset()`, and all subsequent pointer events for that specific touch sequence are routed *exclusively* to the winner.

---

## 3. The `Recognizer` Trait

Every gesture in `taim-gesture` implements the `Recognizer` trait. This trait defines the core contract between the gesture logic and the Arena. You can implement this trait to create entirely custom gestures.

```rust
pub trait Recognizer {
    /// Receive a pointer event and update internal state.
    /// This is where the recognizer analyzes movement and timing.
    fn event(&mut self, event: &PointerEvent);

    /// The current verdict of the recognizer.
    fn disposition(&self) -> Disposition;

    /// Called by the arena to accept this recognizer as the winner.
    /// The recognizer should trigger its success callback here.
    fn accept(&mut self);

    /// Called by the arena to reject this recognizer.
    /// The recognizer should clean up any visual feedback it may have started.
    fn reject(&mut self);

    /// Reset internal state for the next gesture sequence.
    fn reset(&mut self);
}

pub enum Disposition {
    /// Still tracking, could go either way. Waiting for more events.
    Possible, 
    /// I claim this gesture! (E.g., distance threshold crossed).
    Accepted, 
    /// I cannot process this gesture. (E.g., moved too far for a tap).
    Rejected, 
}
```

---

## 4. Built-in Recognizers

`taim-gesture` provides a rich set of built-in recognizers for common interactions, configurable to suit different needs.

### `TapRecognizer`
Detects a brief pointer down and up without significant movement.
-   **Config**: `max_duration` (e.g., 300ms), `max_movement` (e.g., 10 logical pixels).
-   **Behavior**: Rejects if pointer moves beyond `max_movement` or if the pointer stays down longer than `max_duration`.

### `LongPressRecognizer`
Detects a pointer down that remains stationary for a specific duration.
-   **Config**: `min_duration` (e.g., 500ms), `max_movement` (e.g., 10 pixels).
-   **Behavior**: Accepts immediately when `min_duration` is reached.

### `PanRecognizer`
Detects continuous movement in any direction. Often used for dragging items or freeform canvas panning.
-   **Config**: `min_distance` (distance before it claims the gesture).
-   **Behavior**: Accepts once `min_distance` is crossed. Provides continuous deltas.

### `SwipeRecognizer`
Detects quick, linear movements. Distinct from Pan because it focuses on velocity rather than just continuous tracking.
-   **Config**: `min_velocity` (pixels/second), `min_distance`, `direction` (Up, Down, Left, Right).
-   **Behavior**: Accepts upon release if velocity and distance criteria are met in the specified direction.

### `EdgeSwipeRecognizer`
Similar to Swipe, but only triggers if the gesture originates from the edge of the screen. Crucial for system-level navigation (like iOS "back" swipe) or side drawers.
-   **Config**: `edge_width` (how close to the edge it must start).
-   **Behavior**: Rejects immediately on `PointerDown` if not within `edge_width`.

### `PinchRecognizer`
Detects two pointers moving closer together or further apart.
-   **Config**: `min_scale_delta`.
-   **Behavior**: Tracks the distance between two pointers and provides a scaling factor.

### `RotateRecognizer`
Detects two pointers rotating around a central axis.
-   **Config**: `min_angle_delta`.
-   **Behavior**: Calculates the angle between two pointers and tracks changes.

---

## 5. Velocity Tracking

Many physics-based UI interactions (like momentum scrolling or flinging a card) rely on knowing exactly how fast the pointer was moving when it was released.

`taim-gesture` includes a robust `VelocityTracker`. It uses a least-squares polynomial fit over the last few milliseconds of pointer data (typically the last 100ms) to calculate a smooth, accurate velocity vector. This is essential because digitizer hardware often reports slightly jittery coordinates. A simple `(current - previous) / time` calculation would yield highly erratic velocities.

Recognizers like `Pan` and `Swipe` utilize this internally and provide the final calculated velocity in their callback events, which you can then plug directly into an `taim-motion` `Decay` animation.

---

## 6. NestedScroll Coordination

The `NestedScroll` system is a specialized coordination mechanism built on top of the gesture arena. It handles the extremely common but complex scenario of a scrolling view inside another scrolling view (e.g., a multi-line text input inside a scrolling page, or a horizontal row inside a vertical list).

### Rules of NestedScroll

When a user drags inside a nested scroll view, the system must decide which view gets the scroll delta. 
1.  **Child First (Pre-Scroll)**: The inner child scroll view is given the opportunity to consume the drag delta first.
2.  **Parent Overflow**: If the child hits its scroll boundary (e.g., it reaches the absolute bottom), it cannot consume any more delta in that direction. It passes the remaining unconsumed drag delta up to the parent.
3.  **Momentum Transfer (Fling)**: If the child is flung (swiped) and hits its boundary while still moving, the remaining velocity is transferred to the parent. This causes the parent to seamlessly take over the momentum scroll, feeling like a single continuous physical motion.
4.  **Overscroll**: If the parent also hits its boundary, the velocity can be used to drive an overscroll bounce effect (using an `taim-motion` Spring).

---

## 7. Event Flow Architecture

Understanding how an OS event turns into a UI change is critical for debugging complex gesture issues. The flow is as follows:

1.  **`winit`**: The underlying windowing library (e.g., `winit` on desktop) receives an OS-level mouse or touch event.
2.  **`hit_test`**: The UI engine traverses the render tree to find which widgets are under the pointer coordinates. This generates a list of targets from front to back.
3.  **`route_pointer`**: The engine dispatches a `PointerDown` event to those widgets in the hit test path.
4.  **`arena`**: The widgets instantiate their configured `Recognizer`s and add them to the global `Arena`.
5.  **`recognizer`**: Subsequent `PointerMove` and `PointerUp` events are fed directly to the active `Recognizer`s by the arena.
6.  **`resolution`**: The arena resolves the winner based on the rules described in Section 2.
7.  **`widget callback`**: Once the Arena declares a winner, the winning `Recognizer` fires its callback (e.g., `on_swipe`), executing the user-defined closure attached to the `Widget`. This closure typically updates the application state, triggering a re-render.

---

## 8. Complete Example: Swipe-to-Dismiss

Here is how you might implement a swipe-to-dismiss interaction for a list item using the provided recognizers.

```rust
use taim_ui::prelude::*;
use taim_gesture::{SwipeRecognizer, Direction};

fn swipeable_card(id: String, content: String, cx: &mut Context) -> impl Widget {
    // We attach a raw recognizer rather than using a high-level helper like on_tap
    Container::new(Text::new(content))
        .padding(16.0)
        .background_color(Color::WHITE)
        .corner_radius(8.0)
        .shadow(Shadow::new().blur(4.0).color(Color::rgba(0, 0, 0, 0.1)))
        .add_recognizer(
            SwipeRecognizer::new()
                .direction(Direction::Left | Direction::Right) // Allow horizontal swipe in either direction
                .min_velocity(500.0) // Require a decently fast flick (500 pixels per second)
                .min_distance(50.0) // Require at least 50 pixels of travel
                .on_swipe(move |event, cx| {
                    // This closure executes ONLY if the SwipeRecognizer wins the arena.
                    // If a PanRecognizer won, or a vertical scroll won, this would not execute.
                    println!("Swiped card {} with velocity {}", id, event.velocity);
                    
                    // Dispatch an action to remove this item from the data model.
                    // The UI will re-render without this card.
                    cx.dispatch(RemoveItemAction(id.clone()));
                })
        )
}
```

---

## 9. Complete Example: Pinch-to-Zoom

Implementing pinch-to-zoom involves tracking the scale delta provided by the `PinchRecognizer` and applying it to a render transform.

```rust
use taim_ui::prelude::*;
use taim_gesture::PinchRecognizer;

#[derive(Default, Clone, State)]
struct ImageState {
    scale: f32,
    base_scale: f32,
}

impl ImageState {
    fn new() -> Self { Self { scale: 1.0, base_scale: 1.0 } }
}

fn zoomable_image(url: &str, state: &ImageState, cx: &mut Context) -> impl Widget {
    let current_scale = state.scale;

    Image::new(url)
        .scale(current_scale) // Apply current scale to the widget's render transform
        .add_recognizer(
            PinchRecognizer::new()
                // on_start is called when the gesture is first recognized (Accepted)
                .on_start(move |_event, cx| {
                    cx.update_state(|s: &mut ImageState| {
                        s.base_scale = s.scale;
                    });
                })
                // on_update is called continuously as the fingers move
                .on_update(move |event, cx| {
                    // event.scale_delta is relative to the start of the gesture
                    // (e.g., 1.1 means fingers are 10% further apart than when they first touched)
                    
                    cx.update_state(|s: &mut ImageState| {
                        // Multiply the base scale by the delta to avoid compounded floating point errors
                        let new_scale = (s.base_scale * event.scale_delta).clamp(1.0, 5.0);
                        s.scale = new_scale;
                    });
                })
                // on_end is called when fingers are lifted
                .on_end(move |_event, cx| {
                    println!("Finished zooming. Final scale: {}", current_scale);
                })
        )
}
```

---

## 10. Dealing with Conflicts and Debugging

When designing complex UIs, conflicting gestures are inevitable. The Arena works beautifully, but you as the developer still have to ensure reasonable configurations.

For example, placing a `SwipeRecognizer` (which requires a fast flick) directly on top of a `PanRecognizer` (which activates on any movement) might result in the `PanRecognizer` always winning before the user's swipe velocity reaches the swipe threshold. 

To resolve such scenarios, carefully tune `min_velocity` and `min_distance` properties on your recognizers, ensuring the specific interaction you want to occur takes precedence when appropriate. Testing on real devices is always highly recommended.

Additionally, `taim-gesture` provides a debug mode that visually indicates which recognizers are active and which one won the arena, which can be invaluable when fine-tuning these interactions. You can enable it via `taim_gesture::set_debug_mode(true)`.

### Common Conflict Scenarios
1.  **Horizontal Scroll inside a Vertical Scroll**: Usually handled correctly by `NestedScroll` constraints if configured. However, make sure horizontal `PanRecognizer` requires enough horizontal distance to prevent diagonal scrolling from accidentally grabbing the horizontal gesture if vertical was intended.
2.  **Double Tap vs Single Tap**: You may have a delay required before `TapRecognizer` fires if a `DoubleTapRecognizer` is also in the arena for the same widget.
3.  **Long Press vs Pan**: By default, `LongPress` requires no movement. If a user moves their finger even slightly while trying to long press, `Pan` might win if `max_movement` is set too low for `LongPress`. Give it enough buffer.
