//! Deterministic multilingual shaping-identity oracles.
//!
//! `phase.md` Phase 10's exit gate requires that "cache keys include
//! every shaping-affecting input" and that there is "no reshaping for
//! unchanged text". Both are properties of [`ShapingKey`], and both are
//! testable without a font rasterizer: the key *is* the cache identity,
//! so proving it discriminates on every shaping input -- and only on
//! shaping inputs -- proves the cache cannot serve a wrongly-shaped run,
//! nor reshape one it already has.
//!
//! (Rendered pixel goldens would need a licensed reference font and human
//! visual approval; `assets/fonts/test/README.md` records that gap. What
//! is pinned here is the identity contract those pixels would depend on.)

use taim_core::text::paragraph::{Direction, ShapingKey};
use taim_core::text::{FallbackChain, Script};

use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

/// The mandatory language matrix from `phase.md`.
const CORPUS: &[(&str, Script)] = &[
    ("English text", Script::Latin),
    ("हिंदी", Script::Devanagari),
    ("ਪੰਜਾਬੀ", Script::Gurmukhi),
    ("العربية", Script::Arabic),
    ("বাংলা", Script::Bengali),
    ("தமிழ்", Script::Tamil),
    ("👨‍👩‍👧‍👦", Script::Emoji),
];

/// Reference scale factors a real device presents.
const SCALES: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

fn hash_of(key: &ShapingKey) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash_into(&mut hasher);
    hasher.finish()
}

// ---------------------------------------------------------------------
// "No reshaping for unchanged text": identical inputs => identical key
// ---------------------------------------------------------------------

#[test]
fn identical_inputs_produce_an_identical_key_and_hash() {
    // The cache-hit path: two independently-built keys for the same text
    // and attributes must be equal *and* hash equally, or unchanged text
    // would reshape on every frame.
    for scale in SCALES {
        for (text, _) in CORPUS {
            let first = ShapingKey::for_text(text)
                .locale("en-US")
                .direction(Direction::Auto)
                .axis(*b"opsz", 14.0 * scale);
            let second = ShapingKey::for_text(text)
                .locale("en-US")
                .direction(Direction::Auto)
                .axis(*b"opsz", 14.0 * scale);

            assert_eq!(first, second, "{text} @ {scale}");
            assert_eq!(hash_of(&first), hash_of(&second), "{text} @ {scale}");
        }
    }
}

#[test]
fn every_script_in_the_corpus_maps_to_its_deterministic_fallback_chain() {
    for (text, expected) in CORPUS {
        let key = ShapingKey::for_text(text);

        assert_eq!(key.script, *expected, "{text}");
        assert_eq!(
            key.fallback,
            FallbackChain::for_script(*expected),
            "{text}: fallback must follow deterministically from the script"
        );
        assert!(
            !key.fallback.families().is_empty(),
            "{text}: every script needs at least one family to try"
        );
    }
}

// ---------------------------------------------------------------------
// "Cache keys include every shaping-affecting input"
// ---------------------------------------------------------------------

#[test]
fn a_different_script_produces_a_different_key() {
    // The core correctness property: a run shaped for Devanagari must
    // never be served from a cache entry shaped for Arabic.
    let mut seen: Vec<(Script, ShapingKey)> = Vec::new();

    for (text, script) in CORPUS {
        let key = ShapingKey::for_text(text);

        for (other_script, other_key) in &seen {
            assert_ne!(
                key, *other_key,
                "{script:?} and {other_script:?} collide on one cache key"
            );
        }

        seen.push((*script, key));
    }

    assert_eq!(seen.len(), CORPUS.len());
}

#[test]
fn a_different_locale_produces_a_different_key() {
    // Same characters, different locale: Han unification and locale-aware
    // shaping mean these are genuinely different runs.
    let base = ShapingKey::for_text("text");
    let english = base.clone().locale("en-US");
    let german = base.clone().locale("de-DE");

    assert_ne!(english, german);
    assert_ne!(hash_of(&english), hash_of(&german));

    // An unset locale is distinct from any set one.
    assert_ne!(base, english);
}

#[test]
fn a_different_direction_produces_a_different_key() {
    let base = ShapingKey::for_text("text");

    let auto = base.clone().direction(Direction::Auto);
    let ltr = base.clone().direction(Direction::LeftToRight);
    let rtl = base.clone().direction(Direction::RightToLeft);

    assert_ne!(ltr, rtl, "a mirrored run must not reuse an LTR shaping");
    assert_ne!(auto, ltr);
    assert_ne!(auto, rtl);
}

#[test]
fn a_different_variable_axis_value_produces_a_different_key() {
    // Variable fonts: the same text at weight 400 and weight 700 are
    // different shapings, and an optical size that tracks scale factor
    // must not be collapsed either.
    let light = ShapingKey::for_text("variable").axis(*b"wght", 400.0);
    let bold = ShapingKey::for_text("variable").axis(*b"wght", 700.0);

    assert_ne!(light, bold);
    assert_ne!(hash_of(&light), hash_of(&bold));

    // Every reference scale is a distinct optical size.
    let mut keys = Vec::new();
    for scale in SCALES {
        let key = ShapingKey::for_text("variable").axis(*b"opsz", 14.0 * scale);
        assert!(
            !keys.contains(&key),
            "scale {scale} collides with another scale's key"
        );
        keys.push(key);
    }
    assert_eq!(keys.len(), SCALES.len());
}

#[test]
fn adding_an_axis_changes_the_key_but_reordering_axes_does_not() {
    let bare = ShapingKey::for_text("variable");
    let weighted = bare.clone().axis(*b"wght", 400.0);

    assert_ne!(bare, weighted, "an axis is a shaping-affecting input");

    // ...but axis *order* is not: the same set of axes supplied in any
    // order is the same shaping, and must hit the same cache entry rather
    // than reshaping.
    let first = ShapingKey::for_text("variable")
        .axis(*b"wght", 400.0)
        .axis(*b"wdth", 100.0);
    let second = ShapingKey::for_text("variable")
        .axis(*b"wdth", 100.0)
        .axis(*b"wght", 400.0);

    assert_eq!(first, second);
    assert_eq!(hash_of(&first), hash_of(&second));
}

#[test]
fn replacing_an_axis_value_does_not_accumulate_duplicates() {
    // Setting the same axis twice must replace, not append: a key that
    // grew every time an axis was re-set would both leak memory and
    // produce a different key for identical shaping.
    let once = ShapingKey::for_text("variable").axis(*b"wght", 700.0);
    let twice = ShapingKey::for_text("variable")
        .axis(*b"wght", 400.0)
        .axis(*b"wght", 700.0);

    assert_eq!(once, twice);
    assert_eq!(once.axes.len(), 1);
    assert_eq!(twice.axes.len(), 1);
}

#[test]
fn axis_values_that_differ_only_in_float_representation_stay_distinct() {
    // Axes are quantized by bit pattern, so nearby-but-different values
    // must not silently collapse into one cache entry.
    let a = ShapingKey::for_text("variable").axis(*b"wght", 400.0);
    let b = ShapingKey::for_text("variable").axis(*b"wght", 400.5);

    assert_ne!(a, b);
}

// ---------------------------------------------------------------------
// The full matrix
// ---------------------------------------------------------------------

#[test]
fn the_whole_corpus_at_every_scale_yields_pairwise_distinct_keys() {
    // The complete identity claim: across every language and every
    // reference scale, no two distinct (text, scale) pairs share a cache
    // key. A collision here would render one language with another's
    // shaping.
    let mut keys: Vec<((&str, f32), ShapingKey)> = Vec::new();

    for (text, _) in CORPUS {
        for scale in SCALES {
            let key = ShapingKey::for_text(text).axis(*b"opsz", 14.0 * scale);

            for ((other_text, other_scale), other_key) in &keys {
                // Two different languages that share a script legitimately
                // share a key; our corpus has one text per script, so any
                // collision here is a real defect.
                assert_ne!(
                    key, *other_key,
                    "({text}, {scale}) collides with ({other_text}, {other_scale})"
                );
            }

            keys.push(((text, scale), key));
        }
    }

    assert_eq!(
        keys.len(),
        CORPUS.len() * SCALES.len(),
        "the matrix must cover every language at every scale"
    );
}
