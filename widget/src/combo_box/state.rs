//! Search indexing and retained option state for combo boxes.

use std::cell::RefCell;
use std::fmt::Display;

/// The local state of a [`ComboBox`].
#[derive(Debug, Clone)]
pub struct State<T> {
    pub(super) options: Vec<T>,
    inner: RefCell<Inner<T>>,
}

#[derive(Debug, Clone)]
pub(super) struct Inner<T> {
    pub(super) value: String,
    pub(super) option_matchers: Vec<String>,
    pub(super) filtered_options: Filtered<T>,
}

#[derive(Debug, Clone)]
pub(super) struct Filtered<T> {
    pub(super) options: Vec<T>,
    revision: u64,
}

impl<T> State<T>
where
    T: Display + Clone,
{
    /// Creates a new [`State`] for a [`ComboBox`] with the given list of options.
    pub fn new(options: Vec<T>) -> Self {
        Self::with_selection(options, None)
    }

    /// Creates a new [`State`] for a [`ComboBox`] with the given list of options
    /// and selected value.
    pub fn with_selection(options: Vec<T>, selection: Option<&T>) -> Self {
        let value = selection.map(T::to_string).unwrap_or_default();

        // Pre-build "matcher" strings ahead of time so that search is fast
        let option_matchers = build_matchers(&options);

        let filtered_options = Filtered::new(
            search(&options, &option_matchers, &value)
                .cloned()
                .collect(),
        );

        Self {
            options,
            inner: RefCell::new(Inner {
                value,
                option_matchers,
                filtered_options,
            }),
        }
    }

    /// Returns the options of the [`State`].
    ///
    /// These are the options provided when the [`State`]
    /// was constructed with [`State::new`].
    pub fn options(&self) -> &[T] {
        &self.options
    }

    /// Pushes a new option to the [`State`].
    pub fn push(&mut self, new_option: T) {
        let mut inner = self.inner.borrow_mut();

        inner.option_matchers.push(build_matcher(&new_option));
        self.options.push(new_option);

        inner.filtered_options = Filtered::new(
            search(&self.options, &inner.option_matchers, &inner.value)
                .cloned()
                .collect(),
        );
    }

    /// Returns ownership of the options of the [`State`].
    pub fn into_options(self) -> Vec<T> {
        self.options
    }

    pub(super) fn value(&self) -> String {
        let inner = self.inner.borrow();

        inner.value.clone()
    }

    pub(super) fn with_inner<O>(&self, f: impl FnOnce(&Inner<T>) -> O) -> O {
        let inner = self.inner.borrow();

        f(&inner)
    }

    pub(super) fn with_inner_mut(&self, f: impl FnOnce(&mut Inner<T>)) {
        let mut inner = self.inner.borrow_mut();

        f(&mut inner);
    }

    pub(super) fn sync_filtered_options(&self, options: &mut Filtered<T>) {
        let inner = self.inner.borrow();

        inner.filtered_options.sync(options);
    }
}

impl<T> Default for State<T>
where
    T: Display + Clone,
{
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl<T> Filtered<T>
where
    T: Clone,
{
    fn new(options: Vec<T>) -> Self {
        Self {
            options,
            revision: 1,
        }
    }

    pub(super) fn empty() -> Self {
        Self {
            options: vec![],
            revision: 0,
        }
    }

    pub(super) fn update(&mut self, options: Vec<T>) {
        self.options = options;
        self.revision = self.revision.wrapping_add(1).max(1);
    }

    fn sync(&self, other: &mut Filtered<T>) {
        if other.revision != self.revision {
            *other = self.clone();
        }
    }
}

pub(super) fn search<'a, T, A>(
    options: impl IntoIterator<Item = T> + 'a,
    option_matchers: impl IntoIterator<Item = &'a A> + 'a,
    query: &'a str,
) -> impl Iterator<Item = T> + 'a
where
    A: AsRef<str> + 'a,
{
    let mut normalized = String::with_capacity(query.len());
    let mut separator = true;
    for character in query.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            separator = false;
        } else if !separator {
            normalized.push(' ');
            separator = true;
        }
    }

    options
        .into_iter()
        .zip(option_matchers)
        // Make sure each part of the query is found in the option
        .filter_map(move |(option, matcher)| {
            if normalized
                .split_ascii_whitespace()
                .all(|part| matcher.as_ref().contains(part))
            {
                Some(option)
            } else {
                None
            }
        })
}

fn build_matchers<'a, T>(options: impl IntoIterator<Item = T> + 'a) -> Vec<String>
where
    T: Display + 'a,
{
    options.into_iter().map(build_matcher).collect()
}

fn build_matcher<T>(option: T) -> String
where
    T: Display,
{
    let mut matcher = option.to_string();
    matcher.retain(|c| c.is_ascii_alphanumeric());
    matcher.make_ascii_lowercase();
    matcher
}
