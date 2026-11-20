# Tasks and Subscriptions

> **This guide describes the pre-0.15 API** (`Element`, `Message`,
> `Task`, `taim::widget`), which the `taim` crate no longer exports. Its
> code does not compile against this release. For applications, use the
> view API in [Getting started](getting-started.md). This page is kept for
> its concepts while it is rewritten.

When building applications with Vyom-Taim, you often need to perform side effects: fetching data from the network, reading files, interacting with the system clipboard, or listening to continuous events like time and keyboard input. The framework manages all of these through two powerful concepts: **Tasks** and **Subscriptions**.

In the Elm architecture, side effects are not performed directly inside your `update` function. Instead, your `update` function remains pure: it takes the current state and a message, and returns the new state along with a description of the side effects to be executed by the runtime.

## 1. What are Tasks?

A `Task` in Vyom-Taim is a declarative description of an asynchronous operation that the runtime should perform on your behalf. When the operation completes, the runtime can feed the result back into your application by producing a new `Message`, which will be passed to your `update` function.

Because tasks are declarative descriptions, merely creating a task does not execute it. It must be returned from your `update` function to the runtime. This separation ensures that your application logic remains pure and easily testable.

```rust
pub fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::FetchData => {
            self.loading = true;
            // Return a Task to be executed by the runtime
            fetch_data_task()
        }
        Message::DataLoaded(data) => {
            self.loading = false;
            self.data = data;
            // No side effects needed
            Task::none()
        }
    }
}
```

## 2. `Task::none()`

When handling a message requires only state changes and no side effects, you return `Task::none()`. This tells the runtime that it has no additional work to do for this update cycle.

```rust
Message::Increment => {
    self.counter += 1;
    Task::none()
}
```

This is the most common return value in typical UI interactions like toggling a checkbox, expanding a menu, or typing in a text input.

## 3. `Task::perform()`

To execute an asynchronous future, you use `Task::perform()`. It takes a future and a function that maps the result of the future into your application's `Message` type.

```rust
use taim::Task;
use reqwest;

#[derive(Debug, Clone)]
pub enum Message {
    FetchUser(u32),
    UserFetched(Result<User, Error>),
}

pub fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::FetchUser(id) => {
            Task::perform(
                async move {
                    let response = reqwest::get(format!("https://api.example.com/users/{}", id)).await;
                    match response {
                        Ok(res) => res.json::<User>().await.map_err(|_| Error::ParseFailed),
                        Err(_) => Err(Error::NetworkFailed),
                    }
                },
                Message::UserFetched
            )
        }
        Message::UserFetched(result) => {
            // Handle result
            Task::none()
        }
    }
}
```

In this example, the runtime will drive the async block to completion in the background, without blocking the UI thread. Once the `User` is fetched, it will construct a `Message::UserFetched` and dispatch it back to your `update` function.

## 4. `Task::batch()`

Often, you need to trigger multiple independent side effects simultaneously. For example, when an application starts, you might want to fetch user preferences, load cached data from disk, and request initial content from a server.

You can combine multiple tasks into a single task using `Task::batch()`. The runtime will execute them concurrently.

```rust
Message::AppStarted => {
    Task::batch(vec![
        fetch_user_preferences(),
        load_cached_data(),
        connect_to_websocket(),
    ])
}
```

Tasks combined with `batch` do not guarantee any specific order of execution or completion.

## 5. `Task::map()`

When building large applications, you will often split your UI into smaller components. A child component will have its own `Task<child::Message>`, but the parent component needs a `Task<parent::Message>`.

`Task::map()` allows you to transform the output message of a task from one type to another, enabling component composition.

```rust
// Inside the parent's update function
Message::SettingsMessage(settings_msg) => {
    // child component returns Task<settings::Message>
    let child_task = self.settings.update(settings_msg);
    
    // map it to Task<parent::Message>
    child_task.map(Message::SettingsMessage)
}
```

## 6. `Task::then()` / `Task::chain()`

Sometimes you need to perform tasks sequentially, where the second task depends on the successful completion of the first, but you don't want to pollute your application state with intermediate messages.

While Vyom-Taim doesn't have a direct `.then()` on `Task` (as tasks are meant to be mapped to messages), you can achieve sequential execution entirely within the async future provided to `Task::perform()`:

```rust
Message::LoginAndFetchProfile => {
    Task::perform(
        async {
            let auth_token = api::login().await?;
            let profile = api::fetch_profile(auth_token).await?;
            Ok(profile)
        },
        Message::ProfileFetched
    )
}
```

## 7. `Task::abortable()`

Sometimes tasks take a long time, and the user might cancel the operation (e.g., by navigating away from the page). You can make tasks cancellable using a `Handle`.

```rust
use taim::task::Handle;

pub struct State {
    download_handle: Option<Handle>,
}

pub enum Message {
    StartDownload,
    CancelDownload,
    DownloadProgress(f32),
}

pub fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::StartDownload => {
            let (handle, task) = Task::abortable(
                download_file_future(),
                Message::DownloadProgress
            );
            self.download_handle = Some(handle);
            task
        }
        Message::CancelDownload => {
            if let Some(handle) = self.download_handle.take() {
                handle.abort();
            }
            Task::none()
        }
        // ...
    }
}
```

## 8. Window Tasks

Vyom-Taim provides specialized tasks for interacting with the application window. These are typically found in the `window` module.

```rust
use taim::window;

pub fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::CloseRequested => window::close(),
        Message::MinimizeWindow => window::minimize(true),
        Message::MaximizeWindow => window::maximize(true),
        Message::UpdateTitle(new_title) => window::set_title(new_title),
        Message::MoveWindow(x, y) => window::move_to(x, y),
        Message::ResizeWindow(w, h) => window::resize(w, h),
        Message::EnterFullscreen => window::change_mode(window::Mode::Fullscreen),
        Message::GetLatestSettings => {
            Task::perform(window::get_latest(), Message::SettingsReceived)
        }
        // ...
    }
}
```

## 9. Widget Operations

Certain widgets require imperative operations, such as focusing a text input, scrolling to a specific element, or selecting text. These are also managed via tasks to maintain the pure nature of the `update` function.

```rust
use taim::widget::text_input;
use taim::widget::scrollable;

pub fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::FocusSearch => {
            // Focus the text input with the given ID
            text_input::focus("search_input")
        }
        Message::SelectAllSearch => {
            text_input::select_all("search_input")
        }
        Message::ScrollToTop => {
            scrollable::scroll_to("main_scrollable", scrollable::AbsoluteOffset { x: 0.0, y: 0.0 })
        }
        Message::SnapToItem => {
            scrollable::snap_to("main_scrollable", scrollable::RelativeOffset { x: 0.0, y: 0.5 })
        }
        // ...
    }
}
```

## 10. Clipboard Tasks

Interacting with the system clipboard is an asynchronous operation, handled by tasks.

```rust
use taim::clipboard;

pub fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::CopyToClipboard(text) => {
            clipboard::write(text)
        }
        Message::PasteRequested => {
            Task::perform(clipboard::read(), Message::TextPasted)
        }
        // ...
    }
}
```

## 11. Subscriptions Explained

While Tasks are for one-off operations (fetch this, write that), **Subscriptions** are for continuous streams of events (time passing, window resizing, mouse moving, WebSocket messages).

Subscriptions are declarative: you tell the runtime *what* you want to listen to based on your current application state. If the state changes and you no longer return a subscription, the runtime automatically stops listening and cleans up the resources.

## 12. Setting Up

Subscriptions are defined by implementing a `subscription` function (or providing a closure to the `Application` builder) that takes the current state and returns a `Subscription<Message>`.

```rust
impl Application for MyApp {
    // ...
    fn subscription(&self) -> Subscription<Message> {
        // Return active subscriptions based on self's state
        Subscription::none()
    }
}
```

## 13. Time Subscriptions

A common use case is listening to the passing of time, useful for clocks, timers, or animations.

```rust
use taim::time;
use std::time::{Duration, Instant};

fn subscription(&self) -> Subscription<Message> {
    if self.is_playing {
        time::every(Duration::from_millis(1000)).map(|instant| {
            Message::Tick(instant)
        })
    } else {
        Subscription::none()
    }
}
```

## 14. Keyboard Subscriptions

You can subscribe to keyboard events globally, regardless of which widget has focus. This is essential for implementing keyboard shortcuts.

```rust
use taim::keyboard;

fn subscription(&self) -> Subscription<Message> {
    keyboard::listen(|event| {
        match event {
            keyboard::Event::KeyPressed { key, modifiers, .. } => {
                if key == keyboard::Key::Character("s".into()) && modifiers.command() {
                    Some(Message::SaveRequested)
                } else {
                    None
                }
            }
            _ => None,
        }
    })
}
```

## 15. Window Events

You can subscribe to window-level events like resizing or close requests.

```rust
use taim::window;

fn subscription(&self) -> Subscription<Message> {
    window::events().map(|event| match event {
        window::Event::Resized { width, height } => Message::WindowResized(width, height),
        window::Event::CloseRequested => Message::WindowCloseRequested,
        _ => Message::IgnoredEvent,
    })
}
```

## 16. Custom Subscriptions

For integrations with external continuous data sources (like WebSockets, long-polling, or hardware sensors), you can create custom subscriptions using `Subscription::run`.

```rust
use taim::subscription;
use futures::stream::StreamExt;

fn websocket_subscription() -> Subscription<Message> {
    subscription::run("websocket_id", |mut channel| async move {
        let mut ws = connect_websocket().await;
        
        while let Some(msg) = ws.next().await {
            // Send the message to the application
            let _ = channel.send(Message::WsMessage(msg)).await;
        }
        
        // Loop forever if connection closes, or handle reconnection
        std::future::pending().await
    })
}
```

The string `"websocket_id"` is used by the runtime to deduplicate subscriptions. If the `subscription` function returns multiple subscriptions with the same ID, they are treated as the same underlying stream.

## 17. Conditional Subscriptions

Because the `subscription` function takes `&self`, you can easily turn subscriptions on and off based on state. The runtime handles all the setup and teardown automatically.

```rust
fn subscription(&self) -> Subscription<Message> {
    if self.is_recording {
        microphone::listen().map(Message::AudioData)
    } else {
        Subscription::none()
    }
}
```

When `self.is_recording` flips from `false` to `true`, the runtime starts the microphone listener. When it flips back to `false`, the runtime drops the listener, freeing system resources instantly.

## 18. `Subscription::batch()`

Just like Tasks, you can combine multiple subscriptions together.

```rust
fn subscription(&self) -> Subscription<Message> {
    Subscription::batch(vec![
        time::every(Duration::from_secs(1)).map(Message::Tick),
        keyboard::listen(handle_keyboard),
        window::events().map(handle_window_event),
    ])
}
```

## 19. Complete Real-World Example

Here is a complete example of a Stopwatch application that heavily utilizes both Tasks and Subscriptions.

```rust
use taim::time;
use taim::{Application, Task, Subscription, Element, widget::{column, button, text}};
use std::time::{Duration, Instant};

pub struct Stopwatch {
    state: State,
    duration: Duration,
    last_tick: Option<Instant>,
}

enum State {
    Idle,
    Running,
}

#[derive(Debug, Clone)]
pub enum Message {
    Toggle,
    Reset,
    Tick(Instant),
}

impl Application for Stopwatch {
    type Message = Message;
    // ... setup methods omitted ...

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Toggle => {
                match self.state {
                    State::Idle => {
                        self.state = State::Running;
                        self.last_tick = Some(Instant::now());
                    }
                    State::Running => {
                        self.state = State::Idle;
                        self.last_tick = None;
                    }
                }
                Task::none()
            }
            Message::Reset => {
                self.duration = Duration::default();
                self.last_tick = if matches!(self.state, State::Running) {
                    Some(Instant::now())
                } else {
                    None
                };
                Task::none()
            }
            Message::Tick(now) => {
                if let Some(last) = self.last_tick.replace(now) {
                    self.duration += now - last;
                }
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        match self.state {
            State::Idle => Subscription::none(),
            State::Running => {
                // Only listen to time when running!
                time::every(Duration::from_millis(16)).map(Message::Tick)
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let seconds = self.duration.as_secs_f32();
        
        column![
            text(format!("{:.2}", seconds)).size(50),
            button(if matches!(self.state, State::Idle) { "Start" } else { "Stop" })
                .on_press(Message::Toggle),
            button("Reset").on_press(Message::Reset),
        ]
        .spacing(20)
        .into()
    }
}
```

This example perfectly demonstrates the Elm architecture: the `update` function handles state transitions cleanly, while the `subscription` function declaratively requests time ticks *only* when the stopwatch is in the `Running` state. The runtime ensures that CPU isn't wasted processing time ticks when the stopwatch is idle.
