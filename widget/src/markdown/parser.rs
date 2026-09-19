use super::model::Span;
use super::*;

/// A bunch of Markdown that has been parsed.
#[derive(Debug, Default)]
pub struct Content {
    items: Vec<Item>,
    incomplete: FxHashMap<usize, Section>,
    state: State,
}

#[derive(Debug)]
struct Section {
    content: String,
    broken_links: FxHashSet<String>,
}

impl Content {
    /// Creates a new empty [`Content`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates some new [`Content`] by parsing the given Markdown.
    pub fn parse(markdown: &str) -> Self {
        let mut content = Self::new();
        content.push_str(markdown);
        content
    }

    /// Pushes more Markdown into the [`Content`]; parsing incrementally!
    ///
    /// This is specially useful when you have long streams of Markdown; like
    /// big files or potentially long replies.
    pub fn push_str(&mut self, markdown: &str) {
        if markdown.is_empty() {
            return;
        }

        // Append to last leftover text
        let mut leftover = std::mem::take(&mut self.state.leftover);
        leftover.push_str(markdown);

        let input = if leftover.trim_end().ends_with('|') {
            leftover.trim_end().trim_end_matches('|')
        } else {
            leftover.as_str()
        };

        // Pop the last item
        let _ = self.items.pop();

        // Re-parse last item and new text
        for (item, source, broken_links) in parse_with(&mut self.state, input) {
            if !broken_links.is_empty() {
                let _ = self.incomplete.insert(
                    self.items.len(),
                    Section {
                        content: source.to_owned(),
                        broken_links,
                    },
                );
            }

            self.items.push(item);
        }

        self.state.leftover.push_str(&leftover[input.len()..]);

        // Re-parse incomplete sections if new references are available
        if !self.incomplete.is_empty() {
            self.incomplete.retain(|index, section| {
                if self.items.len() <= *index {
                    return false;
                }

                let broken_links_before = section.broken_links.len();

                section
                    .broken_links
                    .retain(|link| !self.state.references.contains_key(link));

                if broken_links_before != section.broken_links.len() {
                    let mut state = State {
                        leftover: String::new(),
                        references: self.state.references.clone(),
                        images: HashSet::new(),
                        #[cfg(feature = "highlighter")]
                        highlighter: None,
                    };

                    if let Some((item, _source, _broken_links)) =
                        parse_with(&mut state, &section.content).next()
                    {
                        self.items[*index] = item;
                    }

                    self.state.images.extend(state.images.drain());
                    drop(state);
                }

                !section.broken_links.is_empty()
            });
        }
    }

    /// Returns the Markdown items, ready to be rendered.
    ///
    /// You can use [`view`] to turn them into an [`Element`].
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Returns the URLs of the Markdown images present in the [`Content`].
    pub fn images(&self) -> &HashSet<Uri> {
        &self.state.images
    }
}

/// Parse the given Markdown content.
///
/// # Example
/// ```no_run
/// # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
/// # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
/// #
/// use taim::widget::markdown;
/// use taim::Theme;
///
/// struct State {
///    markdown: Vec<markdown::Item>,
/// }
///
/// enum Message {
///     LinkClicked(markdown::Uri),
/// }
///
/// impl State {
///     pub fn new() -> Self {
///         Self {
///             markdown: markdown::parse("This is some **Markdown**!").collect(),
///         }
///     }
///
///     fn view(&self) -> Element<'_, Message> {
///         markdown::view(&self.markdown, Theme::Dark)
///             .map(Message::LinkClicked)
///             .into()
///     }
///
///     fn update(state: &mut State, message: Message) {
///         match message {
///             Message::LinkClicked(url) => {
///                 println!("The following url was clicked: {url}");
///             }
///         }
///     }
/// }
/// ```
pub fn parse(markdown: &str) -> impl Iterator<Item = Item> + '_ {
    parse_with(State::default(), markdown).map(|(item, _source, _broken_links)| item)
}

#[derive(Debug, Default)]
struct State {
    leftover: String,
    references: FxHashMap<String, String>,
    images: HashSet<Uri>,
    #[cfg(feature = "highlighter")]
    highlighter: Option<Highlighter>,
}

#[cfg(feature = "highlighter")]
#[derive(Debug)]
struct Highlighter {
    lines: Vec<(String, Vec<Span>)>,
    language: String,
    parser: taim_highlighter::Stream,
    current: usize,
}

#[cfg(feature = "highlighter")]
impl Highlighter {
    pub fn new(language: &str) -> Self {
        Self {
            lines: Vec::new(),
            parser: taim_highlighter::Stream::new(&taim_highlighter::Settings {
                theme: taim_highlighter::Theme::Dark,
                token: language.to_owned(),
            }),
            language: language.to_owned(),
            current: 0,
        }
    }

    pub fn prepare(&mut self) {
        self.current = 0;
    }

    pub fn highlight_line(&mut self, text: &str) -> &[Span] {
        match self.lines.get(self.current) {
            Some(line) if line.0 == text => {}
            _ => {
                if self.current + 1 < self.lines.len() {
                    log::debug!("Resetting highlighter...");
                    self.parser.reset();
                    self.lines.truncate(self.current);

                    for line in &self.lines {
                        log::debug!("Refeeding {n} lines", n = self.lines.len());

                        let _ = self.parser.highlight_line(&line.0);
                    }
                }

                log::trace!("Parsing: {text}", text = text.trim_end());

                if self.current + 1 < self.lines.len() {
                    self.parser.commit();
                }

                let mut spans = Vec::new();

                for (range, highlight) in self.parser.highlight_line(text) {
                    spans.push(Span::Highlight {
                        text: text[range].to_owned(),
                        color: highlight.color(),
                        font: highlight.font(),
                    });
                }

                if self.current + 1 == self.lines.len() {
                    let _ = self.lines.pop();
                }

                self.lines.push((text.to_owned(), spans));
            }
        }

        self.current += 1;

        &self
            .lines
            .get(self.current - 1)
            .expect("Line must be parsed")
            .1
    }
}

fn parse_with<'a>(
    mut state: impl BorrowMut<State> + 'a,
    markdown: &'a str,
) -> impl Iterator<Item = (Item, &'a str, FxHashSet<String>)> + 'a {
    enum Scope {
        List(List),
        Quote(Vec<Item>),
        Table {
            alignment: Vec<pulldown_cmark::Alignment>,
            columns: Vec<Column>,
            rows: Vec<Row>,
            current: Vec<Item>,
        },
    }

    struct List {
        start: Option<u64>,
        bullets: Vec<Bullet>,
    }

    let broken_links = Rc::new(RefCell::new(FxHashSet::default()));

    let mut spans = Vec::new();
    let mut code = String::new();
    let mut code_language = None;
    let mut code_lines = Vec::new();
    let mut strong = false;
    let mut emphasis = false;
    let mut strikethrough = false;
    let mut metadata = false;
    let mut code_block = false;
    let mut link = None;
    let mut image = None;
    let mut stack = Vec::new();

    #[cfg(feature = "highlighter")]
    let mut highlighter = None;

    let parser = pulldown_cmark::Parser::new_with_broken_link_callback(
        markdown,
        pulldown_cmark::Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
            | pulldown_cmark::Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS
            | pulldown_cmark::Options::ENABLE_TABLES
            | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_TASKLISTS,
        {
            let references = state.borrow().references.clone();
            let broken_links = broken_links.clone();

            Some(move |broken_link: pulldown_cmark::BrokenLink<'_>| {
                if let Some(reference) = references.get(broken_link.reference.as_ref()) {
                    Some((
                        pulldown_cmark::CowStr::from(reference.to_owned()),
                        broken_link.reference.into_static(),
                    ))
                } else {
                    let _ = RefCell::borrow_mut(&broken_links)
                        .insert(broken_link.reference.into_string());

                    None
                }
            })
        },
    );

    let references = &mut state.borrow_mut().references;

    for reference in parser.reference_definitions().iter() {
        let _ = references.insert(reference.0.to_owned(), reference.1.dest.to_string());
    }

    let produce = move |state: &mut State, stack: &mut Vec<Scope>, item, source: Range<usize>| {
        if let Some(scope) = stack.last_mut() {
            match scope {
                Scope::List(list) => {
                    list.bullets.last_mut().expect("item context").push(item);
                }
                Scope::Quote(items) => {
                    items.push(item);
                }
                Scope::Table { current, .. } => {
                    current.push(item);
                }
            }

            None
        } else {
            state.leftover = markdown[source.start..].to_owned();

            Some((
                item,
                &markdown[source.start..source.end],
                broken_links.take(),
            ))
        }
    };

    let parser = parser.into_offset_iter();

    // We want to keep the `spans` capacity
    #[allow(clippy::drain_collect)]
    parser.filter_map(move |(event, source)| match event {
        pulldown_cmark::Event::Start(tag) => match tag {
            pulldown_cmark::Tag::Strong if !metadata => {
                strong = true;
                None
            }
            pulldown_cmark::Tag::Emphasis if !metadata => {
                emphasis = true;
                None
            }
            pulldown_cmark::Tag::Strikethrough if !metadata => {
                strikethrough = true;
                None
            }
            pulldown_cmark::Tag::Link { dest_url, .. } if !metadata => {
                link = Some(dest_url.into_string());
                None
            }
            pulldown_cmark::Tag::Image {
                dest_url, title, ..
            } if !metadata => {
                image = Some((dest_url.into_string(), title.into_string()));
                None
            }
            pulldown_cmark::Tag::List(first_item) if !metadata => {
                let prev = if spans.is_empty() {
                    None
                } else {
                    produce(
                        state.borrow_mut(),
                        &mut stack,
                        Item::Paragraph(Text::new(spans.drain(..).collect())),
                        source,
                    )
                };

                stack.push(Scope::List(List {
                    start: first_item,
                    bullets: Vec::new(),
                }));

                prev
            }
            pulldown_cmark::Tag::Item => {
                if let Some(Scope::List(list)) = stack.last_mut() {
                    list.bullets.push(Bullet::Point { items: Vec::new() });
                }

                None
            }
            pulldown_cmark::Tag::BlockQuote(_kind) if !metadata => {
                let prev = if spans.is_empty() {
                    None
                } else {
                    produce(
                        state.borrow_mut(),
                        &mut stack,
                        Item::Paragraph(Text::new(spans.drain(..).collect())),
                        source,
                    )
                };

                stack.push(Scope::Quote(Vec::new()));

                prev
            }
            pulldown_cmark::Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Fenced(language))
                if !metadata =>
            {
                #[cfg(feature = "highlighter")]
                {
                    highlighter = Some({
                        let mut highlighter = state
                            .borrow_mut()
                            .highlighter
                            .take()
                            .filter(|highlighter| highlighter.language == language.as_ref())
                            .unwrap_or_else(|| {
                                Highlighter::new(language.split(',').next().unwrap_or_default())
                            });

                        highlighter.prepare();

                        highlighter
                    });
                }

                code_block = true;
                code_language = (!language.is_empty()).then(|| language.into_string());

                if spans.is_empty() {
                    None
                } else {
                    produce(
                        state.borrow_mut(),
                        &mut stack,
                        Item::Paragraph(Text::new(spans.drain(..).collect())),
                        source,
                    )
                }
            }
            pulldown_cmark::Tag::MetadataBlock(_) => {
                metadata = true;
                None
            }
            pulldown_cmark::Tag::Table(alignment) => {
                stack.push(Scope::Table {
                    columns: Vec::with_capacity(alignment.len()),
                    alignment,
                    current: Vec::new(),
                    rows: Vec::new(),
                });

                None
            }
            pulldown_cmark::Tag::TableHead => {
                strong = true;
                None
            }
            pulldown_cmark::Tag::TableRow => {
                let Scope::Table { rows, .. } = stack.last_mut()? else {
                    return None;
                };

                rows.push(Row { cells: Vec::new() });
                None
            }
            _ => None,
        },
        pulldown_cmark::Event::End(tag) => match tag {
            pulldown_cmark::TagEnd::Heading(level) if !metadata => produce(
                state.borrow_mut(),
                &mut stack,
                Item::Heading(level, Text::new(spans.drain(..).collect())),
                source,
            ),
            pulldown_cmark::TagEnd::Strong if !metadata => {
                strong = false;
                None
            }
            pulldown_cmark::TagEnd::Emphasis if !metadata => {
                emphasis = false;
                None
            }
            pulldown_cmark::TagEnd::Strikethrough if !metadata => {
                strikethrough = false;
                None
            }
            pulldown_cmark::TagEnd::Link if !metadata => {
                link = None;
                None
            }
            pulldown_cmark::TagEnd::Paragraph if !metadata => {
                if spans.is_empty() {
                    None
                } else {
                    produce(
                        state.borrow_mut(),
                        &mut stack,
                        Item::Paragraph(Text::new(spans.drain(..).collect())),
                        source,
                    )
                }
            }
            pulldown_cmark::TagEnd::Item if !metadata => {
                if spans.is_empty() {
                    None
                } else {
                    produce(
                        state.borrow_mut(),
                        &mut stack,
                        Item::Paragraph(Text::new(spans.drain(..).collect())),
                        source,
                    )
                }
            }
            pulldown_cmark::TagEnd::List(_) if !metadata => {
                let scope = stack.pop()?;

                let Scope::List(list) = scope else {
                    return None;
                };

                produce(
                    state.borrow_mut(),
                    &mut stack,
                    Item::List {
                        start: list.start,
                        bullets: list.bullets,
                    },
                    source,
                )
            }
            pulldown_cmark::TagEnd::BlockQuote(_kind) if !metadata => {
                let scope = stack.pop()?;

                let Scope::Quote(quote) = scope else {
                    return None;
                };

                produce(state.borrow_mut(), &mut stack, Item::Quote(quote), source)
            }
            pulldown_cmark::TagEnd::Image if !metadata => {
                let (url, title) = image.take()?;
                let alt = Text::new(spans.drain(..).collect());

                let state = state.borrow_mut();
                let _ = state.images.insert(url.clone());

                produce(state, &mut stack, Item::Image { url, title, alt }, source)
            }
            pulldown_cmark::TagEnd::CodeBlock if !metadata => {
                code_block = false;

                #[cfg(feature = "highlighter")]
                {
                    state.borrow_mut().highlighter = highlighter.take();
                }

                produce(
                    state.borrow_mut(),
                    &mut stack,
                    Item::CodeBlock {
                        language: code_language.take(),
                        code: mem::take(&mut code),
                        lines: code_lines.drain(..).collect(),
                    },
                    source,
                )
            }
            pulldown_cmark::TagEnd::MetadataBlock(_) => {
                metadata = false;
                None
            }
            pulldown_cmark::TagEnd::Table => {
                let scope = stack.pop()?;

                let Scope::Table { columns, rows, .. } = scope else {
                    return None;
                };

                produce(
                    state.borrow_mut(),
                    &mut stack,
                    Item::Table { columns, rows },
                    source,
                )
            }
            pulldown_cmark::TagEnd::TableHead => {
                strong = false;
                None
            }
            pulldown_cmark::TagEnd::TableCell => {
                if !spans.is_empty() {
                    let _ = produce(
                        state.borrow_mut(),
                        &mut stack,
                        Item::Paragraph(Text::new(spans.drain(..).collect())),
                        source,
                    );
                }

                let Scope::Table {
                    alignment,
                    columns,
                    rows,
                    current,
                } = stack.last_mut()?
                else {
                    return None;
                };

                if columns.len() < alignment.len() {
                    columns.push(Column {
                        header: std::mem::take(current),
                        alignment: alignment[columns.len()],
                    });
                } else {
                    rows.last_mut()
                        .expect("table row")
                        .cells
                        .push(std::mem::take(current));
                }

                None
            }
            _ => None,
        },
        pulldown_cmark::Event::Text(text) if !metadata => {
            if code_block {
                code.push_str(&text);

                #[cfg(feature = "highlighter")]
                if let Some(highlighter) = &mut highlighter {
                    for line in text.lines() {
                        code_lines.push(Text::new(highlighter.highlight_line(line).to_vec()));
                    }
                }

                #[cfg(not(feature = "highlighter"))]
                for line in text.lines() {
                    code_lines.push(Text::new(vec![Span::Standard {
                        text: line.to_owned(),
                        strong,
                        emphasis,
                        strikethrough,
                        link: link.clone(),
                        code: false,
                    }]));
                }

                return None;
            }

            let span = Span::Standard {
                text: text.into_string(),
                strong,
                emphasis,
                strikethrough,
                link: link.clone(),
                code: false,
            };

            spans.push(span);

            None
        }
        pulldown_cmark::Event::Code(code) if !metadata => {
            let span = Span::Standard {
                text: code.into_string(),
                strong,
                emphasis,
                strikethrough,
                link: link.clone(),
                code: true,
            };

            spans.push(span);
            None
        }
        pulldown_cmark::Event::SoftBreak if !metadata => {
            spans.push(Span::Standard {
                text: String::from(" "),
                strikethrough,
                strong,
                emphasis,
                link: link.clone(),
                code: false,
            });
            None
        }
        pulldown_cmark::Event::HardBreak if !metadata => {
            spans.push(Span::Standard {
                text: String::from("\n"),
                strikethrough,
                strong,
                emphasis,
                link: link.clone(),
                code: false,
            });
            None
        }
        pulldown_cmark::Event::Rule => produce(state.borrow_mut(), &mut stack, Item::Rule, source),
        pulldown_cmark::Event::TaskListMarker(done) => {
            if let Some(Scope::List(list)) = stack.last_mut()
                && let Some(item) = list.bullets.last_mut()
                && let Bullet::Point { items } = item
            {
                *item = Bullet::Task {
                    items: std::mem::take(items),
                    done,
                };
            }

            None
        }
        _ => None,
    })
}
