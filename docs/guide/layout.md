# Layout & Alignment in Vyom Taim

> **This guide describes the pre-0.15 API** (`Element`, `Message`,
> `Task`, `taim::widget`), which the `taim` crate no longer exports. Its
> code does not compile against this release. For applications, use the
> view API in [Getting started](getting-started.md). This page is kept for
> its concepts while it is rewritten.

Creating beautiful user interfaces requires a robust layout system. Vyom Taim provides a powerful, flexbox-inspired layout engine that gives you complete control over how widgets are sized, positioned, and aligned.

This comprehensive guide covers everything from the basic `Length` type to advanced responsive layout patterns.

---

## 1. The Length Type: Sizing Widgets

The foundation of layout in Taim is the `Length` type, which determines how much space a widget should take up along a particular axis (width or height).

### The Variants of Length

Taim provides several variants of `Length`, each serving a specific layout purpose:

#### `Length::Fill`
The most common variant. It tells the widget to expand and fill all available remaining space in its parent container. If multiple widgets in a row or column use `Fill`, the space is divided equally among them.

```rust
use taim::widget::button;
use taim::Length;

// This button will expand to fill the entire width of its container
button("Submit").width(Length::Fill)
```

#### `Length::FillPortion(u16)`
Similar to `Fill`, but allows you to specify a proportion. If you have two widgets in a row, one with `FillPortion(1)` and the other with `FillPortion(3)`, the first gets 25% of the space, and the second gets 75%.

```rust
use taim::widget::{row, container};
use taim::Length;

// A classic sidebar + content layout
row![
    container("Sidebar").width(Length::FillPortion(1)),
    container("Main Content").width(Length::FillPortion(3)),
]
```

#### `Length::Shrink` (or `Length::Fit`)
Tells the widget to take up only as much space as it needs to display its contents. This is often the default behavior for widgets like `text`.

```rust
use taim::widget::text;
use taim::Length;

// This text widget will be exactly as wide as the string "Hello"
text("Hello").width(Length::Shrink)
```

#### `Length::Fixed(f32)`
Forces the widget to be exactly the specified number of logical pixels wide or tall, regardless of its contents or available space.

```rust
use taim::widget::image;
use taim::Length;

// This image will be exactly 200x200 pixels
image("logo.png")
    .width(Length::Fixed(200.0))
    .height(Length::Fixed(200.0))
```

#### Numeric Auto-Conversion
For convenience, you can pass literal numbers (f32 or u16) to `.width()` and `.height()`, which automatically convert to `Length::Fixed`.

```rust
// These are equivalent
button("Click").width(Length::Fixed(150.0))
button("Click").width(150.0)
button("Click").width(150)
```

### Constraints: Minimum and Maximum
Sometimes you want a widget to fill space, but never get too small or too large. `Length` provides modifier methods for this:

```rust
use taim::widget::text_input;
use taim::Length;

// Fills available space, but never gets smaller than 200px or larger than 600px
text_input("Search...")
    .width(Length::Fill.min(200.0).max(600.0))
```

#### `Length::Bounded` and `Length::Fluid`
For advanced layouts, Taim also supports fluid sizing where bounds are determined dynamically.

---

## 2. Columns: Vertical Layout

The `column` widget stacks its children vertically. It is the primary building block for most application layouts.

```rust
use taim::widget::{column, text, button};

let my_column = column![
    text("Step 1"),
    text("Step 2"),
    button("Continue"),
];
```

### Column Formatting

Columns have numerous methods to format their children:

#### `spacing(f32)`
Adds fixed vertical space between each child widget.
```rust
column![text("A"), text("B")].spacing(20)
```

#### `padding(Padding)`
Adds space around the *inside* edges of the column.
```rust
column![text("Content")].padding(15)
```

#### `align_x(alignment::Horizontal)`
Aligns children horizontally within the column. Options are `Start` (left), `Center`, and `End` (right).
```rust
use taim::alignment;
column![
    text("Left").width(Length::Fill),
].align_x(alignment::Horizontal::Center)
```

#### `push(Widget)` and `extend(Iterator)`
You can dynamically add children to a column instead of using the macro:
```rust
let mut col = column![].spacing(10);
col = col.push(text("First"));

let more_items = vec![text("Second"), text("Third")];
col = col.extend(more_items);
```

#### `wrap()`
If children exceed the height of the column, they will wrap to a new visual column (if `wrap` is enabled).
```rust
column![/* many children */].wrap()
```

---

## 3. Rows: Horizontal Layout

The `row` widget places its children side-by-side horizontally. It shares an almost identical API with `column`, but operates on the X-axis.

```rust
use taim::widget::{row, text, button, Space};

let header = row![
    text("My App Title"),
    Space::with_width(Length::Fill), // Pushes the button to the right
    button("Settings"),
].spacing(10).align_y(taim::alignment::Vertical::Center);
```

### Row Formatting

#### `spacing(f32)`
Adds fixed horizontal space between each child widget.

#### `align_y(alignment::Vertical)`
Aligns children vertically within the row. Options are `Top`, `Center`, and `Bottom`.
```rust
use taim::alignment;
row![
    text("Tall text\nMultiple lines"),
    button("Aligned to bottom"),
].align_y(alignment::Vertical::Bottom)
```

---

## 4. Proportional Layouts (Flex Ratios)

Using `Length::FillPortion` lets you create robust, flexible layouts that maintain proportions regardless of screen size. This is heavily inspired by CSS Flexbox `flex-grow`.

### Example: Dashboard Layout

```rust
use taim::widget::{row, column, container, text};
use taim::Length::FillPortion;

let dashboard = row![
    // Sidebar takes up 1 unit of space (20%)
    container(
        column![
            text("Navigation"),
            text("Dashboard"),
            text("Settings"),
        ].spacing(10)
    ).width(FillPortion(1)),

    // Main Content takes up 4 units of space (80%)
    container(
        text("Main Dashboard Area")
    ).width(FillPortion(4)),
];
```

---

## 5. The Container Widget: Alignment and Padding

The `container` widget wraps a single child widget. It is used for styling (backgrounds, borders) and, crucially, for **alignment**.

By default, widgets like `text` just take up the space they need. If you want to center text on the screen, you put it in a container that fills the screen and center the contents.

### Centering Methods

Taim's `container` provides expressive methods for alignment. They all take a `Length` argument (usually `Length::Fill` or a fixed size) which defines the size of the container itself before aligning its child.

#### `center(Length)`
Centers the child both vertically and horizontally.
```rust
// Centers "Hello" perfectly in the middle of the available space
container(text("Hello"))
    .center(Length::Fill)
```

#### `center_x(Length)` and `center_y(Length)`
Centers only on a specific axis.
```rust
container(text("Centered Horizontally"))
    .center_x(Length::Fill)
    .width(Length::Fill)
```

#### `align_left(Length)` / `align_right(Length)`
Aligns the child to the left or right edge of the container.
```rust
container(text("Pushed Right"))
    .align_right(Length::Fill)
```

#### `align_top(Length)` / `align_bottom(Length)`
Aligns the child to the top or bottom edge.

---

## 6. Padding

Padding is space inside a widget, between its borders and its content. Taim uses a `Padding` struct that provides immense flexibility.

Many widgets accept padding via a `.padding()` method.

### Ways to specify padding:

**1. Single Value (Uniform)**
Applies the same padding to all four sides.
```rust
use taim::Padding;
// 10px on top, bottom, left, and right
container("Text").padding(10) 
```

**2. Two Values (Vertical, Horizontal)**
First value is Top/Bottom, second is Left/Right.
```rust
// 10px vertical, 20px horizontal
container("Text").padding([10, 20])
```

**3. Four Values (Top, Right, Bottom, Left)**
CSS-style clockwise specification.
```rust
container("Text").padding([10, 15, 20, 5])
```

**4. Builder Pattern**
For maximum clarity, you can construct `Padding` manually.
```rust
let pad = Padding::new(10.0).top(25.0).left(5.0);
container("Text").padding(pad)
```

---

## 7. Scrollable Layouts

When content exceeds the available screen space, you need a `scrollable` widget.

```rust
use taim::widget::{scrollable, column, text};

let long_content = column![
    text("Item 1"),
    // ... 100 more items ...
    text("Item 100"),
];

let scroller = scrollable(long_content)
    .direction(scrollable::Direction::Vertical);
```

### Scrollable Features

- **Directions**: `Vertical`, `Horizontal`, or `Both`.
- **Customization**:
  - `scrollbar_width(f32)`: Thickness of the scrollbar track.
  - `scrollbar_margin(f32)`: Space between scrollbar and content.
  - `scroller_width(f32)`: Thickness of the thumb (the part you drag).
  - `hidden()`: Hide the scrollbars visually but still allow scrolling.
- **Callbacks**: `on_scroll(Message)` fires when the user scrolls, passing the current offset.
- **Anchoring**: `anchor_bottom()` is useful for chat apps to automatically scroll to the newest message.

---

## 8. VirtualList (High-Performance Scrolling)

If you have thousands of items to display, `scrollable` will try to render all of them, which kills performance. Instead, use `VirtualList`.

`VirtualList` only renders the items currently visible on screen (plus a small buffer).

```rust
use taim::widget::VirtualList;
// ... VirtualList API ...
```

**WARNING**: Never put a `VirtualList` inside a `Scrollable` or a `Column` that is not bounded. The `VirtualList` needs to know its exact dimensions to calculate which items are visible. If you put it in a Scrollable, it assumes infinite height and renders everything, defeating its purpose.

---

## 9. Stack: Z-Axis Layering

The `Stack` layout allows you to place widgets on top of each other in the Z-axis. This is essential for overlays, badges, tooltips, or floating action buttons.

```rust
use taim::widget::{stack, image, container, text};
use taim::alignment;

let profile_pic_with_badge = stack![
    image("avatar.png").width(100).height(100),
    
    // The badge overlay
    container(text("99+").size(10))
        .padding(5)
        .style(container::danger) // Red background
        .align_right(Length::Fill)
        .align_top(Length::Fill)
];
```

The first widget in the stack is at the bottom (Z-index 0). Subsequent widgets are drawn on top.

---

## 10. Grid Layout

For multi-dimensional layouts, Taim provides a `Grid` layout (if enabled via features).

```rust
use taim::widget::grid;
// ... Grid example ...
```

---

## 11. Responsive Design

To make layouts change based on screen size, use the `responsive` widget. It takes a closure that receives the available `Size` and returns a widget.

```rust
use taim::widget::{responsive, row, column, text};
use taim::Size;

let dynamic_layout = responsive(|size: Size| {
    if size.width > 800.0 {
        // Desktop: Side-by-side
        row![
            text("Sidebar"),
            text("Main Content")
        ].into()
    } else {
        // Mobile: Stacked
        column![
            text("Sidebar"),
            text("Main Content")
        ].into()
    }
});
```

---

## 12. Adaptive Layouts

Sometimes you want to adapt based on window classification (e.g., is it a compact mobile screen or an expanded tablet screen?) rather than exact pixel sizes. Taim integrates window classes for this purpose.

---

## 13. SafeArea

When targeting mobile platforms (iOS, Android), devices often have physical notches, dynamic islands, or software home bars that obscure parts of the screen.

Wrap your top-level layout in a `SafeArea` widget to automatically add padding where necessary to ensure your content is always visible.

---

## 14. Common Layout Patterns

### 1. Header, Body, Footer

```rust
use taim::widget::{column, container, text};
use taim::Length;

column![
    container(text("Header")).height(60).center(Length::Fill),
    container(text("Main Body"))
        .height(Length::Fill) // Body takes all remaining space
        .center(Length::Fill),
    container(text("Footer")).height(40).center(Length::Fill),
]
```

### 2. Form Layout

```rust
use taim::widget::{column, text_input, button, text};

column![
    text("Login").size(32),
    
    column![
        text("Email"),
        text_input("Enter email..."),
    ].spacing(5),
    
    column![
        text("Password"),
        text_input("Enter password...").password(),
    ].spacing(5),
    
    button("Sign In").width(Length::Fill),
].spacing(20).padding(40).max_width(400)
```
