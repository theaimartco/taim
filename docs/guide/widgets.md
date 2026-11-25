# Views

Everything on screen in Taim is a view. A view is built once, reads the
state handles it was given, and repaints by itself when that state
changes. There is no message type and no update function.

```rust,no_run
use taim::prelude::*;

fn main() -> taim::Result {
    taim::launch("Views", |cx| {
        let name = cx.signal(String::new());

        column(views![
            text("What is your name?").size(20),
            text_field("Name", name),
            text(move || format!("Hello, {}!", name.get())),
        ])
        .gap(12.0)
        .padding(24.0)
    })
}
```

`use taim::prelude::*` imports every built-in view and the `views!`
macro. `views![...]` turns a list of different views into children for a
`column`, `row`, `grid` or `layers`.

## Props: fixed values or live closures

Most view arguments take an `impl IntoProp<T>`. You can pass either a
plain value or a closure that reads state:

```rust
use taim::prelude::*;

fn greeting(cx: &mut Scope) -> impl View {
    let count = cx.signal(0);

    column(views![
        text("Fixed text"),
        text(move || format!("Pressed {} times", count.get())),
        button("Press").on_press(move || count.update(|n| *n += 1)),
        button("Reset")
            .on_press(move || {
                count.set(0);
            })
            .enabled(move || count.get() > 0),
    ])
    .gap(8.0)
}
```

A closure prop subscribes to the state it reads. When `count` changes,
only the second `text` and the "Reset" button's enabled state update.

## Text

| View | Purpose |
|---|---|
| `text(content)` | A label. `.size(px)`, `.font(Font)`, `.color(Color)` |
| `icon(Icon::…)` | A glyph from the built-in icon font |

## Input

| View | Bound to | Notes |
|---|---|---|
| `button(label)` | — | `.on_press`, `.enabled`, `.secondary()`, `.ghost()`, `.danger()`, `.label_view(view)` |
| `icon_button(Icon::…)` | — | A button showing only an icon |
| `text_field(placeholder, State<String>)` | the text | `.on_change(\|text\| …)`, `.on_submit`, `.secret()`, `.autofill(…)`, `.plain()` |
| `search(placeholder, State<String>)` | the query | A text field with a search glyph |
| `text_editor(&Buffer)` | a `Buffer` | Multi-line editing; `Buffer::new(cx, "text")` |
| `checkbox(label, State<bool>)` | checked | `.enabled` |
| `switch(label, State<bool>)` | on | `.enabled` |
| `radio(label, value, State<T>)` | selection | One per option |
| `slider(range, State<f32>)` | value | `.step(f32)` |
| `stepper(State<i32>, range)` | value | − and + buttons |
| `dropdown(options, State<Option<usize>>)` | index | `.placeholder(…)` |
| `segmented(labels, State<usize>)` | index | Also `tabs` and `nav_bar` |
| `chip(label, State<bool>)` | selected | A filter chip |
| `date_picker(visible, chosen)` | `State<Date>` | A month calendar |
| `time_picker(State<Time>, step)` | time | Hours and minutes |

Input views write to the state you give them. To react to an edit,
either read the same state elsewhere or attach a callback:

```rust
use taim::prelude::*;

fn note_editor(cx: &mut Scope) -> impl View {
    let body = cx.signal(String::new());
    let title = cx.signal(String::from("New note"));

    column(views![
        text(move || title.get()).size(22),
        text_field("Write something", body).on_change(move |text| {
            let first = text.lines().next().unwrap_or("").trim();
            title.set(if first.is_empty() {
                String::from("New note")
            } else {
                first.to_owned()
            });
        }),
    ])
    .gap(8.0)
}
```

`on_change` runs only for edits the user makes, never when the bound state
is changed from code.

## Display

| View | Purpose |
|---|---|
| `progress(range, value)` | A progress bar |
| `rule()` | A divider. `.thickness(px)` |
| `skeleton(width, height)` | A loading placeholder |
| `avatar(name)` | Initials in a circle. `.size(px)` |
| `badge(view, count)` | A count over a view |
| `tooltip(view, tip)` | Text shown on hover or long press |
| `image(handle)` | A raster image (feature `image`). `.fit(ContentFit::Cover)`, `.radius`, `.label` |
| `svg(handle)` | A vector image (feature `svg`). `.fit`, `.tint`, `.label` |

`ContentFit` is in the prelude:

```rust
use taim::prelude::*;

#[cfg(feature = "image")]
fn cover(path: &str) -> impl View {
    image(taim::view::image_handle::Handle::from_path(path))
        .width(320.0)
        .height(180.0)
        .fit(ContentFit::Cover)
        .radius(12.0)
        .label("Cover photo")
}
```

## Structure

| View | Purpose |
|---|---|
| `column(children)`, `row(children)` | Stacks. See [Layout](layout.md) |
| `grid(columns, children)` | Equal-width cells |
| `layers(children)` | Children drawn on top of each other |
| `container(child)` | Padding, background, border, alignment |
| `scroll(child)` | A scrolling region |
| `list(count, \|index\| view)` | A virtualised list for long data |
| `space()` | Flexible or fixed empty space |
| `scaffold(top, body, bottom, overlays)` | A full screen with bars |
| `app_bar(title, actions)`, `list_tile(icon, title, subtitle)` | Common mobile pieces |
| `accordion(sections, State<Option<usize>>)` | Collapsible sections |
| `carousel(pages, State<usize>)` | Swipeable pages |

## Overlays

`dialog(State<bool>, view)`, `sheet(view, State<bool>)`,
`drawer(view, State<bool>)` and `snackbar(State<Option<String>>)` show
while their state says so, and write to it when the user dismisses them.

```rust
use taim::prelude::*;

fn confirm(cx: &mut Scope) -> impl View {
    let open = cx.signal(false);

    layers(views![
        button("Delete").danger().on_press(move || {
            open.set(true);
        }),
        dialog(
            open,
            column(views![
                text("Delete this note?"),
                button("Cancel").on_press(move || {
                    open.set(false);
                }),
            ])
            .gap(12.0),
        ),
    ])
}
```

## Views that follow state

| View | Purpose |
|---|---|
| `when(condition, \|\| view)` | Shows a view while `condition` is true |
| `each(items, \|item\| view)` | One view per item of a `Vec` |
| `rebuild(key, \|key\| Option<view>)` | Rebuilds a subtree when `key` changes |
| `navigator(Navigator<R>, \|cx, route\| view)` | A stack of pages |

The [todos example](../../examples/todos/src/main.rs) uses `each`, `when`
and `spawn` together.

## Your own views

When none of the built-in views fits, implement `View`. See
[Canvas drawing](canvas-drawing.md) for a full example that measures,
paints and describes itself to screen readers.
