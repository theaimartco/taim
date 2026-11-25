# Accessibility in Taim

Accessibility (a11y) is a foundational pillar of the Taim UI framework. This guide details how our accessibility architecture works, how to use it, and why it is critical for building inclusive applications.

## 1. Why Accessibility Matters

Accessibility is not a feature; it is a fundamental human right. Designing for accessibility means designing for everyone.
- **Inclusivity**: Over 1 billion people worldwide have some form of disability. Your application should be usable by everyone, regardless of their visual, motor, auditory, or cognitive abilities.
- **Legal Compliance**: Many regions require digital accessibility by law (e.g., ADA in the US, EAA in the EU).
- **Better UX for All**: Accessibility features often improve the overall user experience. For example, high contrast mode helps users in bright sunlight, and keyboard navigation helps power users navigate quickly.
- **Business Value**: A more accessible application reaches a wider audience, leading to increased user engagement and satisfaction.

## 2. The Semantic Tree

Under the hood, Taim maintains a **Semantic Tree** that runs parallel to the standard Widget Tree. This tree contains all the semantic information required by assistive technologies, stripped of visual and layout details.

### Retained Semantic Forest
The Semantic Tree is a retained structure. It doesn't rebuild from scratch every frame. Instead, it maintains a persistent representation of the UI's semantics, organized as a "Semantic Forest" to handle multiple windows or disparate UI roots seamlessly.

### Incremental Diffs (Change)
When the Widget Tree updates, it emits incremental `Change` objects to the Semantic Tree. This diffing mechanism ensures high performance. Instead of sending the entire tree to the OS accessibility APIs, Taim only sends what changed (e.g., a node's label updated, a checkbox was toggled, or a node was removed).

```rust,ignore
// Example of how changes are propagated internally
pub struct Change {
    pub node_id: NodeId,
    pub action: ChangeAction, // Add, Update, Remove
    pub new_data: Option<SemanticData>,
}
```

### Router, Dispatch, and Rejection
Action routing is handled by the `Router` and `Dispatch` system. When an assistive technology (AT) triggers an action (like "click" or "increment"), the event is dispatched to the corresponding semantic node. If a node cannot handle the action, it can issue a `Rejection`, allowing the framework to fallback or bubble the action appropriately.

## 3. Automatic Semantics

In Taim, accessibility is built-in by default. All standard widgets (buttons, checkboxes, sliders, text inputs) automatically contribute their semantic information to the tree.

For instance, a `button` is announced as a button with its label; you don't tag it yourself. Images and SVGs take a `.label(...)` so they can be announced, and are treated as decorative without one.

```rust
use taim::prelude::*;

fn submit() -> impl View {
    button("Submit").on_press(|| println!("Submitted"))
}
```

A custom view describes itself in `View::describe`, returning a `semantics::Node` keyed by a stable `taim::widget::Id`:

```rust
use taim::view::{DescribeCx, semantics};
use taim::widget;

struct Gauge {
    percent: f32,
    id: widget::Id, // widget::Id::unique(), created once when the view is built
}

impl Gauge {
    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        Some(
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::Image)
                .with_name("Battery")
                .with_description(format!("{:.0} percent", self.percent)),
        )
    }
}
```

[Custom drawing](canvas-drawing.md) shows this inside a complete `View` implementation.

## 4. The Focus System

Focus management is crucial for accessibility, especially for keyboard and screen reader users.

### Focus and Scope
Taim uses a sophisticated focus system.
- **Focus**: The currently active element receiving input.
- **Scope**: A bounded region for focus. Scopes can be used to trap focus within a specific area, like a modal dialog.

### Focus Direction
Navigation through focusable elements has a defined `Direction`:
- `Direction::Forward`: Moving to the next element (e.g., via Tab).
- `Direction::Backward`: Moving to the previous element (e.g., via Shift+Tab).

### Trapping and Restoration
When opening a modal, focus should be trapped within the modal (`Scope`). When the modal closes, focus must be restored to the element that originally triggered it.

Taim's `dialog`, `sheet` and `drawer` do both automatically: while one is open, focus stays inside it, and closing it returns focus to where it was.

## 5. Screen Readers and Platform Adapters

Taim publishes the semantic tree to the platform through its own adapters
in `taim-accessibility` and `taim_winit`.

### Adapter and Capabilities
The `Adapter` acts as a bridge between the internal Semantic Tree and the platform. It queries platform `Capabilities` to determine what accessibility features are supported and active.

### Session Management
A `Session` is established when an AT is detected. This prevents the framework from spending CPU cycles on accessibility translation when no AT is running.

### Supported ATs
- **TalkBack (Android)**: always on; the tree is sent to `TaimActivity`,
  which exposes it as virtual accessibility nodes.
- **Orca (Linux)**: through AT-SPI, with the `accessibility` feature:
  `taim = { git = "...", features = ["accessibility"] }`.
- **VoiceOver (macOS, iOS) and Narrator, NVDA, JAWS (Windows)**: not yet.
  The tree is built on these platforms, but no bridge publishes it to
  their accessibility APIs. This is required for 1.0.

## 6. Live Regions

Live Regions allow the application to announce dynamic changes to the user without requiring them to move focus.

### Announcements and Urgency
When triggering a live region update, you define an `Announcement` and its `Urgency`:
- `Urgency::Polite`: The screen reader will wait until it finishes its current speech before announcing the update.
- `Urgency::Assertive`: The screen reader will interrupt its current speech to announce the update immediately.

In Taim these are `semantics::Live::Polite` and `semantics::Live::Assertive`. `snackbar` announces its message politely by itself. A custom view marks a node as live with `.with_live(...)` in `describe`, and the new text is announced whenever it changes.

### Privacy Considerations
When announcing sensitive data (like passwords or secure tokens), ensure you use appropriate masking or privacy flags in the semantic tree to prevent unintended readout by screen readers in public spaces.

## 7. Keyboard Navigation

Keyboard navigation is fully supported and customizable.
- **Tab**: Moves focus forward.
- **Shift+Tab**: Moves focus backward.
- **Enter/Space**: Activates the focused element (e.g., clicking a button or toggling a checkbox).
- **Arrow Keys**: Navigates within complex widgets like lists, grids, menus, and sliders.

Built-in overlays close on Escape. A custom view handles keys in `View::event`, matching `Event::Keyboard` and returning `Handled::Yes` for keys it uses so they stop there.

## 8. Reduced Motion

Many users experience motion sickness or vestibular disorders triggered by UI animations. Taim respects the OS-level "Reduce Motion" preference.

The `MotionPolicy` defines how animations should behave. If the user prefers reduced motion, animations can be automatically snapped to their end state or replaced with simple crossfades.

Built-in transitions follow the system setting by themselves. An app can also bind it to its own setting:

```rust,no_run
use taim::prelude::*;

fn main() -> taim::Result {
    taim::launch("Motion", |cx| {
        let calm = cx.signal(false);
        cx.reduced_motion(calm);

        switch("Reduce motion", calm)
    })
}
```

## 9. High Contrast

For users with low vision, a high contrast UI is essential. Taim's color system supports a `HighContrast` color scheme alongside `Light` and `Dark`.

When the OS is set to High Contrast mode, the application automatically switches to the `HighContrast` theme, maximizing legibility with stark contrasting colors and pronounced borders.

## 10. Complete Example: An Accessible Form

Every view here describes itself: the field has its placeholder as a name, the switch announces its state, and the dialog keeps focus while open and returns it on close.

```rust
use taim::prelude::*;

fn profile(cx: &mut Scope) -> impl View {
    let name = cx.signal(String::new());
    let newsletter = cx.signal(false);
    let saved = cx.signal(false);

    layers(views![
        column(views![
            text("User profile").size(24),
            text_field("Full name", name),
            switch("Email me the newsletter", newsletter),
            button("Save profile")
                .on_press(move || {
                    saved.set(true);
                })
                .enabled(move || !name.get().trim().is_empty()),
        ])
        .gap(16.0)
        .padding(24.0),
        dialog(
            saved,
            column(views![
                text("Profile saved"),
                button("Close").on_press(move || {
                    saved.set(false);
                }),
            ])
            .gap(12.0),
        ),
    ])
}
```
