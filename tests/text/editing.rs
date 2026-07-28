use aura_core::text::Selection;
use aura_widget::text_input::Value;

#[test]
fn text_input_value_keeps_combining_and_emoji_sequences_as_graphemes() {
    let value = Value::new("e\u{0301}👨‍👩‍👧‍👦न");

    assert_eq!(value.len(), 3);
    assert_eq!(value.select(0, 2).to_string(), "e\u{0301}👨‍👩‍👧‍👦");
}

#[test]
fn logical_selection_rejects_mid_codepoint_offsets_and_clamps_replacements() {
    let source = "A😀B";
    assert!(Selection::new(source, 1..2).is_none());

    let selection = Selection::new(source, 1..5).expect("emoji boundaries");
    let clamped = selection.clamp_to("éB");
    assert_eq!(clamped.range(), 0..3);
}

#[test]
fn word_navigation_is_bounded_for_mixed_script_values() {
    let value = Value::new("hello العربية বাংলা");
    let end = value.len();

    assert_eq!(value.previous_start_of_word(0), 0);
    assert!(value.next_end_of_word(0) <= end);
    assert!(value.previous_start_of_word(end) <= end);
}
