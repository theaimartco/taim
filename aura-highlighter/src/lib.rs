//! Incremental syntax highlighting for Aura, with no external dependencies.
//!
//! # What this is
//!
//! A line-oriented tokenizer. [`Stream`] consumes one line at a time,
//! carrying only the state that genuinely crosses a line boundary (an
//! open block comment or an open multi-line string), and emits byte
//! ranges tagged with a [`Scope`]. A [`Theme`] turns those scopes into
//! colours and fonts.
//!
//! # What this is not
//!
//! It is not a grammar engine. It cannot resolve types, follow includes,
//! expand macros, or highlight embedded languages. Every [`Syntax`] in
//! [`syntax::ALL`] is a lexical description -- comments, strings,
//! numbers, keywords, built-in type names, call sites -- which is the
//! level a fenced code block in a document actually needs. Choosing this
//! bound is what lets the crate stay dependency-free, deterministic, and
//! cheap enough to re-run on every keystroke.
//!
//! # Incrementality
//!
//! [`Stream::highlight_line`] is *idempotent*: it highlights a line
//! against the last committed state without advancing it, so re-running
//! it on an unchanged line yields the same spans. [`Stream::commit`]
//! advances the state to the end of the line just highlighted, and
//! [`Stream::reset`] returns to the start of the document. A caller that
//! caches highlighted lines only has to re-feed the lines *after* the one
//! that changed.
//!
//! ```
//! use aura_highlighter::{Scope, Settings, Stream, Theme};
//!
//! let mut stream = Stream::new(&Settings::new(Theme::Dark, "rust"));
//!
//! let spans: Vec<_> = stream.highlight_line("let x = 1; // hi").collect();
//!
//! assert_eq!(spans[0].1.scope(), Scope::Keyword);
//! assert_eq!(spans.last().unwrap().1.scope(), Scope::Comment);
//! ```
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/theaimartco/vyom-aura/main/docs/logo.svg"
)]
pub mod syntax;
pub mod theme;

pub use syntax::Syntax;
pub use theme::{Scope, Theme};

use aura_core::text::highlighter;
use aura_core::{Color, Font, font};

use std::ops::Range;

/// The configuration of a [`Stream`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Settings {
    /// The [`Theme`] used to colour the produced tokens.
    pub theme: Theme,
    /// The token naming the language to highlight.
    ///
    /// Resolved through [`Syntax::find`], so both `"rust"` and `".rs"`
    /// work, and an unknown token degrades to [`Syntax::PLAIN`] instead
    /// of failing.
    pub token: String,
}

impl Settings {
    /// Creates [`Settings`] for the given [`Theme`] and language token.
    pub fn new(theme: Theme, token: impl Into<String>) -> Self {
        Self {
            theme,
            token: token.into(),
        }
    }

    /// Returns the [`Syntax`] these [`Settings`] resolve to.
    pub fn syntax(&self) -> &'static Syntax {
        Syntax::find(&self.token)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new(Theme::default(), "")
    }
}

/// A highlighted token produced by a [`Stream`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Highlight {
    scope: Scope,
    theme: Theme,
}

impl Highlight {
    /// Returns the [`Scope`] this [`Highlight`] classified its token as.
    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// Returns the [`Color`] of this [`Highlight`].
    pub fn color(&self) -> Option<Color> {
        Some(self.theme.color(self.scope))
    }

    /// Returns the [`Font`] of this [`Highlight`].
    ///
    /// Only comments get a distinct font (italic). Everything else keeps
    /// the surrounding monospace font so column alignment survives.
    pub fn font(&self) -> Option<Font> {
        match self.scope {
            Scope::Comment => Some(Font {
                style: font::Style::Italic,
                ..Font::MONOSPACE
            }),
            _ => None,
        }
    }

    /// Returns this [`Highlight`] as a [`highlighter::Format`].
    pub fn to_format(self) -> highlighter::Format<Font> {
        highlighter::Format {
            color: self.color(),
            font: self.font(),
        }
    }
}

/// The state that carries from one line to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Carry {
    /// The next line starts outside any multi-line construct.
    #[default]
    Ready,
    /// The next line continues a block comment nested `depth` levels deep.
    BlockComment(u32),
    /// The next line continues a multi-line string opened with the
    /// delimiter at this index in [`Syntax::multiline_strings`].
    MultilineString(usize),
}

/// An incremental, line-oriented syntax highlighter.
///
/// See the [crate documentation](crate) for the incrementality contract.
#[derive(Debug, Clone)]
pub struct Stream {
    syntax: &'static Syntax,
    theme: Theme,
    committed: Carry,
    pending: Carry,
    spans: Vec<(Range<usize>, Highlight)>,
}

impl Stream {
    /// Creates a [`Stream`] positioned at the start of a document.
    pub fn new(settings: &Settings) -> Self {
        Self {
            syntax: settings.syntax(),
            theme: settings.theme,
            committed: Carry::Ready,
            pending: Carry::Ready,
            spans: Vec::new(),
        }
    }

    /// Reconfigures this [`Stream`] and returns it to the start of the
    /// document.
    pub fn update(&mut self, settings: &Settings) {
        self.syntax = settings.syntax();
        self.theme = settings.theme;
        self.reset();
    }

    /// Returns the [`Syntax`] this [`Stream`] tokenizes with.
    pub fn syntax(&self) -> &'static Syntax {
        self.syntax
    }

    /// Returns the [`Theme`] this [`Stream`] colours with.
    pub fn theme(&self) -> Theme {
        self.theme
    }

    /// Returns this [`Stream`] to the start of the document.
    pub fn reset(&mut self) {
        self.committed = Carry::Ready;
        self.pending = Carry::Ready;
        self.spans.clear();
    }

    /// Advances the committed state past the line most recently passed to
    /// [`Stream::highlight_line`].
    pub fn commit(&mut self) {
        self.committed = self.pending;
    }

    /// Returns `true` if the line most recently highlighted left a block
    /// comment or multi-line string open.
    pub fn is_inside_multiline(&self) -> bool {
        self.pending != Carry::Ready
    }

    /// Highlights `line`, starting from the committed state.
    ///
    /// This does not advance the committed state; call [`Stream::commit`]
    /// to do that. Highlighting the same line twice therefore produces
    /// the same spans.
    ///
    /// The returned ranges are byte ranges into `line`, always land on
    /// `char` boundaries, are non-empty, are sorted, and together cover
    /// `line` exactly.
    pub fn highlight_line(
        &mut self,
        line: &str,
    ) -> impl Iterator<Item = (Range<usize>, Highlight)> {
        self.pending = tokenize(
            self.syntax,
            line,
            self.committed,
            &mut self.spans,
            self.theme,
        );

        self.spans.iter().cloned()
    }
}

/// A [`highlighter::Highlighter`] backed by a [`Stream`].
///
/// This is the adapter the `text_editor` widget expects: it tracks which
/// line changed and re-feeds the [`Stream`] from there.
#[derive(Debug, Clone)]
pub struct Highlighter {
    stream: Stream,
    current_line: usize,
}

impl highlighter::Highlighter for Highlighter {
    type Settings = Settings;
    type Highlight = Highlight;

    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Highlight)>;

    fn new(settings: &Self::Settings) -> Self {
        Self {
            stream: Stream::new(settings),
            current_line: 0,
        }
    }

    fn update(&mut self, new_settings: &Self::Settings) {
        self.stream.update(new_settings);
        self.current_line = 0;
    }

    fn change_line(&mut self, line: usize) {
        self.stream.reset();
        self.current_line = line;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let spans: Vec<_> = self.stream.highlight_line(line).collect();

        self.stream.commit();
        self.current_line += 1;

        spans.into_iter()
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

/// A `char`-boundary-safe cursor over a line.
struct Cursor<'a> {
    line: &'a str,
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(line: &'a str) -> Self {
        Self { line, offset: 0 }
    }

    fn rest(&self) -> &'a str {
        &self.line[self.offset..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let next = self.peek()?;
        self.offset += next.len_utf8();

        Some(next)
    }

    fn eat(&mut self, prefix: &str) -> bool {
        if self.rest().starts_with(prefix) {
            self.offset += prefix.len();

            true
        } else {
            false
        }
    }

    fn is_done(&self) -> bool {
        self.offset >= self.line.len()
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Tokenizes `line` starting from `carry`, writing merged spans into
/// `out`, and returns the state the next line should start from.
fn tokenize(
    syntax: &Syntax,
    line: &str,
    carry: Carry,
    out: &mut Vec<(Range<usize>, Highlight)>,
    theme: Theme,
) -> Carry {
    out.clear();

    let mut cursor = Cursor::new(line);
    let mut carry = carry;

    // Finish whatever the previous line left open before lexing normally.
    carry = match carry {
        Carry::Ready => Carry::Ready,
        Carry::BlockComment(depth) => {
            continue_block_comment(syntax, &mut cursor, depth, out, theme)
        }
        Carry::MultilineString(index) => {
            continue_multiline_string(syntax, &mut cursor, index, out, theme)
        }
    };

    while carry == Carry::Ready && !cursor.is_done() {
        let start = cursor.offset;

        // Whitespace belongs to the surrounding text run.
        if cursor.peek().is_some_and(char::is_whitespace) {
            while cursor.peek().is_some_and(char::is_whitespace) {
                let _ = cursor.bump();
            }

            push(out, start..cursor.offset, Scope::Text, theme);
            continue;
        }

        if let Some(comment) = syntax
            .line_comments
            .iter()
            .find(|prefix| cursor.rest().starts_with(**prefix))
        {
            let _ = comment;
            cursor.offset = line.len();
            push(out, start..cursor.offset, Scope::Comment, theme);
            continue;
        }

        if let Some((open, _)) = syntax.block_comment
            && cursor.eat(open)
        {
            carry = continue_block_comment(syntax, &mut cursor, 1, out, theme);
            merge_back(out, start, Scope::Comment);
            continue;
        }

        if let Some(index) = syntax
            .multiline_strings
            .iter()
            .position(|open| cursor.rest().starts_with(*open))
        {
            cursor.offset += syntax.multiline_strings[index].len();
            carry = continue_multiline_string(syntax, &mut cursor, index, out, theme);
            merge_back(out, start, Scope::String);
            continue;
        }

        let next = cursor.peek().unwrap_or('\0');

        if syntax.strings.contains(&next) {
            let _ = cursor.bump();
            scan_single_line_string(syntax, &mut cursor, next);
            push(out, start..cursor.offset, Scope::String, theme);
            continue;
        }

        if next.is_ascii_digit() {
            scan_number(&mut cursor);
            push(out, start..cursor.offset, Scope::Number, theme);
            continue;
        }

        if syntax.attribute_sigils.contains(&next) {
            let _ = cursor.bump();

            // `#` alone (a Rust `#[..]`, a C `#include`, a shell `$VAR`)
            // only reads as an attribute when a word follows it.
            if cursor.peek().is_some_and(is_ident_start) || cursor.peek() == Some('[') {
                // `#[` opens a Rust attribute; the bracket belongs to it.
                let _ = cursor.eat("[");

                while cursor.peek().is_some_and(is_ident_continue) {
                    let _ = cursor.bump();
                }

                push(out, start..cursor.offset, Scope::Attribute, theme);
            } else {
                push(out, start..cursor.offset, Scope::Punctuation, theme);
            }

            continue;
        }

        if is_ident_start(next) {
            while cursor.peek().is_some_and(is_ident_continue) {
                let _ = cursor.bump();
            }

            let word = &line[start..cursor.offset];

            let scope = if syntax.is_keyword(word) {
                Scope::Keyword
            } else if syntax.is_type(word) {
                Scope::Type
            } else if cursor.peek() == Some('(') {
                Scope::Function
            } else {
                Scope::Text
            };

            push(out, start..cursor.offset, scope, theme);
            continue;
        }

        let _ = cursor.bump();
        push(out, start..cursor.offset, Scope::Punctuation, theme);
    }

    carry
}

/// Consumes a block comment body, returning the state for the next line.
fn continue_block_comment(
    syntax: &Syntax,
    cursor: &mut Cursor<'_>,
    depth: u32,
    out: &mut Vec<(Range<usize>, Highlight)>,
    theme: Theme,
) -> Carry {
    let Some((open, close)) = syntax.block_comment else {
        return Carry::Ready;
    };

    let start = cursor.offset;
    let mut depth = depth;

    while !cursor.is_done() {
        if cursor.eat(close) {
            depth -= 1;

            if depth == 0 {
                break;
            }
        } else if syntax.nested_block_comments && cursor.eat(open) {
            depth += 1;
        } else {
            let _ = cursor.bump();
        }
    }

    if cursor.offset > start {
        push(out, start..cursor.offset, Scope::Comment, theme);
    }

    if depth == 0 {
        Carry::Ready
    } else {
        Carry::BlockComment(depth)
    }
}

/// Consumes a multi-line string body, returning the state for the next line.
fn continue_multiline_string(
    syntax: &Syntax,
    cursor: &mut Cursor<'_>,
    index: usize,
    out: &mut Vec<(Range<usize>, Highlight)>,
    theme: Theme,
) -> Carry {
    let Some(delimiter) = syntax.multiline_strings.get(index) else {
        return Carry::Ready;
    };

    let start = cursor.offset;
    let mut closed = false;

    while !cursor.is_done() {
        if syntax
            .escape
            .is_some_and(|escape| cursor.peek() == Some(escape))
        {
            let _ = cursor.bump();
            let _ = cursor.bump();
            continue;
        }

        if cursor.eat(delimiter) {
            closed = true;
            break;
        }

        let _ = cursor.bump();
    }

    if cursor.offset > start {
        push(out, start..cursor.offset, Scope::String, theme);
    }

    if closed {
        Carry::Ready
    } else {
        Carry::MultilineString(index)
    }
}

/// Consumes the body of a string that cannot span lines.
fn scan_single_line_string(syntax: &Syntax, cursor: &mut Cursor<'_>, delimiter: char) {
    while let Some(next) = cursor.peek() {
        if syntax.escape == Some(next) {
            let _ = cursor.bump();
            let _ = cursor.bump();
            continue;
        }

        let _ = cursor.bump();

        if next == delimiter {
            break;
        }
    }
}

/// Consumes a numeric literal, including a radix prefix, digit
/// separators, a fractional part, and an exponent or type suffix.
fn scan_number(cursor: &mut Cursor<'_>) {
    let _ = cursor.bump();

    if matches!(cursor.peek(), Some('x' | 'X' | 'b' | 'B' | 'o' | 'O')) {
        let _ = cursor.bump();
    }

    let mut seen_dot = false;

    while let Some(next) = cursor.peek() {
        match next {
            '.' if !seen_dot => {
                // `1..2` is a range, not a float.
                if cursor.line[cursor.offset..].starts_with("..") {
                    break;
                }

                seen_dot = true;
                let _ = cursor.bump();
            }
            '+' | '-' => break,
            _ if is_ident_continue(next) => {
                let _ = cursor.bump();
            }
            _ => break,
        }
    }
}

/// Appends a span, merging it into the previous one when the scope matches.
fn push(out: &mut Vec<(Range<usize>, Highlight)>, range: Range<usize>, scope: Scope, theme: Theme) {
    if range.is_empty() {
        return;
    }

    if let Some((last, highlight)) = out.last_mut()
        && highlight.scope == scope
        && last.end == range.start
    {
        last.end = range.end;
        return;
    }

    out.push((range, Highlight { scope, theme }));
}

/// Extends the last span backwards to `start`, absorbing an opening
/// delimiter that was consumed before the body was scanned.
fn merge_back(out: &mut [(Range<usize>, Highlight)], start: usize, scope: Scope) {
    if let Some((range, highlight)) = out.last_mut()
        && highlight.scope == scope
        && range.start > start
    {
        range.start = start;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scopes(language: &str, line: &str) -> Vec<(String, Scope)> {
        let mut stream = Stream::new(&Settings::new(Theme::Dark, language));

        stream
            .highlight_line(line)
            .map(|(range, highlight)| (line[range].to_owned(), highlight.scope()))
            .collect()
    }

    fn kinds(language: &str, line: &str) -> Vec<Scope> {
        scopes(language, line)
            .into_iter()
            .filter(|(text, _)| !text.trim().is_empty())
            .map(|(_, scope)| scope)
            .collect()
    }

    /// The invariant every consumer relies on: ranges are usable as
    /// `&line[range]` and lose nothing.
    fn assert_spans_tile(language: &str, lines: &[&str]) {
        let mut stream = Stream::new(&Settings::new(Theme::Dark, language));

        for line in lines {
            let spans: Vec<_> = stream.highlight_line(line).collect();
            let mut end = 0;

            for (range, _) in &spans {
                assert_eq!(range.start, end, "gap or overlap in {line:?}");
                assert!(!range.is_empty(), "empty span in {line:?}");
                assert!(line.is_char_boundary(range.start), "split char in {line:?}");
                assert!(line.is_char_boundary(range.end), "split char in {line:?}");
                end = range.end;
            }

            assert_eq!(end, line.len(), "spans do not cover {line:?}");

            stream.commit();
        }
    }

    #[test]
    fn classifies_a_rust_line() {
        assert_eq!(
            kinds("rust", "let n: u32 = compute(1_000); // done"),
            vec![
                Scope::Keyword,     // let
                Scope::Text,        // n
                Scope::Punctuation, // :
                Scope::Type,        // u32
                Scope::Punctuation, // =
                Scope::Function,    // compute
                Scope::Punctuation, // (
                Scope::Number,      // 1_000
                Scope::Punctuation, // );
                Scope::Comment,     // // done
            ]
        );
    }

    #[test]
    fn strings_swallow_delimiters_and_escapes() {
        assert_eq!(
            scopes("rust", r#"let s = "a \" b"; "#)
                .into_iter()
                .find(|(_, scope)| *scope == Scope::String),
            Some((r#""a \" b""#.to_owned(), Scope::String))
        );
    }

    #[test]
    fn an_unterminated_string_stops_at_the_end_of_the_line() {
        let spans = scopes("rust", "let s = \"oops");

        assert_eq!(spans.last(), Some(&("\"oops".to_owned(), Scope::String)));
    }

    #[test]
    fn block_comments_span_lines() {
        let mut stream = Stream::new(&Settings::new(Theme::Dark, "rust"));

        let first: Vec<_> = stream.highlight_line("code /* open").collect();
        assert_eq!(first.last().unwrap().1.scope(), Scope::Comment);
        assert!(stream.is_inside_multiline());
        stream.commit();

        let second: Vec<_> = stream.highlight_line("still comment").collect();
        assert!(second.iter().all(|(_, h)| h.scope() == Scope::Comment));
        stream.commit();

        let third: Vec<_> = stream.highlight_line("*/ let x = 1;").collect();
        assert_eq!(third[0].1.scope(), Scope::Comment);
        assert!(third.iter().any(|(_, h)| h.scope() == Scope::Keyword));
        assert!(!stream.is_inside_multiline());
    }

    #[test]
    fn rust_block_comments_nest() {
        let mut stream = Stream::new(&Settings::new(Theme::Dark, "rust"));

        let _ = stream.highlight_line("/* outer /* inner").count();
        stream.commit();

        let _ = stream.highlight_line("*/ still a comment").count();
        assert!(
            stream.is_inside_multiline(),
            "closing the inner comment must not close the outer one"
        );
        stream.commit();

        let _ = stream.highlight_line("*/").count();
        assert!(!stream.is_inside_multiline());
    }

    #[test]
    fn c_block_comments_do_not_nest() {
        let mut stream = Stream::new(&Settings::new(Theme::Dark, "c"));

        let _ = stream.highlight_line("/* outer /* inner").count();
        stream.commit();

        let _ = stream.highlight_line("*/ done").count();
        assert!(!stream.is_inside_multiline());
    }

    #[test]
    fn python_triple_quotes_span_lines() {
        let mut stream = Stream::new(&Settings::new(Theme::Dark, "python"));

        let _ = stream.highlight_line("doc = \"\"\"start").count();
        assert!(stream.is_inside_multiline());
        stream.commit();

        let _ = stream.highlight_line("end\"\"\"").count();
        assert!(!stream.is_inside_multiline());
    }

    #[test]
    fn highlight_line_is_idempotent_without_commit() {
        let mut stream = Stream::new(&Settings::new(Theme::Dark, "rust"));

        let first: Vec<_> = stream.highlight_line("/* open").collect();
        let second: Vec<_> = stream.highlight_line("/* open").collect();

        assert_eq!(first, second);
    }

    #[test]
    fn reset_returns_to_the_start_of_the_document() {
        let mut stream = Stream::new(&Settings::new(Theme::Dark, "rust"));

        let _ = stream.highlight_line("/* open").count();
        stream.commit();
        assert!(stream.is_inside_multiline());

        stream.reset();

        let after: Vec<_> = stream.highlight_line("let x = 1;").collect();
        assert_eq!(after[0].1.scope(), Scope::Keyword);
    }

    #[test]
    fn multibyte_text_is_never_split() {
        assert_spans_tile(
            "rust",
            &["let नमस्ते = \"日本語 🎉\"; // टिप्पणी", "/* بلوك", "تعليق */"],
        );
    }

    #[test]
    fn spans_tile_every_supported_language() {
        let sample = [
            "value = 1.5e3 // trailing",
            "\"quoted\" 'other' `back`",
            "# hash --dash <tag attr=\"x\"/>",
            "/* block */ SELECT * FROM t;",
            "",
            "   ",
        ];

        for syntax in syntax::ALL {
            assert_spans_tile(syntax.names[0], &sample);
        }
    }

    #[test]
    fn an_unknown_language_still_tiles_and_finds_numbers() {
        assert_spans_tile("brainfuck", &["+++ 42 [>+<-]"]);

        assert!(kinds("brainfuck", "+++ 42").contains(&Scope::Number));
    }

    #[test]
    fn ranges_are_not_a_range_operator() {
        assert_eq!(
            kinds("rust", "0..10"),
            vec![Scope::Number, Scope::Punctuation, Scope::Number]
        );
    }

    #[test]
    fn attributes_need_a_word() {
        assert_eq!(
            scopes("rust", "#[derive]"),
            vec![
                ("#[derive".to_owned(), Scope::Attribute),
                ("]".to_owned(), Scope::Punctuation),
            ]
        );

        // A bare sigil with nothing to name is just punctuation.
        assert_eq!(
            kinds("rust", "a # b"),
            vec![Scope::Text, Scope::Punctuation, Scope::Text]
        );
    }

    #[test]
    fn comments_are_italic_and_everything_else_is_not() {
        let highlight = |scope| Highlight {
            scope,
            theme: Theme::Dark,
        };

        assert_eq!(
            highlight(Scope::Comment).font().map(|font| font.style),
            Some(font::Style::Italic)
        );

        for scope in Scope::ALL.iter().filter(|s| **s != Scope::Comment) {
            assert_eq!(highlight(*scope).font(), None, "{scope:?} must not restyle");
        }
    }

    #[test]
    fn the_highlighter_adapter_tracks_lines() {
        use aura_core::text::Highlighter as _;

        let mut highlighter = Highlighter::new(&Settings::new(Theme::Dark, "rust"));

        assert_eq!(highlighter.current_line(), 0);

        let _ = highlighter.highlight_line("/* open").count();
        assert_eq!(highlighter.current_line(), 1);

        // The carry survives, because the adapter commits each line.
        let spans: Vec<_> = highlighter.highlight_line("still comment").collect();
        assert!(spans.iter().all(|(_, h)| h.scope() == Scope::Comment));

        highlighter.change_line(0);
        assert_eq!(highlighter.current_line(), 0);

        let spans: Vec<_> = highlighter.highlight_line("let x = 1;").collect();
        assert_eq!(spans[0].1.scope(), Scope::Keyword);
    }

    #[test]
    fn updating_settings_switches_language() {
        let mut stream = Stream::new(&Settings::new(Theme::Dark, "rust"));
        assert_eq!(stream.syntax().names[0], "rust");

        stream.update(&Settings::new(Theme::Light, "python"));

        assert_eq!(stream.syntax().names[0], "python");
        assert_eq!(stream.theme(), Theme::Light);
        assert_eq!(kinds("python", "def f(): pass")[0], Scope::Keyword);
    }
}
