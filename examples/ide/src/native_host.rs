//! IDE-local interoperation with the retained widget tree. The application
//! itself runs on TAIM's native shell; no Program/AppRunner host is restored.
use super::{Message, ShortcutContext, Studio, Task, shortcut_message};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;
use taim::view::{
    AnyView, Constraints, DescribeCx, EventCx, Handled, MeasureCx, PaintCx, Scope, Sender, Sizing,
    State as Signal, TaskScope, View,
};
use taim_core::{
    Event, Point, Rectangle, Size, Theme, clipboard, event, input_method, keyboard, mouse,
    renderer, semantics, shell, time::Instant, widget, window,
};
use taim_futures::futures::StreamExt as _;
use taim_runtime::{
    Action, UserInterface,
    user_interface::{self, Cache},
};

type Renderer = taim::Renderer;
type Ui<'a> = UserInterface<'a, Message, Theme, Renderer>;

pub(super) fn focus(id: impl Into<widget::Id>) -> Task<Message> {
    taim_runtime::task::widget(widget::operation::focusable::focus(id.into()))
}

pub(super) fn run(
    (studio, startup): (Studio, Task<Message>),
    backend: taim::Backend,
) -> taim::Result {
    taim::app("Virgo IDE", move |cx| build(cx, studio, startup))
        .settings(taim::Settings {
            backend,
            ..taim::Settings::default()
        })
        .run()
}

fn submit(task: Task<Message>, tasks: &TaskScope, sender: Sender<Action<Message>>) {
    let Some(mut stream) = taim_runtime::task::into_stream(task) else {
        return;
    };
    let _ = tasks.spawn(async move {
        while let Some(action) = stream.next().await {
            let mut pending = action;
            loop {
                match sender.send(pending) {
                    Ok(()) => break,
                    Err(std::sync::mpsc::TrySendError::Disconnected(_)) => return,
                    Err(std::sync::mpsc::TrySendError::Full(action)) => {
                        pending = action;
                        smol::Timer::after(Duration::from_millis(1)).await;
                    }
                }
            }
        }
    });
}

struct Session {
    studio: RefCell<Studio>,
    theme: Signal<Theme>,
    tasks: TaskScope,
    sender: RefCell<Option<Sender<Action<Message>>>>,
    operations: RefCell<Vec<Box<dyn widget::Operation>>>,
    events: RefCell<Vec<Event>>,
}

impl Session {
    fn dispatch(&self, message: Message) {
        let (task, theme) = {
            let mut studio = self.studio.borrow_mut();
            let task = studio.update(message);
            (task, studio.theme())
        };
        let _ = self.theme.replace(theme);
        submit(
            task,
            &self.tasks,
            self.sender.borrow().as_ref().unwrap().clone(),
        );
    }

    fn action(&self, action: Action<Message>) {
        match action {
            Action::Output(message) => self.dispatch(message),
            Action::Widget(operation) => self.operations.borrow_mut().push(operation),
            Action::Event { event, .. } => self.events.borrow_mut().push(event),
            Action::Tick | Action::Reload => {}
            // The IDE's task producers are message streams and widget
            // operations. Keep unexpected effects visible, never silently
            // claim that an unsupported OS operation completed.
            _ => self
                .studio
                .borrow_mut()
                .terminal
                .log(super::LogLevel::Error, "Unsupported IDE task effect"),
        }
    }
}

fn build(cx: &mut Scope, studio: Studio, startup: Task<Message>) -> AnyView {
    let polling = studio.workspace_root.is_some();
    let theme = cx.signal(studio.theme());
    cx.theme(theme);
    let session = Rc::new(Session {
        studio: RefCell::new(studio),
        theme,
        tasks: cx.tasks(),
        sender: RefCell::new(None),
        operations: RefCell::new(Vec::new()),
        events: RefCell::new(Vec::new()),
    });
    let sender = cx.channel({
        let session = session.clone();
        move |action| session.action(action)
    });
    *session.sender.borrow_mut() = Some(sender.clone());
    submit(startup, &session.tasks, sender.clone());
    if polling {
        let sender = sender.clone();
        cx.spawn(async move {
            loop {
                smol::Timer::after(Duration::from_secs(2)).await;
                match sender.send(Action::Output(Message::WorkspacePoll)) {
                    Ok(()) | Err(std::sync::mpsc::TrySendError::Full(_)) => {}
                    Err(std::sync::mpsc::TrySendError::Disconnected(_)) => break,
                }
            }
        });
    }
    let waker = shell::Waker::new(move || {
        let _ = sender.send(Action::Tick);
    });
    Workbench {
        session,
        cache: RefCell::new(Some(Cache::new())),
        bounds: Cell::new(Size::ZERO),
        pointer: Cell::new(None),
        interaction: Cell::new(mouse::Interaction::None),
        input: RefCell::new(input_method::InputMethod::Disabled),
        semantics: RefCell::new(Vec::new()),
        deadline: Cell::new(None),
        animating: Cell::new(false),
        waker,
        id: widget::Id::unique(),
    }
    .into()
}

struct Workbench {
    session: Rc<Session>,
    cache: RefCell<Option<Cache>>,
    bounds: Cell<Size>,
    pointer: Cell<Option<Point>>,
    interaction: Cell<mouse::Interaction>,
    input: RefCell<input_method::InputMethod>,
    semantics: RefCell<Vec<semantics::Node>>,
    deadline: Cell<Option<Instant>>,
    animating: Cell<bool>,
    waker: shell::Waker,
    id: widget::Id,
}

impl Workbench {
    fn with_ui<T>(&self, renderer: &Renderer, run: impl FnOnce(&mut Ui<'_>) -> T) -> T {
        let studio = self.session.studio.borrow();
        let cache = self.cache.borrow_mut().take().unwrap_or_default();
        let mut ui = UserInterface::build_ref(studio.view(), self.bounds.get(), cache, renderer);
        for mut operation in self.session.operations.borrow_mut().drain(..) {
            ui.operate(renderer, operation.as_mut());
        }
        let result = run(&mut ui);
        *self.cache.borrow_mut() = Some(ui.into_cache());
        result
    }

    fn feedback(&self, state: user_interface::State) -> clipboard::Clipboard {
        match state {
            user_interface::State::Updated {
                mouse_interaction,
                redraw_request,
                input_method,
                clipboard,
                is_animating,
                ..
            } => {
                self.interaction.set(mouse_interaction);
                *self.input.borrow_mut() = input_method;
                self.animating.set(is_animating);
                self.deadline.set(match redraw_request {
                    window::RedrawRequest::NextFrame => Some(Instant::now()),
                    window::RedrawRequest::At(deadline) => Some(deadline),
                    window::RedrawRequest::Wait => None,
                });
                clipboard
            }
            user_interface::State::Outdated => {
                self.deadline.set(Some(Instant::now()));
                clipboard::Clipboard::new()
            }
        }
    }

    fn shortcut(&self, event: &Event) -> Option<Message> {
        let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
            return None;
        };
        let studio = self.session.studio.borrow();
        shortcut_message(
            key.as_ref(),
            *modifiers,
            ShortcutContext {
                terminal_focused: studio.terminal_raw_focus
                    && studio.show_terminal
                    && studio.terminal.active_tab == super::PanelTab::Terminal,
                palette_open: studio.palette.is_open,
                palette_selection: studio.palette.selected_item().map(|item| item.id),
                find_open: studio.docs.find_open,
                autocomplete_open: studio.autocomplete.is_open,
                autocomplete_selection: studio.autocomplete.selected_index,
            },
        )
    }
}

impl View for Workbench {
    fn sizing(&self) -> Sizing {
        Sizing::FILL
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let size = constraints.clamp(constraints.max);
        self.bounds.set(size);
        self.with_ui(cx.renderer(), |ui| {
            *self.semantics.borrow_mut() = ui.semantics(cx.renderer());
        });
        size
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let renderer = cx.renderer();
        let studio = self.session.studio.borrow();
        let cache = self.cache.borrow_mut().take().unwrap_or_default();
        let mut ui = UserInterface::build_ref(studio.view(), self.bounds.get(), cache, &*renderer);
        let mut messages = Vec::new();
        let (state, _) = ui.update_ref(
            &window::Headless,
            &self.waker,
            &[Event::Window(
                window::Event::RedrawRequested(Instant::now()),
            )],
            self.pointer
                .get()
                .map_or(mouse::Cursor::Unavailable, mouse::Cursor::Available),
            &*renderer,
            &mut messages,
        );
        let _ = self.feedback(state);
        let theme = studio.theme();
        use taim_core::theme::Base as _;
        ui.draw(
            renderer,
            &theme,
            &renderer::Style {
                text_color: theme.base().text_color,
            },
            self.pointer
                .get()
                .map_or(mouse::Cursor::Unavailable, mouse::Cursor::Available),
        );
        *self.semantics.borrow_mut() = ui.semantics(&*renderer);
        *self.cache.borrow_mut() = Some(ui.into_cache());
        drop(studio);
        for message in messages {
            self.session.dispatch(message);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if matches!(event, Event::Window(window::Event::Focused))
            || matches!(event, Event::Mouse(mouse::Event::ButtonPressed(_))) && cx.is_hovered()
        {
            cx.request_focus();
        }
        self.pointer.set(cx.pointer());
        let (state, statuses, messages) = {
            let Some(renderer) = cx.renderer() else {
                return Handled::No;
            };
            self.with_ui(renderer, |ui| {
                let mut messages = Vec::new();
                let mut events: Vec<_> = self.session.events.borrow_mut().drain(..).collect();
                events.push(event.clone());
                let (state, statuses) = ui.update_ref(
                    &window::Headless,
                    &self.waker,
                    &events,
                    self.pointer
                        .get()
                        .map_or(mouse::Cursor::Unavailable, mouse::Cursor::Available),
                    renderer,
                    &mut messages,
                );
                (state, statuses, messages)
            })
        };
        let clipboard = self.feedback(state);
        if let Some(clipboard::Content::Text(text)) = clipboard.write {
            cx.copy(text);
        }
        if clipboard.reads.contains(&clipboard::Kind::Text) {
            cx.paste();
        }
        for message in messages {
            self.session.dispatch(message);
        }
        let captured = statuses.last() == Some(&event::Status::Captured);
        let shortcut = if captured { None } else { self.shortcut(event) };
        let handled = captured || shortcut.is_some();
        if let Some(message) = shortcut {
            self.session.dispatch(message);
        }
        if handled || self.deadline.get().is_some() {
            cx.request_paint();
        }
        Handled::from(handled)
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        let mut node = semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::Group)
            .with_name("Virgo IDE");
        node.children = self.semantics.borrow().clone();
        Some(node)
    }

    fn focusable(&self) -> bool {
        true
    }
    fn cursor(&self) -> Option<mouse::Interaction> {
        Some(self.interaction.get())
    }
    fn input_method(&self, _: Rectangle) -> input_method::InputMethod {
        self.input.borrow().clone()
    }
    fn is_animating(&self, _: Instant) -> bool {
        self.animating.get()
    }
    fn redraw_at(&self) -> Option<Instant> {
        self.deadline.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taim::view::Root;
    use taim_core::{Renderer as _, renderer::Headless as _};

    fn editor(node: &semantics::Node) -> Option<&semantics::Node> {
        if node.role == semantics::Role::TextField && node.bounds.height > 100.0 {
            return Some(node);
        }
        node.children.iter().find_map(editor)
    }

    fn frame(root: &mut Root, renderer: &mut Renderer) -> semantics::Node {
        let size = Size::new(1200.0, 800.0);
        let _ = root.take_changes();
        root.layout(renderer, size);
        renderer.reset(Rectangle::with_size(size));
        root.paint(renderer, taim::Color::WHITE, None, Instant::now());
        root.describe(size)
    }

    #[test]
    fn native_host_renders_editor_and_delivers_keyboard_edits_to_the_existing_document() {
        let (studio, startup) = Studio::new();
        let mut root = Root::new(move |cx| build(cx, studio, startup));
        let mut renderer = taim_futures::futures::executor::block_on(
            <Renderer as renderer::Headless>::new(renderer::Settings::default(), Some("tiny-skia")),
        )
        .expect("CPU renderer");
        let tree = frame(&mut root, &mut renderer);
        let node = editor(&tree).expect("the native host publishes the editor");
        let before = node.value().cloned();
        let at = Point::new(node.bounds.x + 30.0, node.bounds.y + 20.0);
        for event in [
            Event::Window(window::Event::Focused),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ] {
            let _ = root.handle_with_renderer(&renderer, &event, Some(at), Instant::now());
        }
        let _ = frame(&mut root, &mut renderer);
        assert!(
            root.input_method().is_enabled(),
            "the focused editor negotiates OS text input"
        );
        let key = keyboard::Key::Character("x".into());
        let event = Event::Keyboard(keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: Some("x".into()),
            repeat: false,
        });
        assert!(
            root.handle_with_renderer(&renderer, &event, Some(at), Instant::now())
                .captured
        );
        let tree = frame(&mut root, &mut renderer);
        let after = editor(&tree).unwrap().value().cloned();
        assert_ne!(before, after, "typing must reach the real Studio document");
        let pixels = renderer.screenshot(Size::new(1200, 800), 1.0, taim::Color::BLACK);
        assert!(
            pixels
                .chunks_exact(4)
                .any(|p| p[0] > 100 || p[1] > 100 || p[2] > 100)
        );
        assert!(
            !root.is_animating(Instant::now()),
            "caret blinking must not run a continuous render loop"
        );
        assert!(
            root.redraw_at().is_some(),
            "the editor schedules its next caret blink"
        );

        // A shortcut starts a widget-focus Task. Verify its asynchronous
        // operation reaches the same retained UI through the native mailbox.
        let key = keyboard::Key::Named(keyboard::key::Named::F1);
        let event = Event::Keyboard(keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        });
        assert!(
            root.handle_with_renderer(&renderer, &event, Some(at), Instant::now())
                .captured
        );
        fn focused_input(node: &semantics::Node) -> bool {
            node.role == semantics::Role::TextField
                && node.bounds.height < 100.0
                && node.state.contains(semantics::State::FOCUSED)
                || node.children.iter().any(focused_input)
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if focused_input(&frame(&mut root, &mut renderer)) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "command-palette focus Task did not execute"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
