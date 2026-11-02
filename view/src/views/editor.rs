//! Multi-line text editing, for notes and for code.
use super::look;
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Constraints, Sizing};
use crate::state::{Scope, State};
use crate::{Border, Color, Event, Handled, Padding, Point, Rectangle, Renderer, Size, View};
use crate::{keyboard, mouse, semantics};

use taim_core::Renderer as _;
use taim_core::text::editor::{Action, Cursor, Edit, Motion, Selection};
use taim_core::text::highlighter::{self, Highlighter, PlainText};
use taim_core::text::{self, Editor as _, LineHeight, Renderer as _, Wrapping};
use taim_core::{Font, Pixels, Theme, clipboard, input_method, widget};

use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex, PoisonError};

type Engine = <Renderer as text::Renderer>::Editor;

/// The text an [`text_editor`] edits.
///
/// A buffer is a copyable handle, like [`State`]: pass it to two views to
/// show the same text twice, or capture it in a closure to read it. Anything that reads the buffer
/// through its methods (a status bar showing the caret's line, say) is
/// repainted when the text, the caret or the selection changes.
#[derive(Clone, Copy)]
pub struct Buffer {
    engine: State<Arc<Mutex<Engine>>>,
    revision: State<u64>,
}

impl Buffer {
    /// A buffer holding `text`, caret at the start.
    pub fn new(cx: &mut Scope, text: &str) -> Self {
        Self {
            engine: cx.signal(Arc::new(Mutex::new(Engine::with_text(text)))),
            revision: cx.signal(0),
        }
    }

    /// Replaces the whole text. The caret goes back to the start.
    pub fn set_text(&self, text: &str) {
        self.with(|engine| *engine = Engine::with_text(text));
        self.touch();
    }

    /// The whole text, lines joined with the buffer's own line ending.
    pub fn text(&self) -> String {
        let _ = self.revision.get();
        self.with(|engine| {
            let count = engine.line_count();
            let mut joined = String::new();

            for index in 0..count {
                let Some(line) = engine.line(index) else {
                    break;
                };
                joined.push_str(&line.text);

                if index + 1 < count {
                    let ending = match line.ending {
                        text::editor::LineEnding::None => text::editor::LineEnding::default(),
                        ending => ending,
                    };
                    joined.push_str(ending.as_str());
                }
            }

            joined
        })
    }

    /// Whether the buffer holds no text.
    pub fn is_empty(&self) -> bool {
        let _ = self.revision.get();
        self.with(|engine| engine.is_empty())
    }

    /// How many lines the text has (an empty buffer has one).
    pub fn line_count(&self) -> usize {
        let _ = self.revision.get();
        self.with(|engine| engine.line_count())
    }

    /// Line `index` without its line ending.
    pub fn line(&self, index: usize) -> Option<String> {
        let _ = self.revision.get();
        self.with(|engine| engine.line(index).map(|line| line.text.into_owned()))
    }

    /// The caret, and the other end of the selection if one is made.
    pub fn cursor(&self) -> Cursor {
        let _ = self.revision.get();
        self.with(|engine| engine.cursor())
    }

    /// The selected text, if any is selected.
    pub fn selected_text(&self) -> Option<String> {
        let _ = self.revision.get();
        self.with(|engine| engine.copy())
    }

    /// The first visible line and how many pixels of it are scrolled off.
    pub fn scroll_position(&self) -> (usize, f32) {
        let _ = self.revision.get();
        self.with(|engine| engine.scroll_position())
    }

    /// Edits, moves the caret, selects or scrolls.
    pub fn perform(&self, action: Action) {
        self.with(|engine| engine.perform(action));
        self.touch();
    }

    /// Puts the caret (and selection) at `cursor`.
    pub fn move_to(&self, cursor: Cursor) {
        self.with(|engine| engine.move_to(cursor));
        self.touch();
    }

    /// Runs `work` on the engine. Nothing holds the lock across a call
    /// into user code, so a poisoned lock only means an earlier panic.
    fn with<R>(&self, work: impl FnOnce(&mut Engine) -> R) -> R {
        let shared = self.engine.get();
        let mut engine = shared.lock().unwrap_or_else(PoisonError::into_inner);
        work(&mut engine)
    }

    fn touch(&self) {
        self.revision.update(|revision| *revision += 1);
    }
}

impl std::fmt::Debug for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buffer").finish_non_exhaustive()
    }
}

/// An editable, scrollable block of text showing `buffer`.
pub fn text_editor(buffer: &Buffer) -> TextEditor {
    TextEditor {
        buffer: *buffer,
        font: None,
        size: None,
        line_height: LineHeight::default(),
        wrapping: Wrapping::None,
        padding: Padding::new(8.0),
        placeholder: None,
        colors: Box::new(Plain),
        on_action: None,
        styled_for: RefCell::new(None),
        dragging: false,
        awaiting_paste: false,
        clicks: Clicks::default(),
        area: Cell::new(Rectangle::default()),
        id: widget::Id::unique(),
    }
}

/// See [`text_editor`].
pub struct TextEditor {
    buffer: Buffer,
    font: Option<Font>,
    size: Option<Pixels>,
    line_height: LineHeight,
    wrapping: Wrapping,
    padding: Padding,
    placeholder: Option<String>,
    colors: Box<dyn Colors>,
    on_action: Option<Box<dyn Fn(&Action)>>,
    /// Theme the colors were last computed for.
    styled_for: RefCell<Option<String>>,
    dragging: bool,
    awaiting_paste: bool,
    clicks: Clicks,
    area: Cell<Rectangle>,
    id: widget::Id,
}

impl TextEditor {
    /// The font; the renderer's default when unset. Code wants a
    /// monospaced one.
    pub fn font(mut self, font: Font) -> Self {
        self.font = Some(font);
        self
    }

    /// Text size in logical pixels.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = Some(size.into());
        self
    }

    /// Line height, relative to the text size or absolute.
    pub fn line_height(mut self, line_height: impl Into<LineHeight>) -> Self {
        self.line_height = line_height.into();
        self
    }

    /// Space between the border and the text.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Wraps long lines at the view's width instead of scrolling sideways.
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrapping = if wrap {
            Wrapping::WordOrGlyph
        } else {
            Wrapping::None
        };
        self
    }

    /// Text shown, dimmed, while the buffer is empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Colors the text with `H`: `format` turns each highlight into a
    /// color and font for the window's theme.
    pub fn highlight<H>(
        mut self,
        settings: H::Settings,
        format: impl Fn(&H::Highlight, &Theme) -> highlighter::Format<Font> + 'static,
    ) -> Self
    where
        H: Highlighter,
    {
        self.colors = Box::new(Highlighted::<H> {
            highlighter: RefCell::new(H::new(&settings)),
            settings,
            format: Box::new(format),
        });
        self
    }

    /// Calls `observe` after every action the view applied: edits,
    /// caret moves, clicks, drags and scrolls.
    pub fn on_action(mut self, observe: impl Fn(&Action) + 'static) -> Self {
        self.on_action = Some(Box::new(observe));
        self
    }

    fn text_area(&self, bounds: Rectangle) -> Rectangle {
        bounds.shrink(self.padding)
    }

    fn apply(&mut self, cx: &mut EventCx<'_>, action: Action) {
        self.buffer.perform(action.clone());
        cx.request_paint();

        if let Some(observe) = &self.on_action {
            observe(&action);
        }
    }

    fn key(
        &mut self,
        cx: &mut EventCx<'_>,
        key: &keyboard::Key,
        modifiers: keyboard::Modifiers,
        typed: Option<&str>,
    ) -> Handled {
        use keyboard::key::Named;

        let extend = modifiers.shift();
        let widen = modifiers.command() || modifiers.alt();
        let motion = |motion: Motion| {
            let motion = if widen { motion.widen() } else { motion };
            if extend {
                Action::Select(motion)
            } else {
                Action::Move(motion)
            }
        };

        let action = match key {
            keyboard::Key::Named(Named::ArrowLeft) => motion(Motion::Left),
            keyboard::Key::Named(Named::ArrowRight) => motion(Motion::Right),
            keyboard::Key::Named(Named::ArrowUp) => motion(Motion::Up),
            keyboard::Key::Named(Named::ArrowDown) => motion(Motion::Down),
            keyboard::Key::Named(Named::Home) => motion(Motion::Home),
            keyboard::Key::Named(Named::End) => motion(Motion::End),
            keyboard::Key::Named(Named::PageUp) => motion(Motion::PageUp),
            keyboard::Key::Named(Named::PageDown) => motion(Motion::PageDown),
            keyboard::Key::Named(Named::Backspace) => Action::Edit(Edit::Backspace),
            keyboard::Key::Named(Named::Delete) => Action::Edit(Edit::Delete),
            keyboard::Key::Named(Named::Enter) => Action::Edit(Edit::Enter),
            keyboard::Key::Named(Named::Tab) if extend => Action::Edit(Edit::Unindent),
            keyboard::Key::Named(Named::Tab) => Action::Edit(Edit::Indent),
            keyboard::Key::Named(Named::Escape) => {
                cx.release_focus();
                return Handled::Yes;
            }
            keyboard::Key::Character(character) if modifiers.command() => {
                match character.as_str() {
                    "a" => Action::SelectAll,
                    "c" | "x" => {
                        if let Some(selected) = self.buffer.selected_text() {
                            cx.copy(selected);
                            if character.as_str() == "x" {
                                self.apply(cx, Action::Edit(Edit::Backspace));
                            }
                        }
                        return Handled::Yes;
                    }
                    "v" => {
                        self.awaiting_paste = true;
                        cx.paste();
                        return Handled::Yes;
                    }
                    // Shortcuts belong to whoever bound them.
                    _ => return Handled::No,
                }
            }
            _ if modifiers.command() => return Handled::No,
            _ => match typed.filter(|text| text.chars().all(|c| !c.is_control())) {
                Some(text) if !text.is_empty() => {
                    for character in text.chars() {
                        self.apply(cx, Action::Edit(Edit::Insert(character)));
                    }
                    return Handled::Yes;
                }
                _ => return Handled::No,
            },
        };

        self.apply(cx, action);
        Handled::Yes
    }
}

impl View for TextEditor {
    fn sizing(&self) -> Sizing {
        Sizing::FILL
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let renderer = cx.renderer();
        let room = constraints.with_sizing(self.sizing()).max;
        let room = Size::new(
            if room.width.is_finite() {
                room.width
            } else {
                480.0
            },
            if room.height.is_finite() {
                room.height
            } else {
                240.0
            },
        );
        let inner = Size::new(
            (room.width - self.padding.left - self.padding.right).max(0.0),
            (room.height - self.padding.top - self.padding.bottom).max(0.0),
        );

        let font = self.font.unwrap_or_else(|| renderer.default_font());
        let size = self.size.unwrap_or_else(|| renderer.default_size());
        let (line_height, wrapping, hint) =
            (self.line_height, self.wrapping, renderer.scale_factor());
        let colors = &mut self.colors;

        self.buffer.with(|engine| {
            colors.layout(
                engine,
                Layout {
                    bounds: inner,
                    font,
                    size,
                    line_height,
                    wrapping,
                    hint,
                },
            );
        });

        room
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let focused = cx.has_focus();
        let area = self.text_area(bounds);
        self.area.set(area);

        let edge = if focused {
            palette.primary.base.color
        } else {
            palette.background.strong.color
        };
        cx.fill(
            bounds,
            palette.background.base.color,
            Border::default().width(1.0).color(edge).rounded(6.0),
        );

        let font = self.font.unwrap_or_else(|| cx.renderer().default_font());
        let size = self.size.unwrap_or_else(|| cx.renderer().default_size());
        let text_color = cx.text_color();
        let theme = cx.theme().clone();
        let shared = self.buffer.engine.get();
        let mut engine = shared.lock().unwrap_or_else(PoisonError::into_inner);

        {
            let mut styled_for = self.styled_for.borrow_mut();
            let name = taim_core::theme::Base::name(&theme);
            let restart = styled_for.as_deref() != Some(name);
            if restart {
                *styled_for = Some(name.to_owned());
            }
            self.colors.color(&mut engine, font, &theme, restart);
        }

        let hint = cx.renderer().scale_factor();
        let selection = engine.selection();
        let line = self.line_height.to_absolute(size).0;

        if engine.is_empty()
            && let Some(placeholder) = &self.placeholder
        {
            use text::Renderer as _;
            cx.renderer().fill_text(
                text::Text {
                    content: placeholder.clone(),
                    bounds: area.size(),
                    size,
                    line_height: self.line_height,
                    font,
                    align_x: text::Alignment::Default,
                    align_y: taim_core::alignment::Vertical::Top,
                    shaping: text::Shaping::Advanced,
                    wrapping: self.wrapping,
                    ellipsis: text::Ellipsis::None,
                    hint_factor: hint,
                },
                area.position(),
                palette.background.strongest.color,
                area,
            );
        }

        let origin = area.position() - Point::ORIGIN;
        cx.renderer().with_layer(area, |renderer| {
            if !engine.is_empty() {
                renderer.fill_editor(&engine, area.position(), text_color, area);
            }

            match &selection {
                Selection::Caret(at) if focused => {
                    let caret = Rectangle::new(*at + origin, Size::new(1.5, line));
                    if let Some(visible) = area.intersection(&caret) {
                        renderer.fill_quad(
                            taim_core::renderer::Quad {
                                bounds: visible,
                                ..Default::default()
                            },
                            text_color,
                        );
                    }
                }
                Selection::Range(ranges) => {
                    let tint = Color {
                        a: 0.35,
                        ..palette.primary.base.color
                    };
                    for range in ranges.iter() {
                        if let Some(visible) = area.intersection(&(*range + origin)) {
                            renderer.fill_quad(
                                taim_core::renderer::Quad {
                                    bounds: visible,
                                    ..Default::default()
                                },
                                tint,
                            );
                        }
                    }
                }
                Selection::Caret(_) => {}
            }
        });
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let area = self.text_area(cx.bounds());
        let local = |at: Point| Point::new(at.x - area.x, at.y - area.y);

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if cx.is_hovered() => {
                cx.request_focus();
                let Some(at) = cx.pointer() else {
                    return Handled::Yes;
                };

                let action = match self.clicks.register(at, cx.now()) {
                    1 => Action::Click(local(at)),
                    2 => Action::SelectWord,
                    _ => Action::SelectLine,
                };
                if matches!(action, Action::SelectWord | Action::SelectLine) {
                    self.buffer.perform(Action::Click(local(at)));
                }
                self.dragging = true;
                self.apply(cx, action);
                Handled::Yes
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) if self.dragging => {
                self.apply(cx, Action::Drag(local(*position)));
                Handled::Yes
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if self.dragging => {
                self.dragging = false;
                Handled::Yes
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cx.is_hovered() => {
                let size = self.size.unwrap_or(Pixels(14.0));
                let line = self.line_height.to_absolute(size).0.max(1.0);
                let lines = match *delta {
                    mouse::ScrollDelta::Lines { y, .. } => -y * 3.0,
                    mouse::ScrollDelta::Pixels { y, .. } => -y / line,
                };
                let lines = lines.round() as i32;
                if lines == 0 {
                    return Handled::Yes;
                }
                self.apply(cx, Action::Scroll { lines });
                Handled::Yes
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            }) if cx.has_focus() => self.key(cx, key, *modifiers, text.as_deref()),
            Event::InputMethod(input_method::Event::Commit(text)) if cx.has_focus() => {
                for character in text.chars() {
                    self.apply(cx, Action::Edit(Edit::Insert(character)));
                }
                Handled::Yes
            }
            Event::Clipboard(clipboard::Event::Read(Ok(content))) if self.awaiting_paste => {
                self.awaiting_paste = false;
                if let clipboard::Content::Text(text) = &**content {
                    self.apply(cx, Action::Edit(Edit::Paste(Arc::new(text.clone()))));
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        let node = semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::TextField)
            .with_name(self.placeholder.clone().unwrap_or_default())
            .with_state(look::interactive_state(true, cx.has_focus()));

        Some(node.with_value(semantics::Value::Text(self.buffer.text().into())))
    }

    fn cursor(&self) -> Option<mouse::Interaction> {
        Some(mouse::Interaction::Text)
    }

    fn focusable(&self) -> bool {
        true
    }

    fn takes_text(&self) -> bool {
        true
    }

    fn clips(&self) -> bool {
        true
    }
}

/// Layout and style settings the engine lays its text out with.
struct Layout {
    bounds: Size,
    font: Font,
    size: Pixels,
    line_height: LineHeight,
    wrapping: Wrapping,
    hint: Option<f32>,
}

/// Where the text's colors come from.
trait Colors {
    /// Applies `layout`, re-highlighting what it changes.
    fn layout(&mut self, engine: &mut Engine, layout: Layout);

    /// Colors the lines about to be drawn; `restart` re-colors them all
    /// (the theme changed).
    fn color(&self, engine: &mut Engine, font: Font, theme: &Theme, restart: bool);
}

/// No highlighting: every glyph in the text color.
struct Plain;

impl Colors for Plain {
    fn layout(&mut self, engine: &mut Engine, layout: Layout) {
        engine.update(
            layout.bounds,
            layout.font,
            layout.size,
            layout.line_height,
            layout.wrapping,
            layout.hint,
            &mut PlainText,
        );
    }

    fn color(&self, _: &mut Engine, _: Font, _: &Theme, _: bool) {}
}

/// Highlighting by `H`.
struct Highlighted<H: Highlighter> {
    highlighter: RefCell<H>,
    settings: H::Settings,
    #[allow(clippy::type_complexity)]
    format: Box<dyn Fn(&H::Highlight, &Theme) -> highlighter::Format<Font>>,
}

impl<H: Highlighter> Colors for Highlighted<H> {
    fn layout(&mut self, engine: &mut Engine, layout: Layout) {
        let highlighter = self.highlighter.get_mut();
        highlighter.update(&self.settings);
        engine.update(
            layout.bounds,
            layout.font,
            layout.size,
            layout.line_height,
            layout.wrapping,
            layout.hint,
            highlighter,
        );
    }

    fn color(&self, engine: &mut Engine, font: Font, theme: &Theme, restart: bool) {
        let mut highlighter = self.highlighter.borrow_mut();
        if restart {
            highlighter.change_line(0);
        }
        engine.highlight(font, &mut *highlighter, |highlight| {
            (self.format)(highlight, theme)
        });
    }
}

/// Counts presses that land close together in time and space.
#[derive(Default)]
struct Clicks {
    last: Option<(Point, crate::Instant)>,
    count: u8,
}

impl Clicks {
    fn register(&mut self, at: Point, now: crate::Instant) -> u8 {
        let repeated = self.last.is_some_and(|(previous, then)| {
            now.saturating_duration_since(then).as_millis() < 450
                && (previous.x - at.x).abs() < 4.0
                && (previous.y - at.y).abs() < 4.0
        });

        self.count = if repeated { self.count % 3 + 1 } else { 1 };
        self.last = Some((at, now));
        self.count
    }
}
