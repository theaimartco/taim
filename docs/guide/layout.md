# Layout

Taim lays views out in a single pass. A parent offers each child a range of
sizes (its `Constraints`), the child picks a size, and the parent places
it. Every view also declares its `Sizing` on each axis:

| `Axis` | Meaning |
|---|---|
| `Hug` | As small as its content allows (the default) |
| `Fixed(px)` | Exactly this many logical pixels |
| `Fill(weight)` | A share of the space its siblings leave over |

You rarely touch these types directly: views expose them as builder methods.

## Rows and columns

`column` stacks children top to bottom, `row` left to right.

```rust
use taim::prelude::*;

fn toolbar() -> impl View {
    row(views![
        button("Back").ghost(),
        text("Inbox").size(18),
        space(),
        button("Edit"),
    ])
    .gap(8.0)
    .padding(12.0)
    .align(Align::Center)
    .fill_width()
}
```

| Method | Effect |
|---|---|
| `.gap(px)` | Space between children |
| `.padding(p)` | Space around all children. Takes `f32`, `[v, h]` or `[top, right, bottom, left]` |
| `.align(Align::Start / Center / End)` | Cross-axis alignment: horizontal in a column, vertical in a row |
| `.fill_width()`, `.fill_height()` | Take the space the parent offers on that axis |
| `.push(view)` | Add a child after construction |

`space()` with no size fills the leftover space, which pushes the views
after it to the far end. `space().width(16.0)` or `space().height(16.0)`
is a fixed gap.

## Containers

`container` wraps one child to pad, size, align or decorate it.

```rust
use taim::prelude::*;

fn centered_card() -> impl View {
    container(
        container(text("Centered"))
            .padding(24.0)
            .width(240.0)
            .center(),
    )
    .fill()
    .center()
}
```

| Method | Effect |
|---|---|
| `.width(px)`, `.height(px)` | A fixed size |
| `.fill()`, `.fill_width()`, `.fill_height()` | Take the offered space |
| `.center()`, `.center_x()`, `.center_y()` | Align the child inside |
| `.align_right()`, `.align_bottom()` | Push the child to an edge |
| `.padding(p)` | Inner spacing |

Styling methods (`.background`, `.border`, `.glass`) are in
[Styling](styling.md).

## Grids

`grid(columns, children)` places children into equal-width columns and
wraps onto new rows.

```rust
use taim::prelude::*;

fn photos() -> impl View {
    grid(
        3,
        (1..=9).map(|n| AnyView::of(container(text(format!("{n}"))).padding(20.0))),
    )
    .gap(8.0)
}
```

## Layers

`layers(children)` draws children on top of each other in the same space:
the first is at the back. Overlays such as `dialog` and `snackbar` are
usually placed in a `layers` over the page.

## Scrolling and long lists

`scroll(view)` makes its child scroll when it is taller than the window.
For hundreds or thousands of rows use `list`, which builds only the rows
on screen:

```rust
use taim::prelude::*;

fn inbox(cx: &mut Scope) -> impl View {
    let subjects = cx.signal((1..=10_000).map(|n| format!("Mail {n}")).collect::<Vec<_>>());

    list(move || subjects.with(Vec::len), move |index| {
        let subject = subjects.with(|all| all[index].clone());
        AnyView::of(text(subject).size(15))
    })
    .item_height(44.0)
}
```

## Full screens

On mobile, `scaffold(top, body, bottom, overlays)` places an app bar, a
body that fills the remaining space, and a bottom bar, with overlays drawn
above all three.

```rust
use taim::prelude::*;

fn home(cx: &mut Scope) -> AnyView {
    let tab = cx.signal(0_usize);

    scaffold(
        app_bar("Home", []),
        scroll(text("Content")),
        nav_bar([("⌂", "Home"), ("⚙", "Settings")], tab),
        [],
    )
}
```

## Custom layout

A custom `View` measures itself in `measure` and places its children
through `MeasureCx`. See the built-in `Stack` and `Grid` in
`view/src/views/` for complete examples, and
[Canvas drawing](canvas-drawing.md) for a leaf view.
