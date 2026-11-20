# Getting started

This guide takes you from nothing to a running Taim application on the
desktop, then shows where to go for Android, iOS, the web and the stores.
Every Rust block here is compiled as a doctest of the `taim` crate
(`cargo test --doc -p taim`), so it matches the API of this release.

## 1. Install Rust and the system libraries

Taim needs Rust 1.95 or newer (edition 2024). Install it with
[rustup](https://rustup.rs), then add your platform's libraries:

**macOS**: Xcode Command Line Tools (`xcode-select --install`).

**Windows**: Visual Studio Build Tools with the "Desktop development with
C++" workload.

**Ubuntu / Debian**

```bash
sudo apt install -y pkg-config libx11-dev libxkbcommon-dev libwayland-dev
```

**Fedora**

```bash
sudo dnf install -y pkgconf-pkg-config libX11-devel libxkbcommon-devel wayland-devel
```

**Arch**

```bash
sudo pacman -S --needed pkgconf libx11 libxkbcommon wayland
```

## 2. Create a project

Taim is not yet published on crates.io. Clone the repository and let
`taim-new` write a project next to it:

```bash
git clone https://github.com/theaimartco/taim.git
cd taim
cargo run -p taim-new -- ../hello --name "Hello" --package com.example.hello
cd ../hello
cargo run
```

`taim-new` writes:

| Path | What it is |
|---|---|
| `src/main.rs` | The desktop entry point |
| `src/lib.rs` | Your application, shared by every platform |
| `android/` | A Gradle project that builds an APK or a Play Store bundle |
| `macos/` | A packaging config for sandboxed `.app` bundles and Mac App Store installers |

To add Taim to an existing project instead, depend on the repository:

```toml
[dependencies]
taim = { git = "https://github.com/theaimartco/taim" }
```

## 3. Your first view

```rust,no_run
use taim::prelude::*;

fn main() -> taim::Result {
    taim::launch("Counter", |cx| {
        let count = cx.signal(0_i64);

        column(views![
            text(move || format!("Count: {}", count.get())).size(32),
            row(views![
                button("−1").on_press(move || count.update(|n| *n -= 1)),
                button("+1").on_press(move || count.update(|n| *n += 1)),
            ])
            .gap(8.0),
        ])
        .gap(16.0)
        .padding(24.0)
    })
}
```

Three ideas carry the whole framework:

1. **The build closure runs once.** `launch` calls it to build the view
   tree. It is not called again when state changes.
2. **State is a handle.** `cx.signal(0)` returns a `State<i64>`: a small
   `Copy` handle you can move into as many closures as you like. `get`,
   `set`, `update` and `with` read and change it from any thread.
3. **Closures are live.** `text("Hello")` is fixed; `text(move ||
   ...)` is re-evaluated when a state it read changes. Only views that read
   a changed state are measured and painted again. Nothing else is touched.

There is no message enum, no `update` function and no diffing of a
virtual tree.

## 4. Lay things out

```rust
use taim::prelude::*;

fn profile() -> impl Into<AnyView> {
    container(
        column(views![
            row(views![
                avatar("Ada Lovelace").size(48.0),
                column(views![text("Ada Lovelace").size(18), text("Analyst").size(13)]).gap(2.0),
                space(),
                button("Follow"),
            ])
            .gap(12.0)
            .align(Align::Center)
            .fill_width(),
            rule(),
            text("Notes on the Analytical Engine."),
        ])
        .gap(12.0)
        .padding(16.0),
    )
    .fill()
}
```

- `column` and `row` stack children with `.gap`, `.padding` and `.align`.
- `space()` takes the leftover room, pushing what follows to the end.
- `container` positions one child: `.center()`, `.fill()`, `.align_bottom()`.
- `layers` draws children on top of each other; `grid` places them in columns.
- `scroll(view)` scrolls with the wheel, a finger drag and a fling.

## 5. Lists that follow state

`each` keeps one row per item and rebuilds only the rows whose item
changed. `when` shows a view while a condition holds.

```rust
use taim::prelude::*;

fn tasks(cx: &mut Scope) -> impl Into<AnyView> {
    let tasks = cx.signal(vec!["Write the guide".to_owned()]);
    let draft = cx.signal(String::new());
    let add = move || {
        let title = draft.get();
        if !title.trim().is_empty() {
            tasks.update(|all| all.push(title));
            draft.set(String::new());
        }
    };
    column(views![
        row(views![
            text_field("New task", draft).on_submit(add),
            button("Add").on_press(add),
        ])
        .gap(8.0),
        each(tasks, |task: &String| text(task.clone())),
        when(move || tasks.with(Vec::is_empty), || text("Nothing to do.")),
    ])
    .gap(8.0)
}
```

For thousands of rows, use `list`: it builds only the rows on screen.

```rust
use taim::prelude::*;

fn inbox(cx: &mut Scope) -> impl Into<AnyView> {
    let subjects = cx.signal((1..=100_000).map(|n| format!("Message {n}")).collect::<Vec<_>>());
    list(move || subjects.with(Vec::len), move |index| {
        text(move || subjects.with(|all| all[index].clone())).into()
    })
}
```

## 6. Screens and back navigation

```rust
use taim::prelude::*;

#[derive(Clone, PartialEq)]
enum Route {
    Inbox,
    Message(u32),
}

fn screens(cx: &mut Scope) -> impl Into<AnyView> {
    let nav = Navigator::new(cx, Route::Inbox);
    navigator(nav, move |_cx, route| match route {
        Route::Inbox => button("Open message 7")
            .on_press(move || nav.push(Route::Message(7)))
            .into(),
        Route::Message(id) => column(views![
            text(format!("Message {id}")),
            button("Back").on_press(move || {
                nav.pop();
            }),
        ])
        .into(),
    })
}
```

Pushes slide in, pops slide out, and an edge swipe (on touch screens) or the
Android back button pops. `nav.replace`, `nav.pop_to_root` and `nav.depth`
cover the rest.

## 7. Work in the background

`cx.tasks()` gives a handle that runs futures in the background for this
window. State handles are `Send`, so the future sets state directly and the
views that read it update. Closing the window cancels the work.

```rust
use taim::prelude::*;

fn loader(cx: &mut Scope) -> impl Into<AnyView> {
    let status = cx.signal("Idle".to_owned());
    let tasks = cx.tasks();
    column(views![
        text(move || status.get()),
        button("Load").on_press(move || {
            status.set("Loading…".to_owned());
            tasks.spawn(async move {
                let body = "42 records".to_owned(); // your HTTP call here
                status.set(body);
            });
        }),
    ])
    .gap(8.0)
}
```

The free function `spawn` starts detached work that outlives the window;
prefer `cx.tasks()` for anything that touches its state. For non-`Send`
results, `cx.channel` delivers messages to a handler on the window thread.

## 8. Settings and the platform

```rust
use std::time::Duration;

fn remember_launch() -> Result<u32, taim::storage::Error> {
    let mut settings = taim::storage::Store::open("settings")?;
    let launches = settings.get_as::<u32>("launches").unwrap_or(0) + 1;
    settings.set("launches", launches)?;
    Ok(launches)
}

fn say_hello() {
    let _ = taim::platform::open_url("https://example.com");
    let _ = taim::platform::share_text("Made with Taim");
    let _ = taim::platform::vibrate(Duration::from_millis(20));
}
```

`Store` writes through a temporary file, fsyncs and renames it into place,
so a crash or power loss never leaves a half-written file. On the web it is
`localStorage`. `taim::platform::data_dir()` is the application's private
folder on every platform.

## 9. Liquid glass

```rust
use taim::prelude::*;

fn player(artwork: AnyView) -> impl Into<AnyView> {
    layers(views![
        artwork,
        container(glass(text("Now playing")).radius(28.0))
            .align_bottom()
            .padding(16.0),
    ])
}
```

The pane frosts and lenses what is behind it. Without a GPU, or with
"reduce transparency" on, it draws as a solid tinted surface. See
[liquid glass](../design/liquid-glass.md).

## 10. Run it everywhere

| Platform | Command | Guide |
|---|---|---|
| macOS, Windows, Linux | `cargo run --release` | — |
| Android | `cd android && ./gradlew installDebug` | [android.md](../platform/android.md) |
| iOS | `python3 scripts/ios/package.py ...` | [ios.md](../platform/ios.md) |
| Web | `cargo build --target wasm32-unknown-unknown` | [web.md](../platform/web.md) |

Release builds on Windows open without a console window, and the store
packagers for Google Play, the App Store, the Mac App Store, the Microsoft
Store and the Snap Store are described in
[store submission](../release/store-submission.md).

## Next

- Browse every view: `cargo run -p gallery`.
- A phone-style app with tabs, a drawer, a sheet and pull-to-refresh:
  `cargo run -p mobile_shell`.
- [Architecture](../architecture/) and [performance](../performance/numbers.md).
