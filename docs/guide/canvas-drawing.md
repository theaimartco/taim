# Custom Drawing with Canvas

The `Canvas` widget gives you full control over 2D rendering. It's perfect for charts, diagrams, custom visualizations, drawing apps, and any UI that can't be expressed with standard widgets.

> **Feature gate**: Requires `features = ["canvas"]` in your `Cargo.toml`.

---

## Basic Setup

To use the Canvas, you implement the `canvas::Program` trait. This trait has three methods:

- **`draw()`** — Called every frame to produce geometry. This is where you draw shapes, text, and images.
- **`update()`** — Called when user events (mouse, keyboard) occur within the canvas bounds. Return `None` to ignore, or `Some(Action)` to emit a message or request a redraw.
- **`mouse_interaction()`** — Return a cursor style based on the current state and cursor position.

```rust
use taim::widget::canvas::{self, Canvas};
use taim::mouse;
use taim::{Color, Element, Point, Rectangle, Renderer, Theme};

struct MyDrawing;

impl<Message> canvas::Program<Message> for MyDrawing {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        // Create a frame to draw on
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Draw a blue circle
        let circle = canvas::Path::circle(
            Point::new(bounds.width / 2.0, bounds.height / 2.0),
            50.0,
        );
        frame.fill(&circle, Color::from_rgb(0.2, 0.5, 1.0));

        // Draw a red border around it
        frame.stroke(
            &circle,
            canvas::Stroke::default()
                .with_color(Color::from_rgb(1.0, 0.2, 0.2))
                .with_width(3.0),
        );

        vec![frame.into_geometry()]
    }
}
```

## Using Canvas in Your View

```rust
fn view(state: &State) -> Element<'_, Message> {
    canvas(MyDrawing)
        .width(taim::Fill)
        .height(taim::Fill)
        .into()
}
```

---

## Drawing Primitives

The `canvas::Frame` provides a rich set of drawing operations:

### Paths

Paths define shapes. You can create them from built-in helpers or build custom ones:

```rust
// Built-in path constructors
let line = canvas::Path::line(Point::new(0.0, 0.0), Point::new(200.0, 100.0));
let circle = canvas::Path::circle(Point::new(100.0, 100.0), 50.0);
let rect = canvas::Path::rectangle(Point::new(10.0, 10.0), taim::Size::new(80.0, 40.0));
let rounded = canvas::Path::rounded_rectangle(
    Point::new(10.0, 10.0),
    taim::Size::new(80.0, 40.0),
    12.0, // border radius
);

// Custom path with builder
let custom = canvas::Path::new(|builder| {
    builder.move_to(Point::new(0.0, 0.0));
    builder.line_to(Point::new(100.0, 50.0));
    builder.quadratic_curve_to(
        Point::new(150.0, 0.0),   // control point
        Point::new(200.0, 50.0),  // end point
    );
    builder.close();
});
```

### Fill and Stroke

```rust
// Fill a path with a solid color
frame.fill(&path, Color::from_rgb(0.8, 0.2, 0.4));

// Fill with a gradient
let gradient = canvas::Gradient::linear(
    Point::new(0.0, 0.0),
    Point::new(200.0, 200.0),
)
.add_stop(0.0, Color::from_rgb(1.0, 0.0, 0.0))
.add_stop(1.0, Color::from_rgb(0.0, 0.0, 1.0));

frame.fill(&path, gradient);

// Stroke (outline) a path
frame.stroke(
    &path,
    canvas::Stroke {
        style: canvas::Style::Solid(Color::WHITE),
        width: 2.0,
        line_cap: canvas::LineCap::Round,
        line_join: canvas::LineJoin::Round,
        line_dash: canvas::LineDash::default(),
    },
);
```

### Text

```rust
frame.fill_text(canvas::Text {
    content: String::from("Hello, Canvas!"),
    position: Point::new(50.0, 50.0),
    size: taim::Pixels(24.0),
    color: Color::WHITE,
    font: taim::Font::MONOSPACE,
    horizontal_alignment: taim::alignment::Horizontal::Left,
    vertical_alignment: taim::alignment::Vertical::Top,
    ..Default::default()
});
```

### Transforms

Apply transformations to everything drawn after the call:

```rust
// Translate (move the origin)
frame.translate(taim::Vector::new(100.0, 50.0));

// Rotate around the current origin
frame.rotate(std::f32::consts::PI / 4.0); // 45 degrees

// Scale
frame.scale(2.0); // 2x zoom

// Scale non-uniformly
frame.scale_nonuniform(2.0, 0.5); // 2x horizontal, 0.5x vertical
```

> **Tip**: Use `frame.with_save(|frame| { ... })` to apply transforms within a scope without affecting subsequent drawing calls.

```rust
frame.with_save(|frame| {
    frame.translate(taim::Vector::new(100.0, 100.0));
    frame.rotate(angle);
    frame.fill(&shape, Color::RED); // drawn rotated at (100, 100)
});

frame.fill(&other_shape, Color::BLUE); // drawn without rotation
```

---

## Interactive Canvas

To make the canvas respond to user input, implement the `update` method:

```rust
struct DrawingApp {
    points: Vec<Point>,
}

#[derive(Debug, Clone)]
enum DrawMessage {
    AddPoint(Point),
    Clear,
}

impl canvas::Program<DrawMessage> for DrawingApp {
    type State = Option<Point>; // tracks cursor position

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<DrawMessage>> {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position) = cursor.position_in(bounds) {
                    Some(canvas::Action::publish(DrawMessage::AddPoint(position)))
                } else {
                    None
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                *state = cursor.position_in(bounds);
                Some(canvas::Action::request_redraw()) // redraw to show hover
            }
            _ => None,
        }
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Draw all recorded points
        for point in &self.points {
            let dot = canvas::Path::circle(*point, 4.0);
            frame.fill(&dot, Color::from_rgb(0.0, 0.8, 0.4));
        }

        // Draw hover indicator
        if let Some(pos) = state {
            let hover = canvas::Path::circle(*pos, 6.0);
            frame.stroke(
                &hover,
                canvas::Stroke::default()
                    .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.5))
                    .with_width(1.0),
            );
        }

        vec![frame.into_geometry()]
    }
}
```

---

## Geometry Caching

Drawing complex scenes every frame can be expensive. Use `canvas::Cache` to memoize geometry and only redraw when your data changes:

```rust
struct ChartWidget {
    data: Vec<f32>,
    cache: canvas::Cache,
}

impl ChartWidget {
    fn new(data: Vec<f32>) -> Self {
        Self {
            data,
            cache: canvas::Cache::new(),
        }
    }

    /// Call this when the data changes
    fn update_data(&mut self, new_data: Vec<f32>) {
        self.data = new_data;
        self.cache.clear(); // invalidate the cache
    }
}

impl<Message> canvas::Program<Message> for ChartWidget {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        // The closure only runs when the cache is cleared
        let chart = self.cache.draw(renderer, bounds.size(), |frame| {
            // Expensive drawing operations here
            for (i, value) in self.data.iter().enumerate() {
                let x = i as f32 * (bounds.width / self.data.len() as f32);
                let height = value * bounds.height;
                let bar = canvas::Path::rectangle(
                    Point::new(x, bounds.height - height),
                    taim::Size::new(bounds.width / self.data.len() as f32 - 2.0, height),
                );
                frame.fill(&bar, Color::from_rgb(0.3, 0.6, 1.0));
            }
        });

        vec![chart]
    }
}
```

> **Performance tip**: For canvases with both static and dynamic elements, use multiple caches — one for the background grid (rarely changes) and one for the data overlay (changes on every update).

---

## Next Steps

- Combine Canvas with [Animations](animations.md) for animated charts and visualizations
- Use [Gestures](gestures.md) alongside canvas for pan/zoom interactions
- See the [Widget Reference](widgets.md) for the full Canvas API
