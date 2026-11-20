# Accessibility in Vyom-Taim

Accessibility (a11y) is a foundational pillar of the Vyom-Taim UI framework. This guide details how our accessibility architecture works, how to use it, and why it is critical for building inclusive applications.

## 1. Why Accessibility Matters

Accessibility is not a feature; it is a fundamental human right. Designing for accessibility means designing for everyone.
- **Inclusivity**: Over 1 billion people worldwide have some form of disability. Your application should be usable by everyone, regardless of their visual, motor, auditory, or cognitive abilities.
- **Legal Compliance**: Many regions require digital accessibility by law (e.g., ADA in the US, EAA in the EU).
- **Better UX for All**: Accessibility features often improve the overall user experience. For example, high contrast mode helps users in bright sunlight, and keyboard navigation helps power users navigate quickly.
- **Business Value**: A more accessible application reaches a wider audience, leading to increased user engagement and satisfaction.

## 2. The Semantic Tree

Under the hood, Vyom-Taim maintains a **Semantic Tree** that runs parallel to the standard Widget Tree. This tree contains all the semantic information required by assistive technologies, stripped of visual and layout details.

### Retained Semantic Forest
The Semantic Tree is a retained structure. It doesn't rebuild from scratch every frame. Instead, it maintains a persistent representation of the UI's semantics, organized as a "Semantic Forest" to handle multiple windows or disparate UI roots seamlessly.

### Incremental Diffs (Change)
When the Widget Tree updates, it emits incremental `Change` objects to the Semantic Tree. This diffing mechanism ensures high performance. Instead of sending the entire tree to the OS accessibility APIs, Vyom-Taim only sends what changed (e.g., a node's label updated, a checkbox was toggled, or a node was removed).

```rust
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

In Vyom-Taim, accessibility is built-in by default. All standard widgets (buttons, checkboxes, sliders, text inputs) automatically contribute their semantic information to the tree.

For instance, when you use a `Button` widget, you don't need to manually tag it as a button. The framework handles it.

```rust
// Automatic semantics out of the box
Button::new("Submit")
    .on_click(|| println!("Clicked!"))
```

However, if you build a custom widget, you can manually inject semantics using the `.semantics()` modifier.

```rust
MyCustomWidget::new()
    .semantics(|s| {
        s.role(Role::Button)
         .label("Custom Action")
         .disabled(false);
    })
```

## 4. The Focus System

Focus management is crucial for accessibility, especially for keyboard and screen reader users.

### Focus and Scope
Vyom-Taim uses a sophisticated focus system.
- **Focus**: The currently active element receiving input.
- **Scope**: A bounded region for focus. Scopes can be used to trap focus within a specific area, like a modal dialog.

### Focus Direction
Navigation through focusable elements has a defined `Direction`:
- `Direction::Forward`: Moving to the next element (e.g., via Tab).
- `Direction::Backward`: Moving to the previous element (e.g., via Shift+Tab).

### Trapping and Restoration
When opening a modal, focus should be trapped within the modal (`Scope`). When the modal closes, focus must be restored to the element that originally triggered it.

```rust
// Example of focus trapping
Dialog::new()
    .trap_focus(true)
    .restore_focus(true)
```

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

```rust
// Announcing a dynamic change
LiveRegion::announce("File uploaded successfully", Urgency::Polite);
```

### Privacy Considerations
When announcing sensitive data (like passwords or secure tokens), ensure you use appropriate masking or privacy flags in the semantic tree to prevent unintended readout by screen readers in public spaces.

## 7. Keyboard Navigation

Keyboard navigation is fully supported and customizable.
- **Tab**: Moves focus forward.
- **Shift+Tab**: Moves focus backward.
- **Enter/Space**: Activates the focused element (e.g., clicking a button or toggling a checkbox).
- **Arrow Keys**: Navigates within complex widgets like lists, grids, menus, and sliders.

Developers can intercept or augment keyboard events using the `on_key` modifier.

```rust
widget.on_key(|event| {
    if event.key == Key::Escape {
        close_dialog();
        EventResult::Handled
    } else {
        EventResult::Ignored
    }
})
```

## 8. Reduced Motion

Many users experience motion sickness or vestibular disorders triggered by UI animations. Vyom-Taim respects the OS-level "Reduce Motion" preference.

The `MotionPolicy` defines how animations should behave. If the user prefers reduced motion, animations can be automatically snapped to their end state or replaced with simple crossfades.

```rust
// Example of motion policy checking
if env.motion_policy() == MotionPolicy::Reduced {
    // Skip animation
    snap_to_state();
} else {
    // Play animation
    play_spring_animation();
}
```

## 9. High Contrast

For users with low vision, a high contrast UI is essential. Vyom-Taim's color system supports a `HighContrast` color scheme alongside `Light` and `Dark`.

When the OS is set to High Contrast mode, the application automatically switches to the `HighContrast` theme, maximizing legibility with stark contrasting colors and pronounced borders.

## 10. Complete Example: Accessible Form with Dialog Focus Trapping

Here is a comprehensive example demonstrating accessibility in action, including a form and a modal dialog with focus management and live regions.

```rust
use taim::prelude::*;

pub fn accessible_form_example() -> impl Widget {
    let show_dialog = use_state(|| false);
    let name = use_state(|| String::new());

    Column::new()
        .spacing(16.0)
        .push(
            Text::new("User Profile")
                .semantics(|s| s.heading_level(1))
        )
        .push(
            TextInput::new(name.clone())
                .label("Full Name")
                .placeholder("Enter your full name")
                // Semantics are automatically handled by TextInput
        )
        .push(
            Button::new("Save Profile")
                .on_click(move || {
                    show_dialog.set(true);
                })
        )
        .push_if(show_dialog.get(), || {
            Dialog::new()
                .trap_focus(true)
                .restore_focus(true)
                .content(
                    Column::new()
                        .push(Text::new("Profile Saved!").semantics(|s| s.live_region(Urgency::Assertive)))
                        .push(
                            Button::new("Close")
                                .on_click(move || show_dialog.set(false))
                        )
                )
        })
}
```

This example demonstrates how easy it is to build accessible UIs with Vyom-Taim. The framework handles the heavy lifting, allowing you to focus on creating great experiences for all users.
