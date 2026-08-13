# Styling in Vyom Aura

Vyom Aura takes a unique, highly structured approach to styling that ensures your application always looks consistent, professional, and accessible. Instead of manually specifying hex codes and pixel borders on every widget, Aura uses a comprehensive **Theme** and **Style Catalog** system.

This guide will take you from applying basic built-in themes to building complex custom styling engines.

---

## 1. The 22+ Built-in Themes

Aura ships with a vast array of high-quality, pre-built themes out of the box. They are carefully designed with proper contrast ratios and aesthetic harmony.

You can categorize them roughly into:

### Light Themes
- **Light**: The default clean, crisp light interface.
- **CatppuccinLatte**: Soft, pastel light theme.
- **GruvboxLight**: Warm, retro, earthy tones.
- **SolarizedLight**: Classic low-contrast reading theme.
- **TokyoNightLight**: Bright and vibrant.

### Dark Themes
- **Dark**: The standard sleek, modern dark mode.
- **Dracula**: High-contrast, vibrant dark theme with pinks and purples.
- **CatppuccinMocha / Macchiato / Frappe**: Extremely popular soft dark themes.
- **TokyoNight**: Deep blues and neon accents.
- **Nord**: Arctic, cold, bluish-gray dark theme.
- **GruvboxDark**: Warm, earthy, retro dark theme.
- **SolarizedDark**: Classic low-contrast dark theme.
- **Oxocarbon**: Ultra-minimalist, high-contrast dark theme (IBM Carbon inspired).

### Colorful & Unique
- **FairyFloss**: Pastel pinks and purples.
- **Kanagawa**: Inspired by Japanese woodblock prints.
- **Moonfly**: Deep night sky theme.

---

## 2. Setting the Theme

### Static Theming
If your app only needs one theme, return it from your `Application::theme` method:

```rust
use aura::Theme;

impl Application for MyApp {
    // ... other methods ...

    fn theme(&self) -> Theme {
        Theme::TokyoNight
    }
}
```

### Dynamic Theming (State-Based)
Most modern apps allow users to toggle themes. Store the active theme in your application state:

```rust
struct MyApp {
    active_theme: Theme,
}

impl Application for MyApp {
    fn theme(&self) -> Theme {
        self.active_theme.clone()
    }
}
```

### Environment Variable (`AURA_THEME`)
For quick testing without recompiling, users can override the default theme by setting an environment variable:
`AURA_THEME=dracula cargo run`

---

## 3. Custom Themes

If none of the 22 built-in themes match your brand, you can create a completely custom theme.

### `Theme::custom(name, palette)`
You can define a theme by providing a `Palette`.

```rust
use aura::theme::{Theme, Palette};
use aura::color;

let my_palette = Palette {
    primary: color!(0x3b82f6),
    secondary: color!(0x8b5cf6),
    success: color!(0x22c55e),
    danger: color!(0xef4444),
    background: color!(0x0f172a),
    text: color!(0xf8fafc),
};

let my_theme = Theme::custom("MyBrand".into(), my_palette);
```

### `Theme::custom_with_fn`
For extreme control, you can define exactly how styles are generated based on the palette, overriding the default style generators.

---

## 4. The Palette Deep Dive

Every `Theme` is driven by a `Palette`. The palette defines semantic colors rather than specific widget colors.

- **Primary**: The main brand color (used for active buttons, focused inputs).
- **Secondary**: Accents and alternative actions.
- **Success**: Positive actions (saving, completing).
- **Danger**: Destructive actions (deleting, errors).
- **Background**: The base canvas color.
- **Text**: Default text color.

### Extracting Colors
Sometimes you need to draw something manually (like on a Canvas) and need the theme's colors:

```rust
let current_theme = Theme::Dracula;
let palette = current_theme.palette();

let brand_color = palette.primary;
```

---

## 5. Widget Styling: The Style + Status Pattern

This is the most critical concept in Aura's styling engine.

Widgets in Aura do not have methods like `.background_color()` or `.border_radius()`. Instead, they take a **Style Function**.

### How `.style(fn)` works

When Aura draws a widget, it needs to know what it looks like in different states (Hovered, Pressed, Disabled, Active).

A Style Function has this signature (conceptually):
`fn(theme: &Theme, status: Status) -> Appearance`

For example, a Button's style function looks like:
`fn button_style(theme: &Theme, status: button::Status) -> button::Appearance`

The `button::Status` enum defines the states:
```rust
pub enum Status {
    Active,
    Hovered,
    Pressed,
    Disabled,
}
```

The `button::Appearance` struct defines the actual visual properties:
```rust
pub struct Appearance {
    pub background: Option<Background>,
    pub text_color: Color,
    pub border: Border,
    pub shadow: Shadow,
}
```

### Using Pre-built Style Functions
Aura's default `Theme` implements hundreds of these style functions for you. You apply them to widgets using the `.style()` method.

```rust
use aura::widget::button;
use aura::theme::Theme;

// Uses the default 'primary' style
button("Submit").style(button::primary)
```

---

## 6. Style Functions for Every Widget

Here are the most common pre-built style functions available for standard widgets:

### Button
```rust
button::primary    // Solid brand color
button::secondary  // Outlined or alternative color
button::success    // Green/Positive
button::danger     // Red/Destructive
button::text       // No background, just text (like a link)
```

### Container
Containers are the most frequently styled widgets.
```rust
container::background    // Standard app background color
container::transparent   // No background (default)
container::rounded_box   // Background with rounded corners
container::bordered_box  // Transparent with a border
container::dark          // Darker variant of background
container::primary       // Solid primary color
```

### Text
```rust
text::default      // Standard body text
text::primary      // Brand colored text
text::danger       // Error text
```

### Other Widgets
Checkbox, ProgressBar, Slider, Toggler, PickList, etc., all have `primary`, `success`, and `danger` variants.

---

## 7. Container Styling Deep Dive

If you want to create a totally unique looking element (like a glassy card), you create a custom style function for a `Container`.

```rust
use aura::widget::container;
use aura::{Color, Border, Shadow, Theme};
use aura::widget::container::Appearance;

fn glassmorphism_style(theme: &Theme) -> Appearance {
    Appearance {
        // Semi-transparent white
        background: Some(Color::from_rgba8(255, 255, 255, 0.1).into()),
        border: Border {
            color: Color::from_rgba8(255, 255, 255, 0.2),
            width: 1.0,
            radius: 12.0.into(), // Rounded corners
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.1),
            offset: aura::Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        text_color: Some(theme.palette().text),
    }
}

// Apply it
container(text("Glass Card")).style(glassmorphism_style)
```

---

## 8. The `.class()` Method

While `.style(fn)` is powerful, passing function pointers can be tedious. Aura introduced `.class()` as a cleaner alternative.

Every theme maintains a "Catalog" of classes.

```rust
// Instead of this:
button("Save").style(button::success)

// You can do this:
button("Save").class(aura::theme::ButtonClass::Success)
```

Classes are enums, making them easier to store in state, pass around, or serialize.

---

## 9. Root Application Style

By default, an Aura window has a neutral background. To ensure your entire application respects the Theme's background color, apply a style to the root node of your `view()` method:

```rust
impl Application for MyApp {
    fn view(&self) -> Element<Message> {
        let content = text("Hello World");
        
        // Wrap everything in a container styled with the background
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(container::background)
            .into()
    }
}
```

---

## 10. Complete Example: Dark Mode Toggle

Here is how you wire up a fully working Light/Dark mode toggle:

```rust
use aura::{Application, Theme, Element, Length};
use aura::widget::{column, button, text, container};

struct App {
    is_dark: bool,
}

#[derive(Clone, Debug)]
enum Message {
    ToggleTheme,
}

impl Application for App {
    type Message = Message;
    // ...

    fn theme(&self) -> Theme {
        if self.is_dark {
            Theme::TokyoNight
        } else {
            Theme::Light
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::ToggleTheme => {
                self.is_dark = !self.is_dark;
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let toggle_btn = button("Toggle Theme")
            .on_press(Message::ToggleTheme)
            .style(button::primary);

        let content = column![
            text("Welcome to Aura!").size(40),
            toggle_btn,
        ].spacing(20);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .style(container::background) // Crucial for background color updates
            .into()
    }
}
```
