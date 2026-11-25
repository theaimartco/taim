# Custom drawing

Taim has no separate canvas widget. To draw something the built-in views
don't cover, such as a chart, a gauge or a game board, you implement the
`View` trait. A view does four things:

| Method | Job |
|---|---|
| `sizing` | Says whether it hugs its content, has a fixed size, or fills space |
| `measure` | Picks its size within the parent's `Constraints` |
| `paint` | Draws itself through `PaintCx` |
| `describe` | Tells screen readers what it is, or returns `None` if decorative |

`describe` has no default on purpose: every view decides whether
assistive technology can perceive it.

## A bar chart

```rust
use taim::prelude::*;
use taim::view::{
    Axis, Constraints, DescribeCx, IntoProp, MeasureCx, PaintCx, Prop, Sizing, semantics,
};
use taim::{Border, Rectangle, Size, border, widget};

pub struct BarChart {
    values: Prop<Vec<f32>>,
    label: String,
    id: widget::Id,
}

pub fn bar_chart(label: impl Into<String>, values: impl IntoProp<Vec<f32>>) -> BarChart {
    BarChart {
        values: values.into_prop(),
        label: label.into(),
        id: widget::Id::unique(),
    }
}

impl View for BarChart {
    fn sizing(&self) -> Sizing {
        Sizing {
            width: Axis::Fill(1),
            height: Axis::Fixed(160.0),
        }
    }

    fn measure(&mut self, _cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(constraints.max.width.min(1.0e6), 160.0))
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let palette = cx.theme().palette();
        let (track, bar) = (palette.background.weak.color, palette.primary.base.color);

        cx.fill(bounds, track, border::rounded(8.0));

        let values = self.values.get();
        let highest = values.iter().copied().fold(1.0_f32, f32::max);
        let gap = 6.0;
        let width = (bounds.width - gap * (values.len() as f32 + 1.0)) / values.len().max(1) as f32;

        for (index, value) in values.iter().enumerate() {
            let height = (bounds.height - gap * 2.0) * (value / highest);
            let area = Rectangle {
                x: bounds.x + gap + index as f32 * (width + gap),
                y: bounds.y + bounds.height - gap - height,
                width,
                height,
            };
            cx.fill(area, bar, Border::default().rounded(4.0));
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        let values = self.values.get();
        let summary = values
            .iter()
            .map(|value| format!("{value}"))
            .collect::<Vec<_>>()
            .join(", ");

        Some(
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::Image)
                .with_name(self.label.clone())
                .with_description(summary),
        )
    }
}
```

Use it like any built-in view. Because `values` is a prop, the chart
repaints whenever the state it reads changes:

```rust,ignore
// With `bar_chart` from above in scope.
fn sales(cx: &mut Scope) -> impl View {
    let week = cx.signal(vec![3.0, 5.0, 2.0, 8.0, 6.0, 4.0, 7.0]);

    column(views![
        bar_chart("Sales this week", move || week.get()),
        button("Double Monday").on_press(move || week.update(|days| days[0] *= 2.0)),
    ])
    .gap(12.0)
    .padding(24.0)
}
```

## Drawing tools

`PaintCx` draws in window coordinates; `cx.bounds()` is where layout put
the view.

| Method | Draws |
|---|---|
| `fill(area, background, border)` | A rectangle, optionally rounded and outlined. `background` is a `Color` or `Gradient` |
| `fill_shadowed(area, background, border, shadow)` | The same with a drop shadow |
| `fill_material(…)`, `fill_liquid(…)` | Glass surfaces (paint them in their own layer) |
| `renderer()` | The renderer itself, for text and images |

Read colours from `cx.theme().palette()` rather than hard-coding them, so
the view follows light, dark and high-contrast themes.

## Input and animation

Override `event` to react to pointer and keyboard input, and return
`Handled::Yes` to stop the event there. Override `is_animating` to ask for
another frame while something moves, and `cursor` to change the pointer.
The built-in views in `view/src/views/` are complete, tested examples of
each.

## Accessibility

`semantics::Node::new(id, bounds, role)` needs a stable `widget::Id`.
Create one with `widget::Id::unique()` when the view is built and keep it
for the view's life, so screen readers can track the node across frames.
Add a name with `.with_name`, extra detail with `.with_description`, and a
current value with `.with_value`. A purely decorative view returns `None`.
