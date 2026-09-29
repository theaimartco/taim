//! Draw and edit text.
mod selection;
mod state;
mod update;

use state::Internal;
pub use state::Weak;

use crate::core::text::editor::{self, Action, Cursor, Direction, Edit, Motion, Position};
use crate::core::text::highlighter::{self, Highlighter};
use crate::core::text::{LineHeight, Wrapping};
use crate::core::{Font, Pixels, Size};
use crate::text;

use cosmic_text::Edit as _;

use std::borrow::Cow;
use std::sync::Arc;

/// A multi-line text editor.
#[derive(Debug, PartialEq)]
pub struct Editor(Option<Arc<Internal>>);

impl Editor {
    /// Creates a new empty [`Editor`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the buffer of the [`Editor`].
    pub fn buffer(&self) -> &cosmic_text::Buffer {
        buffer_from_editor(&self.internal().editor)
    }

    /// Retains the current editor layout for deferred rendering.
    ///
    /// This keeps the shaped buffer alive until a rendering tree is consumed.
    /// Editing while a frame is pending uses copy-on-write so the frame keeps
    /// the geometry that was measured for its own viewport.
    pub fn downgrade(&self) -> Weak {
        let editor = self.internal();

        Weak {
            raw: editor.clone(),
            bounds: editor.bounds,
        }
    }

    fn internal(&self) -> &Arc<Internal> {
        self.0
            .as_ref()
            .expect("Editor should always be initialized")
    }

    fn with_internal_mut<T>(&mut self, f: impl FnOnce(&mut Internal) -> T) -> T {
        let editor = self.0.take().expect("Editor should always be initialized");

        let mut internal = Arc::try_unwrap(editor).unwrap_or_else(|snapshot| (*snapshot).clone());

        // Clear cursor cache
        let _ = internal
            .selection
            .write()
            .expect("Write to cursor cache")
            .take();

        let result = f(&mut internal);

        self.0 = Some(Arc::new(internal));

        result
    }
}

impl editor::Editor for Editor {
    type Font = Font;

    fn with_text(text: &str) -> Self {
        let mut buffer = cosmic_text::Buffer::new_empty(cosmic_text::Metrics {
            font_size: 1.0,
            line_height: 1.0,
        });

        let mut font_system = text::font_system()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        buffer.set_text(
            text,
            &cosmic_text::Attrs::new(),
            cosmic_text::Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(font_system.raw(), false);

        Editor(Some(Arc::new(Internal {
            editor: cosmic_text::Editor::new(buffer),
            version: font_system.version(),
            ..Default::default()
        })))
    }

    fn is_empty(&self) -> bool {
        let buffer = self.buffer();

        buffer.lines.is_empty() || (buffer.lines.len() == 1 && buffer.lines[0].text().is_empty())
    }

    fn line(&self, index: usize) -> Option<editor::Line<'_>> {
        self.buffer().lines.get(index).map(|line| editor::Line {
            text: Cow::Borrowed(line.text()),
            ending: match line.ending() {
                cosmic_text::LineEnding::Lf => editor::LineEnding::Lf,
                cosmic_text::LineEnding::CrLf => editor::LineEnding::CrLf,
                cosmic_text::LineEnding::Cr => editor::LineEnding::Cr,
                cosmic_text::LineEnding::LfCr => editor::LineEnding::LfCr,
                cosmic_text::LineEnding::None => editor::LineEnding::None,
            },
        })
    }

    fn line_count(&self) -> usize {
        self.buffer().lines.len()
    }

    fn scroll_position(&self) -> (usize, f32) {
        let internal = self.internal();
        let buffer = buffer_from_editor(&internal.editor);
        let scroll = buffer.scroll();
        let line_height = buffer.metrics().line_height.max(1.0);
        let rows = (scroll.vertical.max(0.0) / line_height).floor() as usize;
        let line = (scroll.line + rows).min(buffer.lines.len().saturating_sub(1));
        let remainder = scroll.vertical.max(0.0) % line_height;
        (line, remainder / internal.hint_factor.max(0.001))
    }

    fn copy(&self) -> Option<String> {
        self.internal().editor.copy_selection()
    }

    fn selection(&self) -> editor::Selection {
        selection::resolve(self.internal())
    }

    fn cursor(&self) -> Cursor {
        let editor = &self.internal().editor;

        let position = {
            let cursor = editor.cursor();

            Position {
                line: cursor.line,
                column: cursor.index,
            }
        };

        let selection = match editor.selection() {
            cosmic_text::Selection::None => None,
            cosmic_text::Selection::Normal(cursor)
            | cosmic_text::Selection::Line(cursor)
            | cosmic_text::Selection::Word(cursor) => Some(Position {
                line: cursor.line,
                column: cursor.index,
            }),
        };

        Cursor {
            position,
            selection,
        }
    }

    fn perform(&mut self, action: Action) {
        let mut font_system = text::font_system()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        self.with_internal_mut(|internal| {
            let editor = &mut internal.editor;

            match action {
                // Motion events
                Action::Move(motion) => {
                    if let Some((start, end)) = editor.selection_bounds() {
                        editor.set_selection(cosmic_text::Selection::None);

                        match motion {
                            // These motions are performed as-is even when a selection
                            // is present
                            Motion::Home
                            | Motion::End
                            | Motion::DocumentStart
                            | Motion::DocumentEnd => {
                                editor.action(
                                    font_system.raw(),
                                    cosmic_text::Action::Motion(to_motion(motion)),
                                );
                            }
                            // Other motions simply move the cursor to one end of the selection
                            _ => editor.set_cursor(match motion.direction() {
                                Direction::Left => start,
                                Direction::Right => end,
                            }),
                        }
                    } else {
                        editor.action(
                            font_system.raw(),
                            cosmic_text::Action::Motion(to_motion(motion)),
                        );
                    }
                }

                // Selection events
                Action::Select(motion) => {
                    let cursor = editor.cursor();

                    if editor.selection_bounds().is_none() {
                        editor.set_selection(cosmic_text::Selection::Normal(cursor));
                    }

                    editor.action(
                        font_system.raw(),
                        cosmic_text::Action::Motion(to_motion(motion)),
                    );

                    // Deselect if selection matches cursor position
                    if let Some((start, end)) = editor.selection_bounds()
                        && start.line == end.line
                        && start.index == end.index
                    {
                        editor.set_selection(cosmic_text::Selection::None);
                    }
                }
                Action::SelectWord => {
                    let cursor = editor.cursor();

                    editor.set_selection(cosmic_text::Selection::Word(cursor));
                }
                Action::SelectLine => {
                    let cursor = editor.cursor();

                    editor.set_selection(cosmic_text::Selection::Line(cursor));
                }
                Action::SelectAll => {
                    let buffer = buffer_from_editor(editor);

                    if buffer.lines.len() > 1
                        || buffer
                            .lines
                            .first()
                            .is_some_and(|line| !line.text().is_empty())
                    {
                        let cursor = editor.cursor();

                        editor.set_selection(cosmic_text::Selection::Normal(cosmic_text::Cursor {
                            line: 0,
                            index: 0,
                            ..cursor
                        }));

                        editor.action(
                            font_system.raw(),
                            cosmic_text::Action::Motion(cosmic_text::Motion::BufferEnd),
                        );
                    }
                }

                // Editing events
                Action::Edit(edit) => {
                    let topmost_line_before_edit = editor
                        .selection_bounds()
                        .map(|(start, _)| start)
                        .unwrap_or_else(|| editor.cursor())
                        .line;

                    match edit {
                        Edit::Insert(c) => {
                            editor.action(font_system.raw(), cosmic_text::Action::Insert(c));
                        }
                        Edit::Paste(text) => {
                            editor.insert_string(&text, None);
                        }
                        Edit::Indent => {
                            editor.action(font_system.raw(), cosmic_text::Action::Indent);
                        }
                        Edit::Unindent => {
                            editor.action(font_system.raw(), cosmic_text::Action::Unindent);
                        }
                        Edit::Enter => {
                            editor.action(font_system.raw(), cosmic_text::Action::Enter);
                        }
                        Edit::Backspace => {
                            editor.action(font_system.raw(), cosmic_text::Action::Backspace);
                        }
                        Edit::Delete => {
                            editor.action(font_system.raw(), cosmic_text::Action::Delete);
                        }
                    }

                    let cursor = editor.cursor();
                    let selection_start = editor
                        .selection_bounds()
                        .map(|(start, _)| start)
                        .unwrap_or(cursor);

                    internal.topmost_line_changed =
                        Some(selection_start.line.min(topmost_line_before_edit));
                }

                // Mouse events
                Action::Click(position) => {
                    editor.action(
                        font_system.raw(),
                        cosmic_text::Action::Click {
                            x: (position.x * internal.hint_factor) as i32,
                            y: (position.y * internal.hint_factor) as i32,
                        },
                    );
                }
                Action::Drag(position) => {
                    editor.action(
                        font_system.raw(),
                        cosmic_text::Action::Drag {
                            x: (position.x * internal.hint_factor) as i32,
                            y: (position.y * internal.hint_factor) as i32,
                        },
                    );

                    // Deselect if selection matches cursor position
                    if let Some((start, end)) = editor.selection_bounds()
                        && start.line == end.line
                        && start.index == end.index
                    {
                        editor.set_selection(cosmic_text::Selection::None);
                    }
                }
                Action::Scroll { lines } => {
                    editor.action(
                        font_system.raw(),
                        cosmic_text::Action::Scroll {
                            pixels: lines as f32 * buffer_from_editor(editor).metrics().line_height,
                        },
                    );
                    editor.shape_as_needed(font_system.raw(), false);
                }
            }
        });
    }

    fn move_to(&mut self, cursor: Cursor) {
        self.with_internal_mut(|internal| {
            // TODO: Expose `Affinity`
            internal.editor.set_cursor(cosmic_text::Cursor {
                line: cursor.position.line,
                index: cursor.position.column,
                affinity: cosmic_text::Affinity::Before,
            });

            if let Some(selection) = cursor.selection {
                internal
                    .editor
                    .set_selection(cosmic_text::Selection::Normal(cosmic_text::Cursor {
                        line: selection.line,
                        index: selection.column,
                        affinity: cosmic_text::Affinity::Before,
                    }));
            }
        });
    }

    fn bounds(&self) -> Size {
        self.internal().bounds
    }

    fn min_bounds(&self) -> Size {
        let internal = self.internal();

        let (bounds, _has_rtl) = text::measure(buffer_from_editor(&internal.editor));

        bounds * (1.0 / internal.hint_factor)
    }

    fn hint_factor(&self) -> Option<f32> {
        let internal = self.internal();

        internal.hint.then_some(internal.hint_factor)
    }

    fn update(
        &mut self,
        new_bounds: Size,
        new_font: Font,
        new_size: Pixels,
        new_line_height: LineHeight,
        new_wrapping: Wrapping,
        new_hint_factor: Option<f32>,
        new_highlighter: &mut impl Highlighter,
    ) {
        update::apply(
            self,
            new_bounds,
            new_font,
            new_size,
            new_line_height,
            new_wrapping,
            new_hint_factor,
            new_highlighter,
        );
    }

    fn highlight<H: Highlighter>(
        &mut self,
        font: Self::Font,
        highlighter: &mut H,
        format_highlight: impl Fn(&H::Highlight) -> highlighter::Format<Self::Font>,
    ) {
        let internal = self.internal();
        let buffer = buffer_from_editor(&internal.editor);

        let scroll = buffer.scroll();
        let mut window = (internal.bounds.height * internal.hint_factor
            / buffer.metrics().line_height)
            .ceil() as i32;

        let last_visible_line = buffer.lines[scroll.line..]
            .iter()
            .enumerate()
            .find_map(|(i, line)| {
                let visible_lines = line
                    .layout_opt()
                    .as_ref()
                    .expect("Line layout should be cached")
                    .len() as i32;

                if window > visible_lines {
                    window -= visible_lines;
                    None
                } else {
                    Some(scroll.line + i)
                }
            })
            .unwrap_or(buffer.lines.len().saturating_sub(1));

        let current_line = highlighter.current_line();

        if current_line > last_visible_line {
            return;
        }

        let editor = self.0.take().expect("Editor should always be initialized");

        let mut internal = Arc::try_unwrap(editor).unwrap_or_else(|snapshot| (*snapshot).clone());

        let mut font_system = text::font_system()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let attributes = text::to_attributes(font);

        for line in &mut buffer_mut_from_editor(&mut internal.editor).lines
            [current_line..=last_visible_line]
        {
            let mut list = cosmic_text::AttrsList::new(&attributes);

            for (range, highlight) in highlighter.highlight_line(line.text()) {
                let Some(range) = highlighter::valid_range(line.text(), range) else {
                    continue;
                };
                let format = format_highlight(&highlight);

                if !format.is_empty() {
                    list.add_span(
                        range,
                        &cosmic_text::Attrs {
                            color_opt: format.color.map(text::to_color),
                            ..if let Some(font) = format.font {
                                text::to_attributes(font)
                            } else {
                                attributes.clone()
                            }
                        },
                    );
                }
            }

            let _ = line.set_attrs_list(list);
        }

        internal.editor.shape_as_needed(font_system.raw(), false);

        self.0 = Some(Arc::new(internal));
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self(Some(Arc::new(Internal::default())))
    }
}

fn to_motion(motion: Motion) -> cosmic_text::Motion {
    match motion {
        Motion::Left => cosmic_text::Motion::Left,
        Motion::Right => cosmic_text::Motion::Right,
        Motion::Up => cosmic_text::Motion::Up,
        Motion::Down => cosmic_text::Motion::Down,
        Motion::WordLeft => cosmic_text::Motion::LeftWord,
        Motion::WordRight => cosmic_text::Motion::RightWord,
        Motion::Home => cosmic_text::Motion::Home,
        Motion::End => cosmic_text::Motion::End,
        Motion::PageUp => cosmic_text::Motion::PageUp,
        Motion::PageDown => cosmic_text::Motion::PageDown,
        Motion::DocumentStart => cosmic_text::Motion::BufferStart,
        Motion::DocumentEnd => cosmic_text::Motion::BufferEnd,
    }
}

fn buffer_from_editor<'a, 'b>(editor: &'a impl cosmic_text::Edit<'b>) -> &'a cosmic_text::Buffer
where
    'b: 'a,
{
    match editor.buffer_ref() {
        cosmic_text::BufferRef::Owned(buffer) => buffer,
        cosmic_text::BufferRef::Borrowed(buffer) => buffer,
        cosmic_text::BufferRef::Arc(buffer) => buffer,
    }
}

fn buffer_mut_from_editor<'a, 'b>(
    editor: &'a mut impl cosmic_text::Edit<'b>,
) -> &'a mut cosmic_text::Buffer
where
    'b: 'a,
{
    match editor.buffer_ref_mut() {
        cosmic_text::BufferRef::Owned(buffer) => buffer,
        cosmic_text::BufferRef::Borrowed(buffer) => buffer,
        cosmic_text::BufferRef::Arc(_buffer) => unreachable!(),
    }
}
