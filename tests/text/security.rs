//! The secure-field disclosure threat matrix.
//!
//! `phase.md` Phase 10's security requirements name six channels through
//! which a secure text widget must never expose its content:
//!
//! > clipboard, semantics value, debug logs, surrounding-text APIs,
//! > retained screenshots, or profiler captures
//!
//! plus two further rules: password reveal must be explicit and
//! policy-controlled, and untrusted text must not create unbounded
//! shaping/cache allocations.
//!
//! Each channel gets a test here that attempts the disclosure and
//! asserts it fails. A test that merely asserts a flag is set would not
//! be a threat test; these format, mask, and query the real types the way
//! a leak actually happens.

use taim_core::input_method::{Purpose, SurroundingText};
use taim_core::semantics::{Node, Role, Value as SemanticValue};
use taim_core::widget::Id;
use taim_core::{Rectangle, Size};

use taim_widget::text_input::Value;

/// The secret every test below tries, and fails, to extract.
const SECRET: &str = "correct horse battery staple";

fn node(role: Role) -> Node {
    Node::new(
        Id::unique(),
        Rectangle::with_size(Size::new(100.0, 20.0)),
        role,
    )
}

// ---------------------------------------------------------------------
// Channel 1: surrounding-text APIs
// ---------------------------------------------------------------------

#[test]
fn a_secure_field_discloses_nothing_through_surrounding_text() {
    let disclosure = SurroundingText::for_field(SECRET, 7..12, true);

    assert_eq!(disclosure, SurroundingText::Redacted);
    assert!(disclosure.is_redacted());

    // Not even via the debug formatting of the disclosure itself.
    let formatted = format!("{disclosure:?}");
    assert!(!formatted.contains("correct"));
    assert!(!formatted.contains(SECRET));
}

#[test]
fn a_non_secure_field_still_supplies_surrounding_text_to_the_ime() {
    // The converse: redaction must be specific to secure fields, or an
    // ordinary field would lose IME support entirely.
    let disclosure = SurroundingText::for_field("hello", 0..5, false);

    assert!(!disclosure.is_redacted());
    assert_eq!(
        disclosure,
        SurroundingText::Visible {
            content: "hello".to_owned(),
            selection: 0..5,
        }
    );
}

// ---------------------------------------------------------------------
// Channel 2: semantics value (assistive-technology bridge)
// ---------------------------------------------------------------------

#[test]
fn a_secure_field_node_refuses_a_semantic_value() {
    // The AT bridge is a disclosure channel: a screen reader that can
    // read a password aloud is a vulnerability, not a feature.
    let secure = node(Role::SecureField).with_value(SemanticValue::Text(SECRET.into()));

    assert_eq!(
        secure.value(),
        None,
        "a SecureField must never carry a semantic value"
    );

    let formatted = format!("{secure:?}");
    assert!(
        !formatted.contains("correct"),
        "the secret leaked through the node's own debug output: {formatted}"
    );
}

#[test]
fn a_secure_field_refuses_a_value_no_matter_the_construction_order() {
    // Enforcement must not depend on the caller's ordering: setting the
    // value before or after other properties must be equally refused.
    let a = node(Role::SecureField)
        .with_name("Password")
        .with_value(SemanticValue::Text(SECRET.into()));
    let b = node(Role::SecureField)
        .with_value(SemanticValue::Text(SECRET.into()))
        .with_name("Password");

    assert_eq!(a.value(), None);
    assert_eq!(b.value(), None);

    // The accessible *name* is still required -- redaction must not make
    // the field unusable to a screen reader, only unreadable.
    assert!(a.name.is_some());
    assert!(b.name.is_some());
}

#[test]
fn an_ordinary_text_field_still_discloses_its_value() {
    // The converse: only SecureField is redacted, or every text field
    // would be invisible to assistive technology.
    let ordinary = node(Role::TextField).with_value(SemanticValue::Text("hello".into()));

    assert_eq!(
        ordinary.value(),
        Some(&SemanticValue::Text("hello".into())),
        "an ordinary field must remain readable by a screen reader"
    );
}

// ---------------------------------------------------------------------
// Channel 3: debug logs (and, by extension, panic messages and
// crash reports, which format the same way)
// ---------------------------------------------------------------------

#[test]
fn a_value_never_discloses_its_content_through_debug() {
    // This is the channel that leaks silently: any `log::debug!("{:?}",
    // state)` on a struct holding a Value would spill the password into
    // a log file, a panic message, or a crash report.
    let secret = Value::new(SECRET);

    let formatted = format!("{secret:?}");

    assert!(
        !formatted.contains("correct"),
        "Value's Debug disclosed its content: {formatted}"
    );
    assert!(!formatted.contains(SECRET));
    // Individual graphemes must not survive either -- a derived Debug
    // would print them as a list, which is just as readable.
    assert!(
        !formatted.contains('h') || !formatted.contains('o'),
        "Value's Debug appears to be printing graphemes: {formatted}"
    );
}

#[test]
fn a_value_does_not_disclose_its_length_through_debug() {
    // A length-preserving disclosure is still a disclosure: this crate
    // already holds that line for `SurroundingText`, and a password's
    // length is exactly what that principle protects.
    let short = Value::new("ab");
    let long = Value::new(SECRET);

    assert_eq!(
        format!("{short:?}"),
        format!("{long:?}"),
        "Debug output differs by length, disclosing how long the secret is"
    );
}

#[test]
fn debug_redaction_survives_being_nested_inside_another_struct() {
    // The realistic leak: nobody debug-prints a Value directly; they
    // debug-print the struct that holds one.
    #[derive(Debug)]
    #[allow(dead_code)]
    struct FormState {
        username: String,
        password: Value,
    }

    let state = FormState {
        username: "alice".to_owned(),
        password: Value::new(SECRET),
    };

    let formatted = format!("{state:?}");

    assert!(
        !formatted.contains("correct"),
        "the secret leaked through the containing struct: {formatted}"
    );
    // The non-secret field is still debuggable -- redaction must be
    // surgical, not blanket.
    assert!(formatted.contains("alice"));
}

// ---------------------------------------------------------------------
// Channel 4: clipboard
// ---------------------------------------------------------------------

#[test]
fn a_secure_value_masks_every_grapheme_when_rendered_or_copied() {
    // `Value::secure()` is what a secure field substitutes before the
    // content reaches rendering or any copy path. It must mask *every*
    // grapheme, leaving nothing of the original.
    let masked = Value::new(SECRET).secure();
    let rendered = masked.to_string();

    assert!(!rendered.contains("correct"));
    assert!(
        rendered.chars().all(|c| c == '•'),
        "a masked value must contain nothing but bullets, got: {rendered}"
    );
    assert_eq!(masked.len(), Value::new(SECRET).len());
}

#[test]
fn masking_a_multi_codepoint_grapheme_yields_exactly_one_bullet() {
    // A masked value must not disclose *structure* either: an emoji ZWJ
    // sequence or a combining mark is one grapheme, so it must become one
    // bullet, not one per codepoint -- otherwise the bullet count would
    // reveal how the secret is composed.
    let family = Value::new("👨‍👩‍👧‍👦");
    assert_eq!(family.len(), 1);
    assert_eq!(family.secure().to_string(), "•");

    let combining = Value::new("e\u{0301}");
    assert_eq!(combining.len(), 1);
    assert_eq!(combining.secure().to_string(), "•");
}

// ---------------------------------------------------------------------
// Channel 5 & 6: retained screenshots and profiler captures
// ---------------------------------------------------------------------

#[test]
fn the_ime_purpose_marks_secure_fields_for_platform_capture_policy() {
    // Screenshot and profiler exclusion is ultimately enforced by the
    // platform (FLAG_SECURE on Android, the compositor's own policy on
    // Wayland). What this codebase owes the platform is the *signal*:
    // `Purpose::Secure` is what a backend maps onto that flag.
    //
    // This test pins the signal's existence and distinctness; the
    // backend wiring that consumes it is recorded as a follow-up in
    // docs/architecture/text-ime.md.
    assert_ne!(Purpose::Secure, Purpose::Normal);
    assert_ne!(Purpose::Secure, Purpose::Terminal);

    // The default must never be Secure-by-accident, nor Normal for a
    // field that asked to be secure.
    assert_eq!(Purpose::default(), Purpose::Normal);
}

// ---------------------------------------------------------------------
// Password reveal policy
// ---------------------------------------------------------------------

#[test]
fn revealing_a_password_is_an_explicit_act_that_produces_a_distinct_value() {
    // "Password reveal is explicit": there is no ambient way to see the
    // content. A field either substitutes `secure()` (masked) or it does
    // not (revealed) -- and the two must be observably different, so a
    // reveal can never happen without the caller choosing it.
    let value = Value::new(SECRET);

    let masked = value.secure().to_string();
    let revealed = value.to_string();

    assert_ne!(masked, revealed);
    assert_eq!(
        revealed, SECRET,
        "an explicit reveal shows the real content"
    );
    assert!(!masked.contains("correct"));
}

// ---------------------------------------------------------------------
// Unbounded allocation from untrusted text
// ---------------------------------------------------------------------

#[test]
fn a_pathological_grapheme_cluster_does_not_explode_the_value() {
    // "Untrusted text cannot create unbounded shaping/cache
    // allocations." A single grapheme built from a long combining run is
    // the classic amplification: it must stay *one* grapheme, so
    // per-grapheme costs downstream stay bounded by what the user can
    // actually see.
    let zalgo: String = std::iter::once('e')
        .chain(std::iter::repeat_n('\u{0301}', 2_000))
        .collect();

    let value = Value::new(&zalgo);

    assert_eq!(
        value.len(),
        1,
        "a combining run must collapse to one grapheme, not {}",
        value.len()
    );
    assert_eq!(value.secure().to_string(), "•");
}

#[test]
fn a_long_untrusted_string_stays_proportional_and_terminates() {
    // Linear, not quadratic or worse: a large paste must not blow up.
    let long = "a".repeat(100_000);
    let value = Value::new(&long);

    assert_eq!(value.len(), 100_000);
    assert_eq!(value.secure().len(), 100_000);

    // Word navigation over a large value must terminate and stay in
    // bounds rather than running away.
    assert_eq!(value.previous_start_of_word(0), 0);
    assert!(value.next_end_of_word(0) <= value.len());
}
