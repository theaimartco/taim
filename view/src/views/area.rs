//! A view that reports what the pointer does over its child.
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Constraints, Sizing};
use crate::{AnyView, Child, Event, Handled, Point, Size, View, mouse, semantics};

type Action = Box<dyn Fn()>;

/// Wraps `child` and calls handlers for the pointer over it. Events the
/// child itself used (a button inside, say) never reach the area.
pub fn mouse_area(child: impl Into<AnyView>) -> MouseArea {
    MouseArea {
        child: Child::new(child.into()),
        press: None,
        release: None,
        right_press: None,
        middle_press: None,
        double_press: None,
        enter: None,
        exit: None,
        moved: None,
        hovered: false,
        last_press: None,
        interaction: None,
    }
}

/// See [`mouse_area`].
pub struct MouseArea {
    child: Child,
    press: Option<Action>,
    release: Option<Action>,
    right_press: Option<Action>,
    middle_press: Option<Action>,
    double_press: Option<Action>,
    enter: Option<Action>,
    exit: Option<Action>,
    moved: Option<Box<dyn Fn(Point)>>,
    hovered: bool,
    last_press: Option<crate::Instant>,
    interaction: Option<mouse::Interaction>,
}

impl MouseArea {
    /// The left button went down over the area.
    pub fn on_press(mut self, action: impl Fn() + 'static) -> Self {
        self.press = Some(Box::new(action));
        self
    }

    /// The left button came up over the area.
    pub fn on_release(mut self, action: impl Fn() + 'static) -> Self {
        self.release = Some(Box::new(action));
        self
    }

    /// The right button went down over the area.
    pub fn on_right_press(mut self, action: impl Fn() + 'static) -> Self {
        self.right_press = Some(Box::new(action));
        self
    }

    /// The middle button went down over the area.
    pub fn on_middle_press(mut self, action: impl Fn() + 'static) -> Self {
        self.middle_press = Some(Box::new(action));
        self
    }

    /// Two left presses in quick succession.
    pub fn on_double_click(mut self, action: impl Fn() + 'static) -> Self {
        self.double_press = Some(Box::new(action));
        self
    }

    /// The pointer came over the area.
    pub fn on_enter(mut self, action: impl Fn() + 'static) -> Self {
        self.enter = Some(Box::new(action));
        self
    }

    /// The pointer left the area.
    pub fn on_exit(mut self, action: impl Fn() + 'static) -> Self {
        self.exit = Some(Box::new(action));
        self
    }

    /// The pointer moved over the area; `at` is relative to its corner.
    pub fn on_move(mut self, action: impl Fn(Point) + 'static) -> Self {
        self.moved = Some(Box::new(action));
        self
    }

    /// The cursor shown over the area.
    pub fn cursor(mut self, interaction: mouse::Interaction) -> Self {
        self.interaction = Some(interaction);
        self
    }
}

impl View for MouseArea {
    fn sizing(&self) -> Sizing {
        self.child.view().sizing()
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let size = self.child.measure(cx, constraints);
        self.child.place(Point::ORIGIN);
        size
    }

    fn paint(&self, _cx: &mut PaintCx<'_>) {}

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let Event::Mouse(event) = event else {
            return Handled::No;
        };
        let hovered = cx.is_hovered();

        if hovered != self.hovered {
            self.hovered = hovered;
            let transition = if hovered { &self.enter } else { &self.exit };
            if let Some(action) = transition {
                action();
            }
        }

        let run = |action: &Option<Action>| match action {
            Some(action) => {
                action();
                Handled::Yes
            }
            None => Handled::No,
        };

        match event {
            mouse::Event::ButtonPressed(mouse::Button::Left) if hovered => {
                let now = cx.now();
                let double = self
                    .last_press
                    .is_some_and(|before| now.saturating_duration_since(before).as_millis() < 400);
                self.last_press = if double { None } else { Some(now) };

                let doubled = if double {
                    run(&self.double_press)
                } else {
                    Handled::No
                };
                let single = run(&self.press);
                Handled::from(doubled == Handled::Yes || single == Handled::Yes)
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) if hovered => run(&self.release),
            mouse::Event::ButtonPressed(mouse::Button::Right) if hovered => run(&self.right_press),
            mouse::Event::ButtonPressed(mouse::Button::Middle) if hovered => {
                run(&self.middle_press)
            }
            mouse::Event::CursorMoved { position } if hovered => {
                if let Some(action) = &self.moved {
                    let bounds = cx.bounds();
                    action(Point::new(position.x - bounds.x, position.y - bounds.y));
                }
                Handled::No
            }
            _ => Handled::No,
        }
    }

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    fn cursor(&self) -> Option<mouse::Interaction> {
        self.interaction
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.child);
    }
}
