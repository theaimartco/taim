# Background work

Taim has no `Task` or `Subscription` types. State handles are `Send`, so
background work writes to state directly, and the views reading that state
update and wake the window by themselves.

## Spawning work

`cx.spawn` runs a future in the background for as long as the window is
open. Closing the window cancels it.

```rust
use std::time::Duration;
use taim::prelude::*;

fn loader(cx: &mut Scope) -> impl View {
    let status = cx.signal(String::from("Idle"));
    let tasks = cx.tasks();

    column(views![
        text(move || status.get()),
        button("Load").on_press(move || {
            status.set(String::from("Loading…"));
            tasks.spawn(async move {
                std::thread::sleep(Duration::from_millis(500));
                status.set(String::from("Done"));
            });
        }),
    ])
    .gap(8.0)
}
```

- `cx.spawn(future)` starts work while the view is being built.
- `cx.tasks()` returns a `TaskScope` you can move into callbacks to start
  work later, such as from a button press. Its `spawn` returns `false`
  once the window has closed.
- `spawn(future)` from the prelude starts a detached task that outlives the
  window. Prefer the scoped forms for anything that touches a window's
  state.

Each task runs on its own thread, so blocking calls (file I/O, a blocking
HTTP client) are fine inside it.

## Streams of events: channels

For a long-lived producer (a socket, a file watcher, a timer thread), make
a channel. Its handler runs on the window thread, so it can hold
non-`Send` values, and the `Sender` can go to any thread:

```rust
use std::time::Duration;
use taim::prelude::*;

fn clock(cx: &mut Scope) -> impl View {
    let ticks = cx.signal(0_u64);
    let sender = cx.channel(move |tick: u64| {
        ticks.set(tick);
    });

    std::thread::spawn(move || {
        for tick in 1.. {
            std::thread::sleep(Duration::from_secs(1));
            if sender.send(tick).is_err() {
                break;
            }
        }
    });

    text(move || format!("{} seconds", ticks.get()))
}
```

`send` fails once the window has closed, which is the signal for the
producer to stop. The channel is bounded (256 messages), so a producer
that outpaces the window gets `TrySendError::Full` instead of using
unbounded memory.

## Timers and animation

For visual timing (blinking, easing, transitions) use the animation views
(`animated`, `appear`) or a custom view's `is_animating` and `deadline`
methods, not a timer thread. They run on the frame clock, so they stop when
the window is hidden and respect reduced motion. See
[Animations](animations.md).

## Platform services

Platform calls such as notifications, biometrics and the keychain
(`taim::apple`, `taim::android`, `taim::platform`) return their results
through the same mechanisms: call them from a task and write the result to
state. They report failures as a `ServiceError`.
