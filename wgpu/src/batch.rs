//! Ordering-preserving grouping of consecutive same-key items into runs.
//!
//! [`Batchable`]/[`group`] formalize the grouping [`crate::quad::Batch`]
//! already does ad-hoc (via its own `order: Vec<(Kind, usize)>` field):
//! consecutive items sharing the same [`Batchable::Key`] (a pipeline,
//! clip region, or z-order-compatible identity -- whatever makes two
//! items renderable in a single draw call together) collapse into one
//! run, without ever reordering items across a run boundary. Reordering
//! would be an optimization with a correctness cost (two overlapping,
//! differently-ordered quads paint different pixels), so [`group`]
//! deliberately does no sorting -- it only merges what is *already*
//! adjacent and matching, exactly mirroring the batching every existing
//! draw path in this crate already relies on being correct.
//!
//! `tests/gpu/batching.rs` is the ordering-correctness oracle this
//! module exists to satisfy: it proves that grouping same-key items
//! together, then rendering each run in one pass, produces the exact
//! same visible result (same z-order, same blending) as rendering every
//! item as its own individual draw call in original order.

/// Something that can be grouped into a batch alongside other items
/// sharing the same [`Batchable::Key`].
pub trait Batchable {
    /// The key two items must share to be grouped into the same run.
    /// Typically a pipeline/material identity (e.g.
    /// [`crate::quad::Kind`]); items with a different key always start
    /// a new run, even if adjacent.
    type Key: PartialEq;

    /// Returns this item's batching key.
    fn batch_key(&self) -> Self::Key;
}

/// A single run of consecutive items sharing one [`Batchable::Key`],
/// expressed as a half-open range of indices into the original slice --
/// mirroring [`crate::quad::Batch`]'s existing `(Kind, count)` encoding,
/// generalized to any [`Batchable::Key`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run<K> {
    /// The key shared by every item in this run.
    pub key: K,
    /// The start index (inclusive) of this run in the original slice.
    pub start: usize,
    /// The end index (exclusive) of this run in the original slice.
    pub end: usize,
}

impl<K> Run<K> {
    /// The number of items in this run.
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Returns whether this run is empty (`start == end`). A [`Run`]
    /// produced by [`group`] is never empty; this exists for
    /// completeness and symmetry with [`Iterator::len`]-style APIs.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Groups consecutive items sharing the same [`Batchable::Key`] into
/// [`Run`]s, preserving the original order of `items` exactly -- the
/// returned runs, concatenated, cover `0..items.len()` with no gaps, no
/// overlaps, and no reordering.
pub fn group<T: Batchable>(items: &[T]) -> Vec<Run<T::Key>> {
    let mut runs: Vec<Run<T::Key>> = Vec::new();

    for (index, item) in items.iter().enumerate() {
        let key = item.batch_key();

        match runs.last_mut() {
            Some(run) if run.key == key => {
                run.end = index + 1;
            }
            _ => {
                runs.push(Run {
                    key,
                    start: index,
                    end: index + 1,
                });
            }
        }
    }

    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Item(&'static str);

    impl Batchable for Item {
        type Key = &'static str;

        fn batch_key(&self) -> Self::Key {
            self.0
        }
    }

    #[test]
    fn an_empty_slice_produces_no_runs() {
        let items: Vec<Item> = Vec::new();

        assert_eq!(group(&items), Vec::new());
    }

    #[test]
    fn a_single_item_produces_a_single_run() {
        let items = [Item("a")];

        assert_eq!(
            group(&items),
            vec![Run {
                key: "a",
                start: 0,
                end: 1
            }]
        );
    }

    #[test]
    fn consecutive_same_key_items_merge_into_one_run() {
        let items = [Item("a"), Item("a"), Item("a")];

        let runs = group(&items);

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].key, "a");
        assert_eq!(runs[0].len(), 3);
    }

    #[test]
    fn alternating_keys_produce_one_run_each_in_original_order() {
        let items = [Item("a"), Item("b"), Item("a")];

        let runs = group(&items);

        assert_eq!(
            runs,
            vec![
                Run {
                    key: "a",
                    start: 0,
                    end: 1
                },
                Run {
                    key: "b",
                    start: 1,
                    end: 2
                },
                Run {
                    key: "a",
                    start: 2,
                    end: 3
                },
            ],
            "a key repeated non-adjacently must NOT merge with its earlier \
             occurrence -- that would reorder items across the 'b' in between"
        );
    }

    #[test]
    fn runs_cover_the_whole_slice_with_no_gaps_or_overlaps() {
        let items = [
            Item("a"),
            Item("a"),
            Item("b"),
            Item("c"),
            Item("c"),
            Item("c"),
        ];

        let runs = group(&items);
        let mut expected_start = 0;

        for run in &runs {
            assert_eq!(run.start, expected_start);
            expected_start = run.end;
        }

        assert_eq!(expected_start, items.len());
    }
}
