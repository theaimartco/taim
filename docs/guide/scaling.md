# Structuring larger apps

A small Taim app fits in one closure passed to `taim::launch`. As it grows,
split it the way you would split any Rust code: into functions and
modules. Taim needs no component trait, message enum or update function
to do this, because state handles are small `Copy` values you pass around.

## Components are functions

A component is a function that takes the state it needs and returns a view:

```rust,no_run
use taim::prelude::*;

pub fn counter(label: &'static str, count: State<i32>) -> impl View {
    row(views![
        text(move || format!("{label}: {}", count.get())),
        space(),
        button("-").secondary().on_press(move || count.update(|n| *n -= 1)),
        button("+").on_press(move || count.update(|n| *n += 1)),
    ])
    .gap(8.0)
    .align(Align::Center)
    .fill_width()
}

fn main() -> taim::Result {
    taim::launch("Counters", |cx| {
        let apples = cx.signal(0);
        let pears = cx.signal(0);

        column(views![
            counter("Apples", apples),
            counter("Pears", pears),
            text(move || format!("Total: {}", apples.get() + pears.get())).size(20),
        ])
        .gap(12.0)
        .padding(24.0)
    })
}
```

The parent owns the state and passes it down; the "Total" line follows both
counters without any wiring.

### Owning state inside a component

If a component's state is nobody else's business, take `&mut Scope` and
create it there:

```rust
use taim::prelude::*;

pub fn expander(cx: &mut Scope, title: &'static str, body: &'static str) -> impl View {
    let open = cx.signal(false);

    column(views![
        button(move || if open.get() { format!("▾ {title}") } else { format!("▸ {title}") })
            .ghost()
            .on_press(move || open.update(|open| *open = !*open)),
        when(open, move || text(body)),
    ])
    .gap(4.0)
}
```

## Grouping state into a model

When many components share a lot of state, gather it in a `Copy` struct
of handles and give it methods:

```rust
use taim::prelude::*;

#[derive(Clone, Copy)]
pub struct Cart {
    items: State<Vec<(String, u32)>>,
    coupon: State<Option<String>>,
}

impl Cart {
    pub fn new(cx: &mut Scope) -> Self {
        Self {
            items: cx.signal(Vec::new()),
            coupon: cx.signal(None),
        }
    }

    pub fn add(self, name: &str, cents: u32) {
        self.items.update(|items| items.push((name.to_owned(), cents)));
    }

    pub fn total(self) -> u32 {
        let total: u32 = self.items.with(|items| items.iter().map(|(_, cents)| cents).sum());
        if self.coupon.with(Option::is_some) { total * 9 / 10 } else { total }
    }
}

pub fn checkout(cart: Cart) -> impl View {
    column(views![
        button("Add tea").on_press(move || cart.add("Tea", 450)),
        text(move || format!("Total: ${:.2}", cart.total() as f32 / 100.0)),
    ])
    .gap(8.0)
}
```

Business logic stays in plain methods you can unit test without a window.

## Screens and navigation

Use a `Navigator` for a stack of screens. Each route is a value, and each
page is built by a function:

```rust
use taim::prelude::*;

#[derive(Clone, PartialEq)]
enum Route {
    Home,
    Detail(u32),
}

fn app(cx: &mut Scope) -> impl View {
    let nav = Navigator::new(cx, Route::Home);

    navigator(nav, move |_cx, route| match route {
        Route::Home => AnyView::of(button("Open item 7").on_press(move || nav.push(Route::Detail(7)))),
        Route::Detail(id) => AnyView::of(text(format!("Item {id}"))),
    })
    .transition(PageTransition::Slide)
}
```

## A module layout that scales

```text
src/
  main.rs          // taim::launch, theme, top-level navigator
  model/           // Copy structs of state handles, with methods
  screens/         // one module per route: fn screen(cx, model) -> impl View
  components/      // reusable views: fn card(...) -> impl View
```

## Keeping it fast

- Read state inside closures (`text(move || …)`), not while building. A
  value read once while building never updates.
- Prefer several small states over one large struct in a single state:
  a view repaints when any state it reads changes.
- Use `list` for long collections and `each` for short ones.
- Use `rebuild` only when a whole subtree really changes shape. It throws
  the old subtree away.
