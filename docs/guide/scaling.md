# Scaling Applications

When you first start building an application with Vyom-Taim, a single `update` function, one `Message` enum, and one `view` function might be all you need. However, as your application grows in complexity, features, and lines of code, managing everything in a single file becomes unwieldy.

This guide explains how to scale your application architecture by breaking it down into manageable, reusable components.

## 1. When to Scale

You should consider scaling and refactoring your application when:
- Your `update` function becomes a massive `match` statement spanning hundreds of lines.
- Your `Message` enum has dozens of variants, many of which only pertain to specific UI sections.
- Your `view` function is too deeply nested.
- You need multiple distinct screens (e.g., Home, Settings, Profile) with their own dedicated state.
- The file exceeds roughly 500 lines of code.

## 2. The Component Pattern

The primary way to scale an Taim application is by applying the Component pattern. A Component is essentially a mini-application. It has its own `State`, `Message`, `update`, and `view`.

```rust
// A basic Counter Component
pub mod counter {
    use taim::{Element, Task, widget::{row, button, text}};

    pub struct State {
        value: i32,
    }

    #[derive(Debug, Clone)]
    pub enum Message {
        Increment,
        Decrement,
    }

    impl State {
        pub fn new() -> Self {
            Self { value: 0 }
        }

        pub fn update(&mut self, message: Message) -> Task<Message> {
            match message {
                Message::Increment => self.value += 1,
                Message::Decrement => self.value -= 1,
            }
            Task::none()
        }

        pub fn view(&self) -> Element<Message> {
            row![
                button("-").on_press(Message::Decrement),
                text(self.value.to_string()),
                button("+").on_press(Message::Increment),
            ]
            .into()
        }
    }
}
```

This encapsulates all the logic and rendering for a counter. A parent application can now use this counter without knowing how it works internally.

## 3. The Action Enum

While the Component pattern is great for encapsulation, child components often need to communicate with their parents. For example, a child component might need to tell the parent to navigate to a different screen, or that some data was successfully saved.

Instead of passing callbacks into the component, the recommended pattern is for the child's `update` function to return both a `Task` AND an `Action`.

```rust
pub enum Action {
    None,
    RunTask(Task<Message>),
    NavigateToSettings,
    DataSaved(String),
}

// In the child component:
pub fn update(&mut self, message: Message) -> Action {
    match message {
        Message::SaveData => {
            // The child instructs the parent it wants to save
            Action::DataSaved("New Data".into())
        }
        Message::GoSettings => {
            Action::NavigateToSettings
        }
    }
}
```

The parent then matches on this `Action` to perform application-level state changes.

## 4. Composing Screens

When building a multi-screen app, your parent `State` is usually an enum representing the active screen.

```rust
pub enum Screen {
    Home(home::State),
    List(list::State),
    Detail(detail::State),
}

pub struct App {
    current_screen: Screen,
}
```

When a message arrives, you dispatch it to the currently active screen.

## 5. Message Mapping

Because the child component produces `child::Message` in its view and returns `Task<child::Message>`, the parent must map these to `parent::Message`.

```rust
#[derive(Debug, Clone)]
pub enum Message {
    HomeMsg(home::Message),
    ListMsg(list::Message),
}

// In parent's view:
fn view(&self) -> Element<Message> {
    match &self.current_screen {
        Screen::Home(home_state) => {
            // Element::map transforms Element<home::Message> into Element<parent::Message>
            home_state.view().map(Message::HomeMsg)
        }
        Screen::List(list_state) => {
            list_state.view().map(Message::ListMsg)
        }
    }
}

// In parent's update:
fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::HomeMsg(msg) => {
            if let Screen::Home(state) = &mut self.current_screen {
                // Task::map transforms Task<home::Message> into Task<parent::Message>
                return state.update(msg).map(Message::HomeMsg);
            }
            Task::none()
        }
        // ...
    }
}
```

This pattern strongly types the message passing boundaries between components.

## 6. Nested Components

This pattern can be nested arbitrarily deep. An application can contain a `Dashboard` screen. The `Dashboard` screen can contain a `Sidebar` component. The `Sidebar` component can contain a `ProfileWidget` component.

At each layer, the parent is responsible for mapping the messages and tasks of its immediate children. The parent acts as an orchestrator, deciding how child actions affect sibling components or global state.

## 7. Shared State

When multiple screens need access to the same data (e.g., the currently logged-in User, or a global configuration), store that data in the parent's state and pass it down as arguments to the child's `view` and `update` functions.

```rust
pub struct GlobalState {
    pub current_user: User,
    pub theme: Theme,
}

pub struct App {
    global: GlobalState,
    screen: Screen,
}

// Passing global state to the child
fn view(&self) -> Element<Message> {
    match &self.screen {
        Screen::Profile(profile_state) => {
            // The child profile screen needs user info to render
            profile_state.view(&self.global.current_user).map(Message::ProfileMsg)
        }
    }
}
```

By doing this, you avoid syncing issues. The source of truth remains in the parent, and children simply compute their views based on it.

## 8. Multi-Window Applications

For truly complex applications, you may want to spawn entirely new OS windows. This is done via `window::open()`.

```rust
use taim::window;

pub fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::OpenSettingsWindow => {
            // Request the runtime to open a new window
            window::open(window::Settings::default())
        }
    }
}
```

When managing multiple windows, your application `Message` enum will often need to include a `window::Id` to determine which window generated the event, so you can update the state for that specific window.

## 9. Project Structure

As your application grows, organize your files logically. A recommended structure for a medium-to-large Taim app:

```
src/
├── main.rs         # Entry point, configures Application
├── app.rs          # The root App state, message routing
├── global.rs       # Shared data models (User, Config)
├── screens/        # Top-level views
│   ├── mod.rs
│   ├── home.rs
│   ├── settings.rs
│   └── dashboard.rs
├── components/     # Reusable UI widgets
│   ├── mod.rs
│   ├── header.rs
│   └── user_card.rs
└── style/          # Custom styling and themes
    ├── mod.rs
    ├── colors.rs
    └── text.rs
```

## 10. Complete Example

Here is a simplified but complete example showing a Parent app managing two screens: Home and Settings, sharing a global user state.

```rust
use taim::{Element, Task, widget::{column, button, text}};

// --- GLOBAL SHARED STATE ---
#[derive(Clone)]
pub struct User {
    pub name: String,
}

// --- CHILD 1: HOME SCREEN ---
pub mod home {
    use super::*;

    pub struct State;

    #[derive(Debug, Clone)]
    pub enum Message {
        GoToSettings,
    }

    pub enum Action {
        None,
        NavigateToSettings,
    }

    impl State {
        pub fn update(&mut self, message: Message) -> Action {
            match message {
                Message::GoToSettings => Action::NavigateToSettings,
            }
        }

        pub fn view(&self, user: &User) -> Element<Message> {
            column![
                text(format!("Welcome, {}!", user.name)).size(30),
                button("Open Settings").on_press(Message::GoToSettings),
            ].into()
        }
    }
}

// --- CHILD 2: SETTINGS SCREEN ---
pub mod settings {
    use super::*;

    pub struct State;

    #[derive(Debug, Clone)]
    pub enum Message {
        GoHome,
        ChangeName(String),
    }

    pub enum Action {
        None,
        NavigateHome,
        UpdateName(String),
    }

    impl State {
        pub fn update(&mut self, message: Message) -> Action {
            match message {
                Message::GoHome => Action::NavigateHome,
                Message::ChangeName(new) => Action::UpdateName(new),
            }
        }

        pub fn view(&self, user: &User) -> Element<Message> {
            column![
                text("Settings").size(30),
                text(format!("Current Name: {}", user.name)),
                button("Change to 'Alice'").on_press(Message::ChangeName("Alice".into())),
                button("Back to Home").on_press(Message::GoHome),
            ].into()
        }
    }
}

// --- PARENT APPLICATION ---
pub struct App {
    user: User,
    screen: Screen,
}

pub enum Screen {
    Home(home::State),
    Settings(settings::State),
}

#[derive(Debug, Clone)]
pub enum Message {
    HomeMsg(home::Message),
    SettingsMsg(settings::Message),
}

impl App {
    pub fn new() -> Self {
        Self {
            user: User { name: "Bob".into() },
            screen: Screen::Home(home::State),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::HomeMsg(msg) => {
                if let Screen::Home(state) = &mut self.screen {
                    match state.update(msg) {
                        home::Action::None => return Task::none(),
                        home::Action::NavigateToSettings => {
                            self.screen = Screen::Settings(settings::State);
                        }
                    }
                }
            }
            Message::SettingsMsg(msg) => {
                if let Screen::Settings(state) = &mut self.screen {
                    match state.update(msg) {
                        settings::Action::None => return Task::none(),
                        settings::Action::NavigateHome => {
                            self.screen = Screen::Home(home::State);
                        }
                        settings::Action::UpdateName(new_name) => {
                            self.user.name = new_name;
                        }
                    }
                }
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        match &self.screen {
            Screen::Home(state) => {
                state.view(&self.user).map(Message::HomeMsg)
            }
            Screen::Settings(state) => {
                state.view(&self.user).map(Message::SettingsMsg)
            }
        }
    }
}
```

This architecture provides strong boundaries, testability, and clarity. The `App` orchestrates data flow and navigation, while the screens focus purely on local interactions and rendering.
