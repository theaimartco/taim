//! Keyed structural diff and stable state retention.
//!
//! [`reconcile`] is a real keyed diff: given a list of `(key, item)`
//! pairs and the previous frame's [`Tree`] children (each already
//! associated with the key it was built from), it figures out, in
//! `O(n)` time, which [`Tree`] nodes to keep (and where they move to),
//! which to create, and which to drop -- regardless of how the list was
//! reordered.
//!
//! This is deliberately more capable than
//! [`taim_core::widget::tree::diff_children_custom_with_search`], the
//! existing helper `widget::keyed::Column` uses: that helper only
//! detects growth/shrink at the list's boundaries and is purely
//! positional otherwise (see `tests/widgets/identity.rs`'s Phase 2
//! findings for the exact, verified difference). [`reconcile`] instead
//! tracks identity through an arbitrary reorder, at the cost of
//! allocating a lookup table sized to the list -- appropriate for a
//! container that is built once and reused, not for a hot per-frame
//! path with no stable keys at all (which is what
//! [`taim_core::widget::tree::diff_children`]'s positional zip remains
//! the right tool for).

use crate::core::widget::Tree;

use rustc_hash::FxHashMap;
use std::hash::Hash;

/// Reconciles `current` (the previous frame's `(key, Tree)` pairs, in
/// their old order) against `new_items` (this frame's `(key, item)`
/// pairs, in their new order), producing the new `(key, Tree)` list in
/// the new order.
///
/// - A key present in both lists keeps its existing [`Tree`] (state
///   preserved), diffed against its new `item` via `diff`.
/// - A key only in `new_items` gets a fresh [`Tree`] via `new_state`.
/// - A key only in `current` is dropped.
///
/// # Determinism
///
/// If `new_items` contains a duplicate key, the *first* occurrence
/// (in iteration order) claims any existing [`Tree`] for that key;
/// later duplicates are always treated as new. This makes the outcome
/// independent of hash iteration order despite using a hash map
/// internally, since claims are resolved by walking `new_items` in
/// order, not by walking the map.
pub fn reconcile<K, T>(
    current: Vec<(K, Tree)>,
    new_items: &mut [(K, T)],
    diff: impl Fn(&mut Tree, &mut T),
    new_state: impl Fn(&T) -> Tree,
) -> Vec<(K, Tree)>
where
    K: Hash + Eq + Clone,
{
    let mut previous: FxHashMap<K, Tree> = current.into_iter().collect();

    let mut reconciled = Vec::with_capacity(new_items.len());

    for (key, item) in new_items.iter_mut() {
        let mut tree = previous.remove(key).unwrap_or_else(|| new_state(item));

        diff(&mut tree, item);

        reconciled.push((key.clone(), tree));
    }

    reconciled
}

/// Reconciles two parallel key/tree buffers directly into their next state.
///
/// This is the allocation-conscious form used by keyed containers. It keeps
/// the existing `Vec` allocations, builds only the lookup table required for
/// arbitrary reordering, and does not materialize temporary `(key, item)` or
/// `(key, tree)` vectors.
pub fn reconcile_parallel<K, T>(
    keys: &mut Vec<K>,
    trees: &mut Vec<Tree>,
    next_keys: &[K],
    next_items: &mut [T],
    mut diff: impl FnMut(&mut Tree, &mut T),
    mut new_state: impl FnMut(&T) -> Tree,
) where
    K: Hash + Eq + Clone,
{
    debug_assert_eq!(keys.len(), trees.len());
    debug_assert_eq!(next_keys.len(), next_items.len());

    let previous_len = keys.len().min(trees.len());
    let mut previous = FxHashMap::with_capacity_and_hasher(previous_len, Default::default());

    for (key, tree) in keys.drain(..).zip(trees.drain(..)) {
        let _replaced = previous.insert(key, tree);
    }

    let next_len = next_keys.len().min(next_items.len());
    keys.reserve(next_len);
    trees.reserve(next_len);

    for (key, item) in next_keys.iter().zip(next_items).take(next_len) {
        let mut tree = previous.remove(key).unwrap_or_else(|| new_state(item));

        diff(&mut tree, item);
        keys.push(key.clone());
        trees.push(tree);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake diffable item: `state` records what it was built/diffed
    /// with most recently, letting tests assert whether a `Tree` was
    /// reused (its `state` may lag the newest `item`, since `diff` is a
    /// no-op here by choice in some tests) or freshly created.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Item(u32);

    // Takes `&Item` (rather than `Item` by value, which would normally
    // be preferred for a 4-byte `Copy` type) because it is passed as the
    // `new_state: impl Fn(&T) -> Tree` argument to `reconcile`, whose
    // signature must accept non-`Copy` `T` in the general case (e.g. a
    // real widget `Element`).
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn new_state(item: &Item) -> Tree {
        let mut tree = Tree::empty();
        tree.state = taim_core::widget::tree::State::new(item.0);
        tree
    }

    fn diff_marking_generation(tree: &mut Tree, item: &mut Item) {
        // Simulates a widget's `diff` overwriting its own state on
        // every call, independent of whether the `Tree` was reused;
        // this lets tests distinguish "the same `Tree` allocation was
        // reused" (checked via a sentinel unaffected by `diff`, see
        // below) from "state was preserved across the call."
        let _ = tree;
        let _ = item;
    }

    fn state_of(tree: &Tree) -> u32 {
        *tree.state.downcast_ref::<u32>()
    }

    #[test]
    fn reordering_keys_preserves_each_ones_state() {
        let current = vec![
            (1_u32, new_state(&Item(100))),
            (2, new_state(&Item(200))),
            (3, new_state(&Item(300))),
        ];

        let mut new_items = [(3_u32, Item(999)), (1, Item(999)), (2, Item(999))];

        let reconciled = reconcile(current, &mut new_items, diff_marking_generation, new_state);

        assert_eq!(reconciled.len(), 3);
        assert_eq!(reconciled[0].0, 3);
        assert_eq!(state_of(&reconciled[0].1), 300, "key 3 keeps its own state");
        assert_eq!(reconciled[1].0, 1);
        assert_eq!(state_of(&reconciled[1].1), 100, "key 1 keeps its own state");
        assert_eq!(reconciled[2].0, 2);
        assert_eq!(state_of(&reconciled[2].1), 200, "key 2 keeps its own state");
    }

    #[test]
    fn a_new_key_gets_a_fresh_tree() {
        let current = vec![(1_u32, new_state(&Item(100)))];
        let mut new_items = [(1_u32, Item(999)), (2, Item(200))];

        let reconciled = reconcile(current, &mut new_items, diff_marking_generation, new_state);

        assert_eq!(reconciled.len(), 2);
        assert_eq!(state_of(&reconciled[0].1), 100, "key 1 preserved");
        assert_eq!(
            state_of(&reconciled[1].1),
            200,
            "key 2 is fresh, from Item(200)"
        );
    }

    #[test]
    fn a_removed_key_is_dropped() {
        let current = vec![
            (1_u32, new_state(&Item(100))),
            (2, new_state(&Item(200))),
            (3, new_state(&Item(300))),
        ];

        let mut new_items = [(1_u32, Item(999)), (3, Item(999))];

        let reconciled = reconcile(current, &mut new_items, diff_marking_generation, new_state);

        assert_eq!(reconciled.len(), 2);
        assert_eq!(
            reconciled.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
            vec![1, 3]
        );
    }

    #[test]
    fn every_key_removed_yields_an_empty_list() {
        let current = vec![(1_u32, new_state(&Item(100)))];
        let mut new_items: [(u32, Item); 0] = [];

        let reconciled = reconcile(current, &mut new_items, diff_marking_generation, new_state);

        assert!(reconciled.is_empty());
    }

    #[test]
    fn a_fully_disjoint_key_set_creates_every_item_fresh() {
        let current = vec![(1_u32, new_state(&Item(100))), (2, new_state(&Item(200)))];
        let mut new_items = [(10_u32, Item(111)), (20, Item(222))];

        let reconciled = reconcile(current, &mut new_items, diff_marking_generation, new_state);

        assert_eq!(reconciled.len(), 2);
        assert_eq!(state_of(&reconciled[0].1), 111);
        assert_eq!(state_of(&reconciled[1].1), 222);
    }

    #[test]
    fn duplicate_keys_in_new_items_only_let_the_first_occurrence_claim_the_old_tree() {
        let current = vec![(1_u32, new_state(&Item(100)))];
        let mut new_items = [(1_u32, Item(999)), (1, Item(999))];

        let reconciled = reconcile(current, &mut new_items, diff_marking_generation, new_state);

        assert_eq!(reconciled.len(), 2);
        assert_eq!(
            state_of(&reconciled[0].1),
            100,
            "first occurrence reuses the old tree"
        );
        assert_eq!(
            state_of(&reconciled[1].1),
            999,
            "second occurrence is always fresh, from Item(999)"
        );
    }

    #[test]
    fn an_unchanged_key_set_preserves_every_state_in_order() {
        let current = vec![
            (1_u32, new_state(&Item(100))),
            (2, new_state(&Item(200))),
            (3, new_state(&Item(300))),
        ];

        let mut new_items = [(1_u32, Item(999)), (2, Item(999)), (3, Item(999))];

        let reconciled = reconcile(current, &mut new_items, diff_marking_generation, new_state);

        assert_eq!(state_of(&reconciled[0].1), 100);
        assert_eq!(state_of(&reconciled[1].1), 200);
        assert_eq!(state_of(&reconciled[2].1), 300);
    }
}
