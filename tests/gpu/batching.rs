//! Ordering-correctness oracle for `aura_wgpu::batch::group`.
//!
//! `wgpu/src/batch.rs`'s own unit tests already cover `group`'s
//! index-range bookkeeping in isolation. These tests instead prove the
//! property that actually matters for correctness: painting every item
//! individually, in original order, and painting the same items grouped
//! into runs (one "draw call" per run, in the run's own order) must
//! produce the *exact same* final visible result whenever later items
//! painted on top can obscure earlier ones -- since grouping same-key
//! items together is only sound if it never lets an item's paint happen
//! at the wrong time relative to a different-key item it overlaps.

use aura_wgpu::batch::{self, Batchable};

/// A synthetic paintable item: a "pixel" position it claims (so overlap
/// is easy to construct) and a key controlling what it batches with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Item {
    key: u8,
    pixel: u32,
    color: u32,
}

impl Batchable for Item {
    type Key = u8;

    fn batch_key(&self) -> u8 {
        self.key
    }
}

/// Paints `items` one at a time, in order, onto a single-pixel-per-slot
/// canvas (later writes to the same `pixel` win) -- the reference
/// behavior any batching strategy must reproduce exactly.
fn paint_individually(items: &[Item], canvas_size: usize) -> Vec<Option<u32>> {
    let mut canvas = vec![None; canvas_size];

    for item in items {
        canvas[item.pixel as usize] = Some(item.color);
    }

    canvas
}

/// Paints `items` by grouping them into runs via `batch::group`, then
/// painting each run's items (still in their original relative order,
/// since `group` never reorders within a run) -- what a real batched
/// renderer does: one draw call per run, runs executed run-by-run.
fn paint_batched(items: &[Item], canvas_size: usize) -> Vec<Option<u32>> {
    let mut canvas = vec![None; canvas_size];
    let runs = batch::group(items);

    for run in runs {
        for item in &items[run.start..run.end] {
            canvas[item.pixel as usize] = Some(item.color);
        }
    }

    canvas
}

fn assert_batching_matches_individual_painting(items: &[Item], canvas_size: usize) {
    assert_eq!(
        paint_batched(items, canvas_size),
        paint_individually(items, canvas_size),
        "batched painting of {items:?} must match painting each item individually, in order"
    );
}

#[test]
fn non_overlapping_items_of_mixed_keys_match_regardless_of_batching() {
    let items = [
        Item {
            key: 0,
            pixel: 0,
            color: 0xAA,
        },
        Item {
            key: 1,
            pixel: 1,
            color: 0xBB,
        },
        Item {
            key: 0,
            pixel: 2,
            color: 0xCC,
        },
    ];

    assert_batching_matches_individual_painting(&items, 3);
}

#[test]
fn a_later_item_of_a_different_key_correctly_overwrites_an_earlier_overlapping_one() {
    // Item 1 (key 1) is painted after item 0 (key 0) and claims the same
    // pixel. If `group` incorrectly reordered same-key items across a
    // different-key item to enlarge a run, item 0's second occurrence
    // could end up painted after item 1 again, wrongly winning the
    // pixel back.
    let items = [
        Item {
            key: 0,
            pixel: 0,
            color: 0xAA,
        }, // painted first
        Item {
            key: 1,
            pixel: 0,
            color: 0xBB,
        }, // painted second, must win
        Item {
            key: 0,
            pixel: 1,
            color: 0xCC,
        }, // a later, different pixel
    ];

    let result = paint_batched(&items, 2);

    assert_eq!(
        result[0],
        Some(0xBB),
        "the later (key 1) item must win the overlapping pixel"
    );
    assert_batching_matches_individual_painting(&items, 2);
}

#[test]
fn many_alternating_overlapping_items_still_match_individual_painting() {
    // A worst case for any batching strategy tempted to reorder for
    // bigger runs: every item overlaps the same pixel, alternating key
    // every time, so only the very last item's color may show.
    let items: Vec<Item> = (0..20)
        .map(|i| Item {
            key: (i % 2) as u8,
            pixel: 0,
            color: i as u32,
        })
        .collect();

    assert_batching_matches_individual_painting(&items, 1);

    let result = paint_batched(&items, 1);
    assert_eq!(result[0], Some(19), "the very last item must win");
}

#[test]
fn runs_are_never_reordered_relative_to_each_other() {
    let items = [
        Item {
            key: 5,
            pixel: 0,
            color: 1,
        },
        Item {
            key: 5,
            pixel: 0,
            color: 2,
        },
        Item {
            key: 9,
            pixel: 0,
            color: 3,
        },
        Item {
            key: 5,
            pixel: 0,
            color: 4,
        },
    ];

    let runs = batch::group(&items);

    assert_eq!(runs.len(), 3, "key 5 must not merge across the key-9 run");
    assert_eq!(runs[0].key, 5);
    assert_eq!(runs[1].key, 9);
    assert_eq!(runs[2].key, 5);

    // The final winner must be color 4 (the true last item), not color
    // 2 (which a buggy "merge same-key runs across the middle" strategy
    // would incorrectly paint last).
    assert_eq!(paint_batched(&items, 1)[0], Some(4));
}
