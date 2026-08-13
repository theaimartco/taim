# Getting Started with Aura

Welcome to **Aura**, a fast, declarative, and highly customizable GUI framework for Rust. Aura is built on top of the Elm architecture, providing a robust and predictable way to build cross-platform desktop applications.

This guide will walk you through everything you need to know to build your first Aura application, from installing dependencies to structuring a real-world project. By the end of this guide, you will have a solid understanding of how Aura works and how to leverage its powerful builder API.

---

## 1. Prerequisites

Before you can start building with Aura, you need to ensure your development environment is properly configured.

### Rust

Aura requires **Rust 1.80** or later. If you don't have Rust installed, the easiest way to install it is via `rustup`.

Open your terminal and run:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

After installation, verify your Rust version:

```bash
rustc --version
```

If you already have Rust installed, ensure it is up to date:

```bash
rustup update
```

### GPU Requirements

Aura leverages hardware acceleration for fast, smooth rendering. You need a modern GPU that supports Vulkan, Metal (macOS), or DX12 (Windows).

### Linux Headers & Dependencies

If you are developing on Linux, you will need to install a few development libraries. Aura uses Wayland or X11 for windowing.

**Ubuntu / Debian:**

```bash
sudo apt update
sudo apt install -y pkg-config libx11-dev libxkbcommon-dev libwayland-dev
```

**Fedora:**

```bash
sudo dnf install -y pkgconf-pkg-config libX11-devel libxkbcommon-devel wayland-devel
```

**Arch Linux:**

```bash
sudo pacman -S --needed pkgconf libx11 libxkbcommon wayland
```

---

## 2. Installation

With your environment set up, you can now add Aura to your project.

### Creating a New Project

Create a new Rust binary project:

```bash
cargo new my_aura_app
cd my_aura_app
```

### Adding Aura

You can add Aura using `cargo add`:

```bash
cargo add aura
```

Alternatively, you can manually edit your `Cargo.toml`:

```toml
[package]
name = "my_aura_app"
version = "0.1.0"
edition = "2021"

[dependencies]
aura = "0.1" # Replace with the latest version
```

### Feature Flags

Aura is highly modular. Depending on your needs, you might want to enable or disable certain features. Here is an overview:

| Feature Flag | Default | Description | When to use |
| :--- | :---: | :--- | :--- |
| `wayland` | Yes | Wayland support for Linux. | If targeting modern Linux desktops. |
| `x11` | Yes | X11 support for Linux. | For compatibility with older Linux setups. |
| `tokio` | Yes | Asynchronous runtime support. | Required if you use `Task` for async I/O. |
| `smol` | No | Alternative async runtime. | If you prefer `smol` over `tokio`. |
| `svg` | Yes | SVG rendering support. | If you need to render vector graphics. |
| `canvas` | Yes | 2D canvas drawing API. | If you need custom drawing primitives. |
| `debug` | No | Enables debug overlays. | Use during development to inspect layout. |

To enable specific features in `Cargo.toml`:

```toml
[dependencies]
aura = { version = "0.1", default-features = false, features = ["tokio", "svg", "wayland"] }
```

---

## 3. Your First App

Let's build the classic counter application. This is the simplest way to understand how Aura ties State, Messages, and Views together.

Replace the contents of `src/main.rs` with the following:

```rust
use aura::{Element, Task};
use aura::widget::{button, column, text, container};

// 1. The State
#[derive(Default)]
struct Counter {
    value: i32,
}

// 2. The Messages
#[derive(Debug, Clone, Copy)]
enum Message {
    Increment,
    Decrement,
}

// 3. The Update Logic
fn update(state: &mut Counter, message: Message) {
    match message {
        Message::Increment => {
            state.value += 1;
        }
        Message::Decrement => {
            state.value -= 1;
        }
    }
}

// 4. The View Logic
fn view(state: &Counter) -> Element<Message> {
    container(
        column![
            button("Increment").on_press(Message::Increment),
            text(format!("Value: {}", state.value)).size(50),
            button("Decrement").on_press(Message::Decrement),
        ]
        .spacing(20)
        .align_items(aura::Alignment::Center)
    )
    .center_x()
    .center_y()
    .into()
}

// 5. The Entry Point
fn main() -> aura::Result {
    aura::run(update, view)
}
```

### Line-by-Line Explanation

1. **Imports:** We import `Element` and `Task` from `aura`, along with common widgets from `aura::widget`.
2. **State:** We define a struct `Counter`. It derives `Default`, which `aura::run` requires to initialize the application state automatically (in this case, `value` defaults to `0`).
3. **Messages:** The `Message` enum defines every possible event that can occur in our application. `Increment` and `Decrement` are our two actions. It must implement `Debug`, `Clone`, and often `Copy`.
4. **Update Logic:** The `update` function takes a mutable reference to our state (`&mut Counter`) and a `Message`. We match on the message to mutate the state accordingly. Since we are not performing any asynchronous work, we don't need to return a `Task` here.
5. **View Logic:** The `view` function takes an immutable reference to our state (`&Counter`) and returns an `Element<Message>`. We use a `column!` macro to stack our widgets vertically. `button("...").on_press(...)` creates a button that emits a specific message when clicked. `container` is used to center everything on the screen.
6. **Entry Point:** The `main` function uses `aura::run(update, view)`. This is the simplest way to start an Aura application. It automatically sets up the window and event loop.

---

## 4. How The Elm Architecture Works

Aura is built heavily on the principles of **The Elm Architecture (TEA)**. TEA provides a strictly unidirectional data flow, which makes debugging and understanding your app's state incredibly straightforward.

### Data Flow Diagram

```text
       +-----------------+
       |                 |
       |     STATE       |
       |                 |
       +-------+---------+
               |
               v (Immutable reference passed)
       +-------+---------+
       |                 |
       |      VIEW       |  <-- Generates UI based on State
       |                 |
       +-------+---------+
               |
               v (User interaction triggers...)
       +-------+---------+
       |                 |
       |    MESSAGES     |  <-- e.g., Click, Input
       |                 |
       +-------+---------+
               |
               v (Message and Mutable State passed)
       +-------+---------+
       |                 |
       |     UPDATE      |  <-- Mutates State, optionally spawns Tasks
       |                 |
       +-------+---------+
               |
               v (New State)
       +-----------------+
       |                 |
       |     STATE       |  <-- Cycle repeats
       |                 |
       +-----------------+
```

### Why it matters vs MVC/MVVM

In MVC (Model-View-Controller) or MVVM (Model-View-ViewModel), data bindings can often be bi-directional. A view can modify a model directly, or a model can trigger arbitrary view updates. This can lead to "spaghetti state", where it's hard to track *why* a particular value changed.

In Aura (and TEA):
1. **State is the single source of truth.**
2. **State can ONLY be modified in the `update` function.**
3. **The `update` function is ONLY triggered by explicit `Messages`.**
4. **The `view` is a pure function of the `State`.**

This makes rendering predictable. If you know your state, you know exactly what your UI looks like. If your UI looks wrong, you know either your state is wrong (check `update`) or your rendering logic is wrong (check `view`).

---

## 5. Custom State

While the `Counter` example used a single `i32`, real applications have complex states. You define your state as a standard Rust `struct`.

```rust
use aura::{Element, Task};
use aura::widget::text;

#[derive(Default)]
struct AppState {
    user_name: String,
    is_logged_in: bool,
    active_tab: usize,
    items: Vec<String>,
}

#[derive(Debug, Clone)]
enum Message {
    None,
}

fn update(state: &mut AppState, message: Message) {
    // Handle messages
}

fn view(state: &AppState) -> Element<Message> {
    text(format!("Welcome, {}!", state.user_name)).into()
}

fn main() -> aura::Result {
    aura::run(update, view)
}
```

By deriving `Default`, you instruct Rust on how to create the initial version of your state. If you need complex initialization (e.g., loading from disk), you will use the Application Builder's `boot_fn` (covered in Section 7).

---

## 6. Messages With Data

Messages aren't just simple flags; they can carry data payloads. This is essential for handling text input, selections, or incoming network data.

```rust
use aura::{Element, Task, Color};
use aura::widget::{text_input, column, text, pick_list};

#[derive(Default)]
struct ProfileEditor {
    name: String,
    favorite_color: Option<Color>,
}

#[derive(Debug, Clone)]
enum Message {
    NameChanged(String),
    ColorSelected(Color),
}

fn update(state: &mut ProfileEditor, message: Message) {
    match message {
        // We receive the new string from the text input
        Message::NameChanged(new_name) => {
            state.name = new_name;
        }
        // We receive the selected color from a pick list
        Message::ColorSelected(color) => {
            state.favorite_color = Some(color);
        }
    }
}

fn view(state: &ProfileEditor) -> Element<Message> {
    column![
        text("Edit your profile:"),
        // The text input takes the current value and a closure mapping 
        // the new string to a Message
        text_input("Enter your name", &state.name)
            .on_input(Message::NameChanged),
            
        text(format!("Hello, {}", state.name)),
    ].into()
}

fn main() -> aura::Result {
    aura::run(update, view)
}
```

Notice how `Message::NameChanged` expects a `String`. The `on_input` method of `text_input` automatically provides that string whenever the user types.

---

## 7. The Application Builder

For anything beyond a simple prototype, `aura::run` is not enough. You need control over the window size, title, themes, fonts, and initialization logic.

This is where `aura::application()` comes in. It returns an Application Builder that lets you configure every aspect of your app.

Here is a comprehensive example using all builder methods:

```rust
use aura::{application, Element, Task, Theme, Size, window};
use aura::widget::text;

struct MyComplexApp {
    data: String,
}

#[derive(Debug, Clone)]
enum Message {
    Loaded(String),
}

// boot_fn initializes the state and can return an initial Task
fn boot() -> (MyComplexApp, Task<Message>) {
    (
        MyComplexApp { data: String::from("Loading...") },
        // Simulate a startup task
        Task::perform(async { "Data loaded!".to_string() }, Message::Loaded)
    )
}

fn update(state: &mut MyComplexApp, message: Message) -> Task<Message> {
    match message {
        Message::Loaded(data) => {
            state.data = data;
            Task::none()
        }
    }
}

fn view(state: &MyComplexApp) -> Element<Message> {
    text(&state.data).into()
}

fn main() -> aura::Result {
    application(boot, update, view)
        // Set a static string for the window title
        .title("My Awesome App")
        
        // OR set a dynamic title based on the state! (Overrides the static title)
        .title_with(|state: &MyComplexApp| {
            format!("App - {}", state.data)
        })
        
        // Set the active theme
        .theme(Theme::CatppuccinMocha)
        
        // OR set a dynamic theme based on state
        // .theme_with(|state| Theme::Light)
        
        // Configure the initial window size
        .window_size(Size::new(800.0, 600.0))
        
        // Configure deeper window settings (resizable, decorations, etc.)
        .window(window::Settings {
            resizable: true,
            decorations: true,
            ..Default::default()
        })
        
        // Center the window on the primary monitor on startup
        .centered()
        
        // Enable or disable MSAA (Multisample Anti-Aliasing)
        .antialiasing(true)
        
        // Set the default font family for the entire app
        // .default_font(Font::with_name("Open Sans"))
        
        // Load custom font bytes (e.g., from `include_bytes!`)
        // .font(include_bytes!("../assets/fonts/Inter-Regular.ttf").as_slice())
        
        // Subscribe to global events (keyboard, mouse, time)
        // .subscription(|state| aura::time::every(...))
        
        // Force a specific scale factor (usually handled by the OS)
        // .scale_factor(|_window_id| 1.5)
        
        // Set a custom background style for the root application
        // .style(|theme| container::Appearance { ... })
        
        // Finally, run the application
        .run()
}
```

### Builder Methods Breakdown

- **`.title("string")`**: Sets a static window title.
- **`.title_with(|state| ...)`**: A closure that returns a `String`. Useful for updating the title dynamically (e.g., "Document - Unsaved").
- **`.theme(Theme)`**: Sets the color palette for the app.
- **`.theme_with(|state| ...)`**: Allows you to switch themes at runtime based on your state.
- **`.window_size(Size)`**: The initial logical size of the window.
- **`.window(Settings)`**: Detailed window configuration.
- **`.centered()`**: A convenience method to place the window in the center of the screen.
- **`.antialiasing(bool)`**: Smooths out vector graphics.
- **`.default_font(Font)` / `.font(&[u8])`**: Essential for branding your application with custom typography.
- **`.subscription(fn)`**: Ties your app to external asynchronous event streams (like global keyboard listeners or timers). See the [Subscriptions Guide](./subscriptions.md) for details.
- **`.scale_factor(fn)`**: Advanced: overrides the OS UI scaling.
- **`.style(fn)`**: Customizes the base window background.
- **`.run()`**: Consumes the builder and starts the event loop.

---

## 8. Running Your App

During development, you will typically run your application in debug mode:

```bash
cargo run
```

**Debug mode** compiles quickly and includes safety checks, but the resulting executable is large and may run slowly. This is especially noticeable in GUI applications if you have complex layouts.

When you are ready to test performance or distribute your app, build in **release mode**:

```bash
cargo run --release
```

**Release mode** takes longer to compile because Rust applies heavy optimizations. The resulting binary will be significantly smaller, and rendering will be buttery smooth. Always measure UI performance in release mode!

---

## 9. Feature Flags (Detailed)

Here is the complete table of feature flags for `aura`:

| Feature | Default | Description | When to Use |
| :--- | :---: | :--- | :--- |
| `wayland` | Yes | Native Wayland windowing support on Linux. | Essential for modern Linux distributions. |
| `x11` | Yes | X11 windowing fallback for Linux. | Essential for compatibility on Linux. |
| `tokio` | Yes | Integration with the `tokio` async runtime. | If you use `reqwest`, `sqlx`, or standard Tokio primitives. |
| `smol` | No | Integration with the `smol` async runtime. | If you prefer a lighter-weight async runtime. |
| `system-fonts` | Yes | Allows querying and loading fonts from the OS. | If you want to use fonts like Arial or Segoe UI without bundling them. |
| `svg` | Yes | Enables the `svg` widget for rendering `.svg` files. | Highly recommended for modern, scalable iconography. |
| `image` | Yes | Enables the `image` widget for `.png`, `.jpg`, etc. | If your app displays raster graphics. |
| `canvas` | Yes | Enables the highly performant `Canvas` widget. | If you need to draw custom charts, games, or complex visualizations. |
| `debug` | No | Shows bounding boxes and layout overlays. | Enable temporarily in `Cargo.toml` when debugging layout issues. |
| `accessibility`| Yes | Enables screen reader integration (AccessKit). | Keep enabled to ensure your app is accessible to all users. |

---

## 10. All Built-in Themes

Aura ships with a massive collection of high-quality, pre-defined themes out of the box. 

You can set the theme programmatically via `.theme(Theme::...)` or use the environment variable `AURA_THEME`.

### Standard Themes
- **`Light`**: A clean, high-contrast light theme (default).
- **`Dark`**: A standard, easy-on-the-eyes dark theme.

### Popular Community Themes
- **`Dracula`**: A dark theme with vibrant pinks and purples.
- **`Nord`**: An arctic, north-bluish cold color palette.
- **`SolarizedLight` / `SolarizedDark`**: Precision colors for machines and people.
- **`GruvboxLight` / `GruvboxDark`**: Retro groove color scheme.
- **`CatppuccinLatte` (Light)**: Soothing pastel light theme.
- **`CatppuccinFrappe` / `Macchiato` / `Mocha`**: Various shades of dark pastel themes.
- **`TokyoNight` / `TokyoNightStorm` / `TokyoNightLight`**: A clean, dark Tokyo-inspired theme.
- **`KanagawaWave` / `KanagawaDragon` / `KanagawaLotus`**: Colors inspired by traditional Japanese woodblock prints.
- **`Moonfly` / `Nightfly`**: Dark themes inspired by the night sky.
- **`Oxocarbon`**: A dark theme inspired by IBM Carbon.
- **`Ferra`**: A warm, earthy dark theme.

### Environment Variable Override

Users can override your app's hardcoded theme (if you allow it) by setting:

```bash
export AURA_THEME=CatppuccinMocha
cargo run
```

### Custom Themes

If none of the built-in themes fit your brand, you can create your own.

```rust
use aura::Theme;

// Option 1: Load from a palette structure
let my_theme = Theme::custom(aura::theme::Palette {
    background: aura::Color::from_rgb(0.1, 0.1, 0.1),
    text: aura::Color::WHITE,
    primary: aura::Color::from_rgb(0.9, 0.4, 0.4),
    // ... define other colors
    ..Default::default()
});

// Option 2: Provide a function for total control over widget styling
let advanced_theme = Theme::custom_with_fn(|_widget_type| {
    // Return specific styling based on widget request
    // See the Advanced Theming guide for details.
    todo!()
});
```

---

## 11. Project Structure

For a "Hello World", putting everything in `main.rs` is fine. For a real application, you should organize your code into modules. 

Here is the recommended folder structure for an Aura project:

```text
my_aura_app/
├── Cargo.toml
└── src/
    ├── main.rs       // Entry point, builder config, module declarations
    ├── app.rs        // The main State struct, global Messages, Update logic
    ├── screens/      // High-level views (e.g., individual pages)
    │   ├── mod.rs
    │   ├── home.rs   // State/Update/View specific to the Home screen
    │   └── settings.rs
    └── widgets/      // Reusable, custom UI components
        ├── mod.rs
        ├── card.rs
        └── user_avatar.rs
```

### Example: Separating `app.rs`

**`src/app.rs`**:
```rust
use aura::{Element, Task};
use crate::screens::{home, settings};

pub struct App {
    current_screen: Screen,
}

enum Screen {
    Home(home::State),
    Settings(settings::State),
}

#[derive(Debug, Clone)]
pub enum Message {
    HomeMsg(home::Message),
    SettingsMsg(settings::Message),
    NavigateToSettings,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self { current_screen: Screen::Home(home::State::new()) },
            Task::none()
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        // Delegate messages to the appropriate screen state
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        // Render the current screen
        use aura::widget::text;
        text("Routing example").into()
    }
}
```

**`src/main.rs`**:
```rust
mod app;
mod screens;
mod widgets;

use aura::application;
use app::App;

fn main() -> aura::Result {
    application(App::new, App::update, App::view)
        .title("Structured App")
        .run()
}
```

This structure keeps your code clean, modular, and easy to test.

---

## 12. Next Steps

Congratulations! You now have a solid understanding of the Aura framework. From here, you can dive deeper into specific topics:

- **[Layout and Widgets](./layout.md)** — Master `row!`, `column!`, `container`, and flexbox styling.
- **[Handling State and Tasks](./tasks.md)** — Learn how to make HTTP requests, read files, and handle asynchronous data.
- **[Subscriptions](./subscriptions.md)** — Listen to global events like keyboard shortcuts, window resizing, or timers.
- **[Custom Widgets](./custom-widgets.md)** — Build your own complex UI elements from scratch using the `Widget` trait.
- **[Canvas API](./canvas.md)** — Draw custom 2D graphics, charts, and shapes.
- **[Advanced Theming](./theming.md)** — Create deeply customized, brand-specific UI styles.

Happy coding!
