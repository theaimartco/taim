use super::look;
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Axis, Constraints, Sizing};
use crate::state::{IntoProp, Prop, State};
use crate::{Border, Event, Handled, Padding, Point, Rectangle, Renderer, Size, View};
use crate::{keyboard, mouse, semantics};

use std::cell::Cell;
use taim_core::Renderer as _;
use taim_core::text::{self, Paragraph as _, Renderer as _};
use taim_core::{clipboard, input_method, widget};
use unicode_segmentation::UnicodeSegmentation;

type Paragraph = <Renderer as text::Renderer>::Paragraph;

/// A single-line text field bound to `value`: typing changes `value`, and
/// changing `value` elsewhere updates the field.
pub fn text_field(placeholder: impl IntoProp<String>, value: State<String>) -> TextField {
    TextField {
        placeholder: placeholder.into_prop(),
        value,
        shown: value.into_prop(),
        on_submit: None,
        secret: false,
        padding: Padding::from([6.0, 10.0]),
        caret: Caret::at(0),
        dragging: false,
        awaiting_paste: false,
        shaped: None,
        scroll: Cell::new(0.0),
        id: widget::Id::unique(),
    }
}

/// See [`text_field`].
pub struct TextField {
    placeholder: Prop<String>,
    value: State<String>,
    shown: Prop<String>,
    on_submit: Option<Box<dyn Fn()>>,
    secret: bool,
    padding: Padding,
    caret: Caret,
    dragging: bool,
    awaiting_paste: bool,
    /// The displayed text (value, bullets, or placeholder), shaped.
    shaped: Option<(Paragraph, bool)>,
    /// Horizontal scroll keeping the caret visible.
    scroll: Cell<f32>,
    id: widget::Id,
}

/// Caret and selection, in grapheme indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Caret {
    position: usize,
    /// The other end of the selection, if any text is selected.
    anchor: Option<usize>,
}

impl Caret {
    fn at(position: usize) -> Self {
        Self {
            position,
            anchor: None,
        }
    }

    /// The selected range, if non-empty.
    fn selection(self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        (anchor != self.position).then(|| (anchor.min(self.position), anchor.max(self.position)))
    }

    /// Moves to `position`, extending the selection when `extend`.
    fn move_to(&mut self, position: usize, extend: bool) {
        if extend {
            let _ = self.anchor.get_or_insert(self.position);
        } else {
            self.anchor = None;
        }
        self.position = position;
    }
}

/// Text edits on grapheme boundaries.
mod edit {
    use super::UnicodeSegmentation;

    pub fn count(text: &str) -> usize {
        text.graphemes(true).count()
    }

    /// Byte offset of grapheme `index` (or the end).
    pub fn byte(text: &str, index: usize) -> usize {
        text.grapheme_indices(true)
            .nth(index)
            .map_or(text.len(), |(offset, _)| offset)
    }

    pub fn replace(text: &str, from: usize, to: usize, with: &str) -> String {
        let (start, end) = (byte(text, from), byte(text, to));
        let mut edited = String::with_capacity(text.len() + with.len());
        edited.push_str(&text[..start]);
        edited.push_str(with);
        edited.push_str(&text[end..]);
        edited
    }

    pub fn slice(text: &str, from: usize, to: usize) -> &str {
        &text[byte(text, from)..byte(text, to)]
    }

    /// Start of the word before `position`.
    pub fn word_left(text: &str, position: usize) -> usize {
        let mut index = 0;
        let mut start = 0;
        for word in text.split_word_bounds() {
            if index >= position {
                break;
            }
            if !word.trim().is_empty() {
                start = index;
            }
            index += count(word);
        }
        start
    }

    /// End of the word after `position`.
    pub fn word_right(text: &str, position: usize) -> usize {
        let mut index = 0;
        for word in text.split_word_bounds() {
            index += count(word);
            if index > position && !word.trim().is_empty() {
                return index;
            }
        }
        count(text)
    }
}

impl TextField {
    /// Runs `action` when Enter is pressed.
    pub fn on_submit(mut self, action: impl Fn() + 'static) -> Self {
        self.on_submit = Some(Box::new(action));
        self
    }

    /// Hides the text (for passwords).
    pub fn secret(mut self) -> Self {
        self.secret = true;
        self
    }

    fn text_area(&self, bounds: Rectangle) -> Rectangle {
        bounds.shrink(self.padding)
    }

    /// Replaces the selection (or inserts at the caret) with `inserted`.
    fn insert(&mut self, inserted: &str) {
        let current = self.value.get();
        let (from, to) = self
            .caret
            .selection()
            .unwrap_or((self.caret.position, self.caret.position));
        let to = to.min(edit::count(&current));
        let from = from.min(to);

        let _ = self
            .value
            .replace(edit::replace(&current, from, to, inserted));
        self.caret = Caret::at(from + edit::count(inserted));
    }

    /// Deletes the selection, or `range(caret)` when nothing is selected.
    fn delete(&mut self, range: impl FnOnce(&str, usize) -> (usize, usize)) {
        let current = self.value.get();
        let (from, to) = self
            .caret
            .selection()
            .unwrap_or_else(|| range(&current, self.caret.position));

        if from == to {
            return;
        }

        let _ = self.value.replace(edit::replace(&current, from, to, ""));
        self.caret = Caret::at(from);
    }

    fn caret_from_pointer(&self, bounds: Rectangle, at: Point) -> usize {
        let area = self.text_area(bounds);
        let local = Point::new(at.x - area.x + self.scroll.get(), area.height / 2.0);

        match &self.shaped {
            Some((paragraph, false)) => paragraph
                .hit_test(local)
                .map_or_else(|| edit::count(&self.shown.get()), text::Hit::cursor),
            _ => 0,
        }
    }

    fn caret_x(&self, position: usize) -> f32 {
        match &self.shaped {
            Some((paragraph, false)) => paragraph
                .grapheme_position(0, position)
                .map_or(0.0, |point| point.x),
            _ => 0.0,
        }
    }

    /// Handles a key press while focused.
    fn key(
        &mut self,
        cx: &mut EventCx<'_>,
        key: &keyboard::Key,
        modifiers: keyboard::Modifiers,
        typed: Option<&str>,
    ) -> Handled {
        use keyboard::key::Named;

        let value = self.value.get();
        let length = edit::count(&value);
        let extend = modifiers.shift();
        let word = modifiers.command() || modifiers.alt();

        match key {
            keyboard::Key::Named(Named::ArrowLeft) => {
                let target = if word {
                    edit::word_left(&value, self.caret.position)
                } else if let (Some((from, _)), false) = (self.caret.selection(), extend) {
                    from
                } else {
                    self.caret.position.saturating_sub(1)
                };
                self.caret.move_to(target, extend);
            }
            keyboard::Key::Named(Named::ArrowRight) => {
                let target = if word {
                    edit::word_right(&value, self.caret.position)
                } else if let (Some((_, to)), false) = (self.caret.selection(), extend) {
                    to
                } else {
                    (self.caret.position + 1).min(length)
                };
                self.caret.move_to(target, extend);
            }
            keyboard::Key::Named(Named::Home) => self.caret.move_to(0, extend),
            keyboard::Key::Named(Named::End) => self.caret.move_to(length, extend),
            keyboard::Key::Named(Named::Backspace) => self.delete(|text, at| {
                let from = if word {
                    edit::word_left(text, at)
                } else {
                    at.saturating_sub(1)
                };
                (from, at)
            }),
            keyboard::Key::Named(Named::Delete) => self.delete(|text, at| {
                let to = if word {
                    edit::word_right(text, at)
                } else {
                    (at + 1).min(edit::count(text))
                };
                (at, to)
            }),
            keyboard::Key::Named(Named::Enter) => {
                if let Some(submit) = &self.on_submit {
                    submit();
                }
            }
            keyboard::Key::Named(Named::Escape) => cx.release_focus(),
            keyboard::Key::Character(character) if modifiers.command() => {
                match character.as_str() {
                    "a" => {
                        self.caret = Caret {
                            position: length,
                            anchor: Some(0),
                        }
                    }
                    "c" | "x" if !self.secret => {
                        if let Some((from, to)) = self.caret.selection() {
                            cx.copy(edit::slice(&value, from, to));
                            if character.as_str() == "x" {
                                self.delete(|_, at| (at, at));
                            }
                        }
                    }
                    "v" => {
                        self.awaiting_paste = true;
                        cx.paste();
                    }
                    _ => return Handled::No,
                }
            }
            _ => match typed.filter(|text| !text.chars().any(char::is_control)) {
                Some(text) if !modifiers.command() => self.insert(text),
                _ => return Handled::No,
            },
        }

        cx.request_paint();
        Handled::Yes
    }
}

impl View for TextField {
    fn sizing(&self) -> Sizing {
        Sizing {
            width: Axis::Fill(1),
            height: Axis::Hug,
        }
    }

    // With no renderer compiled in, the paragraph type is `()`.
    #[allow(clippy::let_unit_value)]
    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let renderer = cx.renderer();
        let value = self.shown.get();
        let placeholder = value.is_empty();
        let display = if placeholder {
            self.placeholder.get()
        } else if self.secret {
            "•".repeat(edit::count(&value))
        } else {
            value
        };

        let size = renderer.default_size();
        let paragraph = Paragraph::with_text(text::Text {
            content: display.as_str(),
            bounds: Size::new(f32::INFINITY, f32::INFINITY),
            size,
            line_height: text::LineHeight::default(),
            font: renderer.default_font(),
            align_x: text::Alignment::Left,
            align_y: taim_core::alignment::Vertical::Top,
            shaping: text::Shaping::default(),
            wrapping: text::Wrapping::None,
            ellipsis: text::Ellipsis::None,
            hint_factor: renderer.scale_factor(),
        });

        let line = text::LineHeight::default().to_absolute(size).0;
        self.shaped = Some((paragraph, placeholder));
        self.caret.position = self.caret.position.min(edit::count(&self.shown.get()));

        let sized = constraints.with_sizing(self.sizing());
        let width = if sized.max.width.is_finite() {
            sized.max.width
        } else {
            240.0
        };
        Size::new(width, line + self.padding.top + self.padding.bottom)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let focused = cx.has_focus();
        let edge = if focused {
            palette.primary.base.color
        } else if cx.is_hovered() {
            palette.background.strongest.color
        } else {
            palette.background.strong.color
        };

        cx.fill(
            bounds,
            palette.background.base.color,
            Border::default()
                .width(if focused { 2.0 } else { 1.0 })
                .color(edge)
                .rounded(6.0),
        );

        let Some((paragraph, placeholder)) = &self.shaped else {
            return;
        };

        let area = self.text_area(bounds);
        let caret_x = if *placeholder {
            0.0
        } else {
            self.caret_x(self.caret.position)
        };

        // Keep the caret inside the visible area.
        let mut scroll = self.scroll.get();
        if caret_x - scroll > area.width {
            scroll = caret_x - area.width;
        } else if caret_x < scroll {
            scroll = caret_x;
        }
        self.scroll.set(scroll.max(0.0));
        let origin = Point::new(area.x - self.scroll.get(), area.y);

        let text_color = if *placeholder {
            palette.background.strongest.color
        } else {
            cx.text_color()
        };
        let selection = (!*placeholder).then(|| self.caret.selection()).flatten();
        let caret = Rectangle::new(
            Point::new(origin.x + caret_x, area.y),
            Size::new(1.5, area.height),
        );
        let highlight = selection.map(|(from, to)| {
            let (start, end) = (self.caret_x(from), self.caret_x(to));
            Rectangle::new(
                Point::new(origin.x + start, area.y),
                Size::new(end - start, area.height),
            )
        });
        let selection_color = palette.primary.weak.color;

        cx.renderer().with_layer(area, |renderer| {
            if let Some(highlight) = highlight {
                renderer.fill_quad(
                    taim_core::renderer::Quad {
                        bounds: highlight,
                        ..Default::default()
                    },
                    selection_color,
                );
            }

            renderer.fill_paragraph(paragraph, origin, text_color, area);

            if focused {
                renderer.fill_quad(
                    taim_core::renderer::Quad {
                        bounds: caret,
                        ..Default::default()
                    },
                    text_color,
                );
            }
        });
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let bounds = cx.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if cx.is_hovered() => {
                cx.request_focus();
                if let Some(at) = cx.pointer() {
                    let position = self.caret_from_pointer(bounds, at);
                    self.caret.move_to(position, false);
                    self.caret.anchor = Some(position);
                }
                self.dragging = true;
                cx.request_paint();
                Handled::Yes
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) if self.dragging => {
                let target = self.caret_from_pointer(bounds, *position);
                self.caret.position = target;
                cx.request_paint();
                Handled::Yes
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if self.dragging => {
                self.dragging = false;
                Handled::Yes
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            }) if cx.has_focus() => self.key(cx, key, *modifiers, text.as_deref()),
            Event::InputMethod(input_method::Event::Commit(text)) if cx.has_focus() => {
                self.insert(text);
                cx.request_paint();
                Handled::Yes
            }
            Event::Clipboard(clipboard::Event::Read(Ok(content))) if self.awaiting_paste => {
                self.awaiting_paste = false;
                if let clipboard::Content::Text(text) = &**content {
                    let line = text.lines().next().unwrap_or_default().to_owned();
                    self.insert(&line);
                    cx.request_paint();
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        let role = if self.secret {
            semantics::Role::SecureField
        } else {
            semantics::Role::TextField
        };
        let node = semantics::Node::new(self.id.clone(), cx.bounds(), role)
            .with_name(self.placeholder.get())
            .with_state(look::interactive_state(true, cx.has_focus()));

        // `with_value` refuses to disclose secure fields by itself.
        Some(node.with_value(semantics::Value::Text(self.shown.get().into())))
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
}

#[cfg(test)]
mod tests {
    use super::edit;

    #[test]
    fn edits_respect_grapheme_boundaries() {
        let text = "ae\u{301}b"; // a, é (e + combining accent), b
        assert_eq!(edit::count(text), 3);
        assert_eq!(edit::replace(text, 1, 2, "x"), "axb");
        assert_eq!(edit::slice(text, 1, 3), "e\u{301}b");
    }

    #[test]
    fn word_jumps() {
        let text = "hello big world";
        assert_eq!(edit::word_right(text, 0), 5);
        assert_eq!(edit::word_right(text, 5), 9);
        assert_eq!(edit::word_left(text, 15), 10);
        assert_eq!(edit::word_left(text, 9), 6);
        assert_eq!(edit::word_left(text, 0), 0);
    }
}
