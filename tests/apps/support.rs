//! Driving a view tree headlessly, as a finger or keyboard would.
#![allow(dead_code)]

use taim::view::Root;
use taim_accessibility::tree::Tree;
use taim_core::renderer::Headless as _;
use taim_core::semantics::Role;
use taim_core::time::{Duration, Instant};
use taim_core::{Color, Event, Point, Size, keyboard, mouse};

pub const WINDOW: Size = Size {
    width: 360.0,
    height: 640.0,
};

pub struct App {
    pub root: Root,
    pub renderer: taim::Renderer,
    pub now: Instant,
}

impl App {
    pub fn new(root: Root) -> Self {
        let renderer = taim_futures::futures::executor::block_on(taim::Renderer::new(
            taim_core::renderer::Settings::default(),
            Some("tiny-skia"),
        ))
        .expect("the CPU renderer starts headless");

        let mut app = Self {
            root,
            renderer,
            now: Instant::now(),
        };
        app.frame();
        app
    }

    /// Lays out and paints one frame at the current time.
    pub fn frame(&mut self) {
        use taim_core::Renderer as _;

        let _ = self.root.take_changes();
        self.root.layout(&self.renderer, WINDOW);
        self.renderer.reset(taim_core::Rectangle::with_size(WINDOW));
        self.root
            .paint(&mut self.renderer, Color::BLACK, None, self.now);
    }

    /// Lets `time` pass, painting frames along the way.
    pub fn wait(&mut self, time: Duration) {
        let end = self.now + time;
        while self.now < end {
            self.now += Duration::from_millis(16);
            self.frame();
        }
    }

    pub fn semantics(&mut self) -> Tree {
        Tree::from_forest(&[self.root.describe(WINDOW)])
    }

    /// Whether anything named `name` is on screen.
    pub fn shows(&mut self, name: &str) -> bool {
        self.semantics()
            .in_order()
            .into_iter()
            .any(|entry| entry.name.as_deref() == Some(name))
    }

    pub fn find(&mut self, role: Role, name: &str) -> Option<Point> {
        self.semantics()
            .in_order()
            .into_iter()
            .find(|entry| entry.role == role && entry.name.as_deref() == Some(name))
            .map(|entry| entry.bounds.center())
    }

    /// A finger tap on the button named `name`.
    pub fn tap(&mut self, name: &str) {
        let at = self
            .find(Role::Button, name)
            .unwrap_or_else(|| panic!("a button named {name:?}"));
        self.tap_at(at);
    }

    pub fn tap_at(&mut self, at: Point) {
        for event in [
            Event::Mouse(mouse::Event::CursorMoved { position: at }),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ] {
            let _ = self.root.handle_input(&event, Some(at), self.now, true);
        }
        let _ = self.root.handle_input(
            &Event::Mouse(mouse::Event::CursorLeft),
            None,
            self.now,
            true,
        );
        self.frame();
    }

    /// A mouse wheel turned by `pixels` over `at` (negative scrolls down).
    pub fn wheel(&mut self, at: Point, pixels: f32) {
        let _ = self.root.handle(
            &Event::Mouse(mouse::Event::CursorMoved { position: at }),
            Some(at),
            self.now,
        );
        let _ = self.root.handle(
            &Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: 0.0, y: pixels },
            }),
            Some(at),
            self.now,
        );
        self.frame();
    }

    /// A finger dragged sideways from `from` by `distance` pixels over
    /// `millis`, in steps a frame apart, then lifted where it stopped.
    pub fn swipe(&mut self, from: Point, distance: f32, millis: u64) {
        let steps = (millis / 16).max(1);
        let mut at = from;
        let send = |app: &mut Self, event: Event, at: Point| {
            let _ = app.root.handle_input(&event, Some(at), app.now, true);
        };
        send(
            self,
            Event::Mouse(mouse::Event::CursorMoved { position: at }),
            at,
        );
        send(
            self,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            at,
        );
        for _ in 0..steps {
            at.x += distance / steps as f32;
            self.now += Duration::from_millis(16);
            send(
                self,
                Event::Mouse(mouse::Event::CursorMoved { position: at }),
                at,
            );
            self.frame();
        }
        send(
            self,
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            at,
        );
        let _ = self.root.handle_input(
            &Event::Mouse(mouse::Event::CursorLeft),
            None,
            self.now,
            true,
        );
        self.frame();
    }

    /// A quick finger flick from `from` up by `distance` pixels, released
    /// while moving.
    pub fn flick(&mut self, from: Point, distance: f32) {
        let mut at = from;
        let send = |app: &mut Self, event: Event, at: Point| {
            let _ = app.root.handle_input(&event, Some(at), app.now, true);
        };
        send(
            self,
            Event::Mouse(mouse::Event::CursorMoved { position: at }),
            at,
        );
        send(
            self,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            at,
        );
        for _ in 0..5 {
            at.y -= distance / 5.0;
            self.now += Duration::from_millis(10);
            send(
                self,
                Event::Mouse(mouse::Event::CursorMoved { position: at }),
                at,
            );
        }
        send(
            self,
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            at,
        );
        let _ = self.root.handle_input(
            &Event::Mouse(mouse::Event::CursorLeft),
            None,
            self.now,
            true,
        );
    }

    /// Names of everything on screen, in order.
    pub fn names(&mut self) -> Vec<String> {
        self.semantics()
            .in_order()
            .into_iter()
            .filter_map(|entry| entry.name.clone().map(|name| name.to_string()))
            .collect()
    }

    /// The Android back button. Returns whether the app used it.
    pub fn back(&mut self) -> bool {
        let named = keyboard::key::Named::BrowserBack;
        let event = Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(named),
            modified_key: keyboard::Key::Named(named),
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        });
        let captured = self.root.handle(&event, None, self.now).captured;
        self.frame();
        captured
    }
}
