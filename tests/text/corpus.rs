use taim_core::text::paragraph::{Direction, ShapingKey};
use taim_core::text::{FallbackChain, LineBreakPolicy, Script};

const CORPUS: &[(&str, Script)] = &[
    ("English text", Script::Latin),
    ("हिंदी", Script::Devanagari),
    ("ਪੰਜਾਬੀ", Script::Gurmukhi),
    ("العربية", Script::Arabic),
    ("বাংলা", Script::Bengali),
    ("தமிழ்", Script::Tamil),
    ("👨‍👩‍👧‍👦", Script::Emoji),
];

#[test]
fn mandatory_language_corpus_has_deterministic_fallbacks() {
    for (text, expected) in CORPUS {
        let key = ShapingKey::for_text(text).direction(Direction::Auto);
        assert_eq!(key.script, *expected, "{text}");
        assert_eq!(key.fallback, FallbackChain::for_script(*expected));
        assert!(!key.fallback.families().is_empty());
    }
}

#[test]
fn combining_marks_and_mixed_bidi_have_safe_break_offsets() {
    let text = "e\u{0301}tude, title كتاب مفتوح / বাংলা";
    let breaks = LineBreakPolicy::Conservative.opportunities(text, Some("und"));

    assert!(
        breaks
            .iter()
            .all(|break_| text.is_char_boundary(break_.offset))
    );
    assert!(breaks.iter().any(|break_| break_.offset > 0));
}

#[test]
fn variable_axis_identity_is_order_independent() {
    let first = ShapingKey::for_text("variable")
        .axis(*b"wght", 400.0)
        .axis(*b"wdth", 100.0);
    let second = ShapingKey::for_text("variable")
        .axis(*b"wdth", 100.0)
        .axis(*b"wght", 400.0);

    assert_eq!(first, second);
}
