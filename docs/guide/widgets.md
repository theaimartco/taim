# Vyom-Aura Widget Reference Guide

This comprehensive reference guide covers the vast array of widgets available in Vyom-Aura, categorized by function. It provides detailed documentation, methods, types, styling, and robust, realistic code examples for each widget.

---

## 1. Layout Widgets

### Column
A container that distributes its contents vertically.

**Constructor:** `column![...children]` or `Column::new()` where children are `impl Into<Element>`

**Key Methods:**
- `.spacing(spacing: impl Into<f32>)` — sets the vertical space between elements
- `.padding(padding: impl Into<Padding>)` — default: `0`
- `.width(length: impl Into<Length>)` — default: `Shrink`
- `.height(length: impl Into<Length>)` — default: `Shrink`
- `.max_width(length: impl Into<Length>)`
- `.align_x(alignment: alignment::Horizontal)` — default: `Start`
- `.push(child: impl Into<Element>)`
- `.extend(children: impl IntoIterator<Item = impl Into<Element>>)`
- `.clip(bool)` — clip overflow

**Important Notes:** Use `spacing` and `padding` judiciously to maintain consistent design rhythms. For dynamic lists, `Column` works well until it exceeds a few hundred items; for massive datasets, consider `VirtualList`.

```rust
// A form layout using Column
column![
    text("User Profile").size(24).font(font::bold()),
    text_input("Enter username...", &state.username)
        .on_input(Message::UsernameChanged),
    text_input("Enter email...", &state.email)
        .on_input(Message::EmailChanged),
    row![
        button("Cancel").on_press(Message::Cancel).style(button::secondary),
        button("Save Profile").on_press(Message::Save).style(button::primary)
    ].spacing(16).align_y(alignment::Vertical::Center)
]
.spacing(20)
.padding(32)
.width(Length::Fill)
.max_width(500)
.align_x(alignment::Horizontal::Center)
```

### Row
A container that distributes its contents horizontally.

**Constructor:** `row![...children]` or `Row::new()`

**Key Methods:**
- `.spacing(spacing: impl Into<f32>)`
- `.padding(padding: impl Into<Padding>)`
- `.width(length: impl Into<Length>)`
- `.height(length: impl Into<Length>)`
- `.align_y(alignment: alignment::Vertical)` — default: `Start`
- `.push(child: impl Into<Element>)`
- `.wrap()` — Allows items to wrap onto the next line if they exceed the width

```rust
// A dashboard top navigation bar
row![
    image("logo.png").width(40).height(40),
    text("Vyom Dashboard").size(20).font(font::bold()),
    Space::with_width(Length::Fill), // Push remaining items to the right
    text(format!("Welcome, {}", state.user_name)),
    button("Logout")
        .on_press(Message::Logout)
        .style(button::danger)
]
.spacing(16)
.padding([12, 24])
.width(Length::Fill)
.align_y(alignment::Vertical::Center)
```

### Container
A layout element that aligns a single widget within its bounds and applies styling (backgrounds, borders, shadows).

**Constructor:** `container(content)` where content is `impl Into<Element>`

**Key Methods:**
- `.width(length: impl Into<Length>)`
- `.height(length: impl Into<Length>)`
- `.padding(padding: impl Into<Padding>)`
- `.center_x(length: impl Into<Length>)` — Centers content horizontally (shortcut for `width` + `align_x`)
- `.center_y(length: impl Into<Length>)` — Centers content vertically
- `.align_x(alignment: alignment::Horizontal)`
- `.align_y(alignment: alignment::Vertical)`
- `.style(fn(&Theme) -> Style)` — custom background, border, text color, and shadow

**Style functions:** `transparent`, `bordered_box`, `card`, `dark_background`

```rust
// A styled card container centering its content
container(
    column![
        text("Analytics Summary").size(18),
        text("Visits: 1,204").style(color::subtle()),
    ].spacing(8)
)
.padding(24)
.width(300)
.center_x(Length::Fill) // Center content horizontally inside the container bounds
.style(container::card)
```

### Stack
A layout that places its children on top of each other along the z-axis.

**Constructor:** `stack![...children]` or `Stack::new()`

**Key Methods:**
- `.width(length: impl Into<Length>)`
- `.height(length: impl Into<Length>)`
- `.push(child: impl Into<Element>)`
- `.alignment(alignment: Alignment)` — Aligns all children

**Notes:** Elements drawn later (pushed last) appear on top. Great for overlays, badges, or floating action buttons.

```rust
// A profile picture with a notification badge
stack![
    image("avatar.jpg")
        .width(64)
        .height(64)
        .border_radius(32.0),
    
    // Positioned badge at top right
    container(
        text("3")
            .size(12)
            .style(color::white())
    )
    .padding([2, 6])
    .style(container::badge_danger)
    // Wrap in a container to position it within the stack
    // (In reality, often used with `Align` or nested containers)
]
.width(64)
.height(64)
```

### Grid
A layout that distributes elements in a two-dimensional grid.

**Constructor:** `Grid::new()` (Often wrapped or utilized via a specialized Grid API in Vyom)

**Key Methods:**
- `.columns(columns: usize)` — Define number of columns
- `.spacing(spacing: impl Into<f32>)`
- `.column_spacing(spacing: impl Into<f32>)`
- `.row_spacing(spacing: impl Into<f32>)`
- `.push(child: impl Into<Element>)`

```rust
// A simple 3-column photo gallery
let mut gallery = Grid::new().columns(3).spacing(10);
for photo in &state.photos {
    gallery = gallery.push(
        image(photo.path.clone())
            .width(Length::Fill)
            .height(200)
            .content_fit(ContentFit::Cover)
    );
}

container(gallery).padding(20).width(Length::Fill)
```

### Space
Empty space used for flexible padding or pushing elements apart.

**Constructor:** `Space::new(width, height)` or `Space::with_width(w)` / `Space::with_height(h)`

**Key Methods:**
- N/A, constructor defines sizing. Use `Length::Fill` to consume remaining space.

```rust
row![
    text("Left side"),
    Space::with_width(Length::Fill), // Acts as a flex spacer
    text("Right side")
]
```

---

## 2. Scrolling & Lists

### Scrollable
A container that allows scrolling its content when it overflows the available bounds.

**Constructor:** `scrollable(content)`

**Key Methods:**
- `.direction(direction: scrollable::Direction)` — `Vertical` (default), `Horizontal`, `Both`
- `.width(length: impl Into<Length>)`
- `.height(length: impl Into<Length>)`
- `.scrollbar_width(width: f32)`
- `.scrollbar_margin(margin: f32)`
- `.scroller_width(width: f32)`
- `.on_scroll(f: impl Fn(scrollable::Viewport) -> Message)` — Triggered on scroll event
- `.id(id: scrollable::Id)` — Used to programmatically control scroll position

**Status:** `Active`, `Hovered`, `Dragging`

**Gotchas:** Scrollables must have a bounded size (e.g., `Length::Fill`, or fixed) to work. If placed inside an unbounded container (like an unconstrained Column), it will panic or stretch infinitely.

```rust
// A vertically scrollable list of messages
let content = column(
    state.messages.iter().map(|msg| {
        text(msg.clone()).into()
    })
).spacing(10);

scrollable(content)
    .direction(scrollable::Direction::Vertical)
    .height(Length::Fill)
    .width(Length::Fill)
    .on_scroll(Message::Scrolled)
    .id(state.scroll_id.clone())
```

### VirtualList
An optimized list that only renders the items currently visible in the viewport, essential for large datasets.

**Constructor:** `VirtualList::new(items, view_fn)`
- `items`: The underlying slice/array of data
- `view_fn`: A closure `|item_ref| -> Element`

**Key Methods:**
- `.item_height(height: f32)` — Required for fast offset calculations (uniform heights)

**WARNING:** A `VirtualList` MUST be wrapped inside a `Scrollable` to function properly. It relies on the Scrollable's viewport bounds to cull off-screen items.

```rust
// Handling a list of 10,000+ items
let v_list = VirtualList::new(
    &state.huge_dataset,
    |item| {
        row![
            text(format!("Item {}", item.id)).width(100),
            text(item.description.clone())
        ]
        .padding(10)
        .into()
    }
).item_height(40.0);

// Must be wrapped in a scrollable!
scrollable(v_list)
    .height(Length::Fill)
```

---

## 3. Responsive & Positioning

### Responsive
A widget that adapts its content based on the actual layout bounds available at runtime.

**Constructor:** `responsive(|size: Size| -> Element)`

**Key Methods:** N/A (Logic handled inside the closure)

```rust
// Switch layout based on width
responsive(|size| {
    if size.width < 600.0 {
        // Mobile view
        column![
            sidebar_content(),
            main_content()
        ].into()
    } else {
        // Desktop view
        row![
            container(sidebar_content()).width(250),
            container(main_content()).width(Length::Fill)
        ].into()
    }
})
```

### Adaptive
Similar to responsive but strictly for choosing between discreet UI modes (e.g. compact vs expanded).

**Constructor:** `adaptive(|mode| -> Element)`

### SafeArea
A container that respects the system safe areas (notches, taskbars, bottom bars on mobile/desktop).

**Constructor:** `safe_area(content)`

```rust
// Prevents content from going under iOS notches or Android navigation bars
safe_area(
    column![
        header(),
        scrollable(content)
    ]
)
```

### Pin
Pins an element to a specific coordinate/bounding box within a container.

**Constructor:** `pin(content).x(x).y(y)`

### Float
Floats an element above other content, anchoring it to a specific corner or edge.

**Constructor:** `float(base_content, floating_content)`

**Key Methods:**
- `.alignment(alignment: alignment::Alignment)` — e.g. TopRight, BottomRight
- `.offset(offset: Vector)` — nudge the floating element

```rust
// Floating Action Button (FAB)
float(
    scrollable(main_list), // Base
    button("+").padding(16).border_radius(32).on_press(Message::CreateNew) // Floating
)
.alignment(alignment::BottomRight)
.offset(Vector::new(-20.0, -20.0))
```

---

## 4. Input & Interactive Widgets

### Button
A clickable element that emits a message when pressed.

**Constructor:** `button(content)` where content is `impl Into<Element>`

**Key Methods:**
- `.on_press(message: Message)` — message emitted on click
- `.on_press_with(f: impl Fn() -> Message)` — lazy message (for closures)
- `.on_press_maybe(option: Option<Message>)` — None = disabled
- `.width(length: impl Into<Length>)` — default: Fit
- `.height(length: impl Into<Length>)` — default: Fit
- `.padding(padding: impl Into<Padding>)` — default: [5, 10]
- `.clip(bool)` — clip overflow
- `.style(fn(&Theme, Status) -> Style)` — custom styling

**Status:** `Active`, `Hovered`, `Pressed`, `Disabled`
**Style functions:** `primary`, `secondary`, `success`, `warning`, `danger`, `text`, `background`, `subtle`

**Disabled state:** A button without `.on_press()` is automatically disabled.

```rust
// Primary action button
button(text("Save Changes").size(16))
    .on_press(Message::Save)
    .padding([8, 16])
    .style(button::primary)

// Conditionally disabled
button("Submit")
    .on_press_maybe(if state.is_valid {
        Some(Message::Submit)
    } else {
        None
    })

// Danger button with Icon
button(row![icon('trash'), text("Delete")].spacing(8))
    .on_press(Message::Delete)
    .style(button::danger)
```

### TextInput
A field for single-line text input.

**Constructor:** `text_input(placeholder: &str, value: &str)`

**Key Methods:**
- `.on_input(f: impl Fn(String) -> Message)` — emits on keystroke
- `.on_submit(message: Message)` — emits on Enter key
- `.password()` — Obscures text (secure mode)
- `.icon(icon: Icon)` — Adds a leading icon
- `.width(length: impl Into<Length>)`
- `.padding(padding: impl Into<Padding>)`
- `.size(size: impl Into<f32>)`
- `.id(id: text_input::Id)` — For focus management

**Status:** `Active`, `Hovered`, `Focused`, `Disabled`

```rust
text_input("Enter your password", &state.password)
    .on_input(Message::PasswordChanged)
    .on_submit(Message::LoginRequested)
    .password()
    .icon(icon::lock())
    .padding(12)
    .width(Length::Fill)
```

### TextEditor
A multi-line text editing field.

**Constructor:** `text_editor(content: &text_editor::Content)`

**Key Methods:**
- `.on_action(f: impl Fn(text_editor::Action) -> Message)` — Handle edits, cursor moves, selections
- `.font(font: Font)`
- `.height(length: impl Into<Length>)`

```rust
// Multi-line editor for writing a bio or code
text_editor(&state.editor_content)
    .on_action(Message::EditorAction)
    .height(300)
    .font(font::monospace())
```

### Slider
A horizontal slider for selecting a numeric value from a range.

**Constructor:** `slider(range: RangeInclusive<T>, value: T, on_change: impl Fn(T) -> Message)`

**Key Methods:**
- `.step(step: T)` — Step value
- `.on_release(message: Message)` — Emitted when user stops dragging
- `.width(length: impl Into<Length>)`
- `.style(fn(&Theme, Status) -> Style)`

**Status:** `Active`, `Hovered`, `Dragged`

```rust
slider(0.0..=100.0, state.volume, Message::VolumeChanged)
    .step(1.0)
    .on_release(Message::SaveVolume)
    .width(200)
```

### VerticalSlider
A vertical version of the slider. Identical API to `slider`, just visual orientation differs.

```rust
vertical_slider(0..=255, state.red_channel, Message::RedChanged)
    .height(150)
```

### Checkbox
A binary toggle control with a label.

**Constructor:** `checkbox(label: impl Into<String>, is_checked: bool)`

**Key Methods:**
- `.on_toggle(f: impl Fn(bool) -> Message)` — Missing = disabled
- `.size(size: f32)` — Checkbox square size
- `.spacing(spacing: f32)` — Space between box and label
- `.text_size(size: f32)`
- `.text_shaping(shaping: text::Shaping)`

```rust
checkbox("I agree to the Terms & Conditions", state.agreed)
    .on_toggle(Message::AgreementToggled)
    .size(24)
    .spacing(12)
```

### Radio
A single-select control from a group.

**Constructor:** `radio(label: impl Into<String>, value: V, selected: Option<V>, on_click: impl Fn(V) -> Message)`

```rust
column![
    radio("Light Theme", ThemeType::Light, Some(state.theme), Message::ThemeSelected),
    radio("Dark Theme", ThemeType::Dark, Some(state.theme), Message::ThemeSelected),
    radio("System Auto", ThemeType::System, Some(state.theme), Message::ThemeSelected),
].spacing(10)
```

### Toggler
A modern switch (iOS-style switch) for boolean states.

**Constructor:** `toggler(label: impl Option<String>, is_checked: bool)`

**Key Methods:**
- `.on_toggle(f: impl Fn(bool) -> Message)`
- `.width(length: impl Into<Length>)`

```rust
toggler(Some("Enable Notifications".to_string()), state.notifications)
    .on_toggle(Message::ToggleNotifications)
    .width(Length::Shrink)
```

### PickList
A dropdown menu for selecting a single item from a list.

**Constructor:** `pick_list(options: impl Into<Cow<[T]>>, selected: Option<T>, on_selected: impl Fn(T) -> Message)`

**Key Methods:**
- `.placeholder(text: impl Into<String>)`
- `.text_size(size: f32)`
- `.padding(padding: Padding)`
- `.handle(handle: pick_list::Handle)` — Customize the dropdown arrow icon
- `.width(length: impl Into<Length>)`

**Gotchas:** `T` must implement `ToString`, `Clone`, and `PartialEq`.

```rust
let languages = vec![Lang::Rust, Lang::Python, Lang::Go, Lang::JS];

pick_list(
    languages,
    state.selected_language.clone(),
    Message::LanguageSelected
)
.placeholder("Select a language...")
.padding(10)
.width(200)
```

### ComboBox
An advanced PickList that allows typing to filter/search the options.

**Constructor:** `combo_box(state: &combo_box::State<T>, placeholder: &str, selected: Option<&T>, on_selected: impl Fn(T) -> Message)`

**Gotchas:** Requires external state management (`combo_box::State`) to handle the filtering logic and search text.

```rust
combo_box(
    &state.country_combo,
    "Search countries...",
    state.selected_country.as_ref(),
    Message::CountrySelected
)
.width(250)
```

---

## 5. Content Widgets

### Text
Displays string content.

**Constructor:** `text(content: impl ToString)`

**Key Methods:**
- `.size(size: impl Into<f32>)`
- `.font(font: Font)`
- `.width(length: impl Into<Length>)`
- `.height(length: impl Into<Length>)`
- `.horizontal_alignment(alignment: alignment::Horizontal)`
- `.vertical_alignment(alignment: alignment::Vertical)`
- `.shaping(shaping: text::Shaping)` — `Basic` or `Advanced` (needed for complex scripts/ligatures)
- `.line_height(line_height: text::LineHeight)`
- `.style(color: impl Into<Color>)`

**Gotchas:** To enforce wrapping, you MUST set `.width(Length::Fill)` or a fixed width. Unconstrained text will render on a single infinite line.

```rust
text("This is a very long paragraph that needs to wrap appropriately within its container constraints.")
    .size(16)
    .width(Length::Fill) // Crucial for wrapping!
    .line_height(text::LineHeight::Relative(1.5))
    .horizontal_alignment(alignment::Horizontal::Justify)
    .shaping(text::Shaping::Advanced)
```

### RichText / Span
Allows inline styling of text (multiple colors/fonts in one paragraph).

**Constructor:** `rich_text(spans: impl IntoIterator<Item = Span>)`

```rust
rich_text([
    span("Vyom ").font(font::bold()).color(color::PRIMARY),
    span("is a modern GUI toolkit written in "),
    span("Rust.").font(font::bold()),
])
.size(18)
.width(Length::Fill)
```

### Image
Renders raster images (PNG, JPG, BMP).

**Constructor:** `image(handle: impl Into<image::Handle>)`

**Key Methods:**
- `.width(length: impl Into<Length>)`
- `.height(length: impl Into<Length>)`
- `.content_fit(fit: ContentFit)` — `Contain`, `Cover`, `Fill`, `None`, `ScaleDown`
- `.border_radius(radius: impl Into<BorderRadius>)`
- `.opacity(opacity: f32)`
- `.rotation(angle: f32)`

**Gotchas:** File paths in `image("path.png")` are resolved relative to the current working directory, not the source file. It's often safer to use `include_bytes!` for small bundled assets.

```rust
image("assets/profile_pic.jpg")
    .width(100)
    .height(100)
    .content_fit(ContentFit::Cover)
    .border_radius(50.0) // Makes it circular
    .opacity(0.9)
```

### Svg
Renders scalable vector graphics.

**Constructor:** `svg(handle: impl Into<svg::Handle>)`

**Key Methods:** Same layout methods as `Image`.
- `.style(fn(&Theme) -> Style)` — SVGs can be re-colored dynamically if the vector supports it (e.g. using `currentColor` in the SVG file).

```rust
svg(svg::Handle::from_memory(include_bytes!("../assets/icon.svg").as_slice()))
    .width(24)
    .height(24)
    .style(svg::color(color::PRIMARY)) // Tints the SVG
```

---

## 6. Display Widgets

### ProgressBar
A visual indicator of progress.

**Constructor:** `progress_bar(range: RangeInclusive<f32>, value: f32)`

**Key Methods:**
- `.height(length: impl Into<Length>)`
- `.style(fn(&Theme) -> Style)`

```rust
progress_bar(0.0..=100.0, state.download_progress)
    .height(8)
    .style(progress_bar::success)
```

### Rule
A horizontal or vertical line used as a divider.

**Constructor:** `rule(direction: Rule::Horizontal | Rule::Vertical)` (Usually just `horizontal_rule(spacing)` or `vertical_rule(spacing)`)

```rust
column![
    text("Top Section"),
    horizontal_rule(10), // 10 is the margin/spacing around the rule
    text("Bottom Section"),
]
```

### QRCode
Generates a 2D QR code from data.

**Constructor:** `qr_code(data: &qrcode::Data)`

```rust
// Generate once in state, not on every frame!
// state.qr_data = qrcode::Data::new("https://vyom.dev").unwrap();

qr_code(&state.qr_data)
    .cell_size(5)
```

### Markdown
Renders rich markdown text.

**Constructor:** `markdown(items: &[markdown::Item])`

**Gotchas:** Markdown must be parsed from string into `Item`s beforehand using `markdown::parse()`. Do not parse within the `view` function for performance reasons.

```rust
// Assuming state.parsed_markdown holds the items
markdown(&state.parsed_markdown)
```

---

## 7. Advanced Widgets

### Navigator
Manages a stack of pages and handles transitions.

**Constructor:** `navigator()` (Usually wrapped around the root view)

**Key Methods:**
- `.transition(PageTransition::SlideRight)`

### Dialog
A modal dialog overlay that blocks interaction with the content beneath it.

**Constructor:** `dialog(base_content, dialog_content, is_showing: bool)`

```rust
// A confirmation modal
dialog(
    main_view(),
    container(
        column![
            text("Are you sure you want to delete this?"),
            row![
                button("No").on_press(Message::CancelDelete),
                button("Yes, Delete").on_press(Message::ConfirmDelete).style(button::danger)
            ].spacing(10)
        ].spacing(20)
    ).padding(30).style(container::card),
    state.show_modal
)
.backdrop(color::from_rgba(0,0,0,0.5)) // Darken background
.on_close(Message::CancelDelete)
```

### Tooltip
Displays floating text when hovering over another widget.

**Constructor:** `tooltip(content: impl Into<Element>, tooltip_text: impl ToString, position: tooltip::Position)`

**Key Methods:**
- `.gap(gap: f32)`
- `.snap_within_viewport(snap: bool)` — Prevents the tooltip from being clipped by the screen edges

```rust
tooltip(
    icon('info'),
    "This setting requires a restart to take effect.",
    tooltip::Position::Top
)
.gap(8.0)
.snap_within_viewport(true)
```

### MouseArea
Detects mouse interactions (hover, enter, exit, custom cursor) without being a button.

**Constructor:** `mouse_area(content)`

**Key Methods:**
- `.on_enter(message: Message)`
- `.on_exit(message: Message)`
- `.on_press(message: Message)`
- `.on_right_press(message: Message)`
- `.interaction(cursor: interaction::Interaction)` — Change mouse cursor (e.g., `Pointer`, `Text`, `Crosshair`)

```rust
mouse_area(
    container(text("Hover me")).padding(20).style(container::bordered_box)
)
.on_enter(Message::HoverStarted)
.on_exit(Message::HoverEnded)
.interaction(interaction::Interaction::Pointer) // Shows hand cursor
```

### Sensor
A specialized widget that reports geometry/visibility changes.

**Constructor:** `sensor(content, on_change: impl Fn(sensor::Event) -> Message)`

**Notes:** Used for advanced visibility tracking (e.g., "is this element on screen?", infinite scrolling triggers).

### PaneGrid
A robust tiling window manager layout (like Blender or modern IDEs).

**Constructor:** `PaneGrid::new(state: &pane_grid::State, view_fn)`

**Key Methods:**
- `.on_drag(f)`
- `.on_resize(f)`
- `.spacing(f32)`

**Notes:** Extremely powerful but requires complex state management (`pane_grid::State`) to handle splits, resizing, and closing panes.

---

## 8. Visuals & Custom Rendering

### Canvas
A blank slate for drawing 2D vector graphics (lines, paths, text, shapes).

**Constructor:** `Canvas::new(program: impl canvas::Program)`

**Key Methods:**
- `.width(Length)`
- `.height(Length)`

**Gotchas:** Requires implementing the `Program` trait, which defines `draw()` and `update()`. Excellent for charts, graphs, or custom UI elements that standard widgets can't handle.

```rust
// Canvas requires a custom struct implementing `Program`
struct MyChart { data: Vec<f32> }
impl canvas::Program<Message> for MyChart {
    type State = ();
    fn draw(&self, _state: &Self::State, renderer: &Renderer, _theme: &Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        // Draw paths, fill with color, etc.
        // frame.fill(&Path::circle(bounds.center(), 50.0), color::BLACK);
        vec![frame.into_geometry()]
    }
}

// In view:
Canvas::new(MyChart { data: state.data.clone() })
    .width(Length::Fill)
    .height(300)
```

### Shader
For rendering custom WGPU shaders (GLSL/WGSL). Advanced feature for high-performance graphics.

**Constructor:** `shader(program: impl shader::Program)`

### Transition
Animates a value (like width, opacity, color) over time and applies it to a widget.

### Themer
A widget that overrides the current theme for its children.

**Constructor:** `themer(theme: Theme, content)`

```rust
// Forces dark mode just for this specific panel
themer(
    Theme::Dark,
    container(
        text("I am always in dark mode!")
    ).style(container::background)
)
```

---
*End of Reference. For further details on trait implementations or custom widgets, consult the Custom Widget Guide.*
