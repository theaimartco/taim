//! Phase 17: *"privacy classification propagated to render/tool/platform
//! paths"*, and its test row — *"screenshots, overview, profiler,
//! semantics and notifications"*.
//!
//! # What makes this suite worth having
//!
//! Each subsystem here already has its own tests, and each of them passes.
//! That is exactly the problem this file exists for: a password leaks not
//! because someone wrote a bad password check, but because the screenshot
//! encoder was written by someone who was not thinking about passwords
//! that day, and nothing told them to.
//!
//! So these tests do not ask "does the text field hide its contents". They
//! ask **the same question of every path at once**, and fail if any one of
//! them answers differently. A per-subsystem suite cannot catch a
//! disagreement between subsystems; only a table can.

use aura_core::security::{Audience, Classified, Privacy};
use aura_core::semantics::{Node, Role, Value};
use aura_core::widget::Id;
use aura_core::{Rectangle, Size};

use aura_profiler::events::{Event, Redacted};
use aura_profiler::{Collector, export};

const PASSWORD: &str = "hunter2-correct-horse";
const NOTIFICATION: &str = "Your test results are ready";

fn node(role: Role) -> Node {
    Node::new(
        Id::unique(),
        Rectangle::new(aura_core::Point::ORIGIN, Size::new(10.0, 10.0)),
        role,
    )
}

// ---------------------------------------------------------------------
// The table: every classification against every path
// ---------------------------------------------------------------------

#[test]
fn a_secret_reaches_exactly_one_path_and_it_is_the_widget_drawing_it() {
    // A password field draws a row of dots. That is the only path.
    for audience in Audience::ALL {
        assert_eq!(
            Privacy::Secret.allows(audience),
            audience == Audience::Render,
            "a secret must reach {audience:?} exactly when it is Render"
        );
    }
}

#[test]
fn protected_pixels_reach_no_path_at_all() {
    // Including `Render`. The compositor holds these; the framework never
    // saw them, so there is no fallback that could produce them and any
    // that appears to is a bug.
    for audience in Audience::ALL {
        assert!(
            !Privacy::Protected.allows(audience),
            "protected content must not reach {audience:?}"
        );
    }
}

#[test]
fn no_non_public_content_reaches_a_screenshot_an_overview_or_a_trace() {
    // The three paths that outlive the moment: a screenshot is a file, a
    // thumbnail is shown over someone's shoulder, a trace gets attached to
    // a bug report.
    for privacy in [Privacy::Private, Privacy::Secret, Privacy::Protected] {
        for audience in [Audience::Screenshot, Audience::Overview, Audience::Profiler] {
            assert!(
                !privacy.allows(audience),
                "{privacy:?} must not reach {audience:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------
// Semantics
// ---------------------------------------------------------------------

#[test]
fn a_secure_field_discloses_no_value_to_the_accessibility_tree() {
    // Enforced on the write path *and* the read path, so a node built by
    // some future code that skipped the setter still cannot disclose.
    let secure = node(Role::SecureField).with_value(Value::Text(PASSWORD.into()));

    assert_eq!(secure.value(), None);
}

#[test]
fn a_secure_fields_role_is_its_classification() {
    // The role *is* the rule. A password field is not secret because
    // someone remembered to mark it; it is secret because of what it is,
    // so every consumer can work it out rather than trusting a flag.
    assert_eq!(Role::SecureField.privacy(), Privacy::Secret);
    assert_eq!(Role::TextField.privacy(), Privacy::Public);
}

#[test]
fn an_ordinary_text_field_still_discloses_its_value() {
    // The converse: a rule that hid everything would be trivially safe and
    // useless. A screen reader must read an ordinary field.
    let ordinary = node(Role::TextField).with_value(Value::Text("hello".into()));

    assert!(ordinary.value().is_some());
}

#[test]
fn semantics_and_the_classification_agree_about_every_role() {
    // The disagreement this suite exists to catch: `semantics` used to
    // decide this with a `role == Role::SecureField` of its own. Two
    // copies of one rule is how the fourth copy ends up disagreeing.
    for role in [
        Role::SecureField,
        Role::TextField,
        Role::StaticText,
        Role::Button,
        Role::Slider,
    ] {
        let discloses = node(role)
            .with_value(Value::Text("x".into()))
            .value()
            .is_some();

        assert_eq!(
            discloses,
            role.privacy().allows(Audience::Semantics),
            "{role:?} disclosed a value that its classification forbids (or vice versa)"
        );
    }
}

// ---------------------------------------------------------------------
// Notifications
// ---------------------------------------------------------------------

#[test]
fn a_private_notification_is_withheld_from_a_lock_screen() {
    let body = Classified::new(NOTIFICATION, Privacy::Private);

    assert_eq!(body.reveal_to(Audience::LockScreen), None);
    assert_eq!(body.reveal_to(Audience::Render), Some(&NOTIFICATION));
}

#[test]
fn a_private_notification_still_reaches_a_screen_reader() {
    // The distinction `Private` exists to draw. Withholding here would
    // make the feature inaccessible rather than private: a notification a
    // sighted user can read and a blind user cannot is not a privacy win.
    let body = Classified::new(NOTIFICATION, Privacy::Private);

    assert_eq!(body.reveal_to(Audience::Semantics), Some(&NOTIFICATION));
}

// ---------------------------------------------------------------------
// The profiler
// ---------------------------------------------------------------------

#[test]
fn the_profiler_and_the_classification_are_the_same_vocabulary() {
    // Not "agree" -- *are*. `aura_profiler::Sensitivity` is a re-export of
    // `security::Privacy`. An earlier draft had a parallel enum with the
    // same four variants, which is the exact failure this whole design is
    // about: two vocabularies is how five paths agree and the sixth leaks.
    let profiler: aura_profiler::Sensitivity = Privacy::Secret;

    assert_eq!(profiler, Privacy::Secret);
}

#[test]
fn a_secure_fields_length_never_reaches_a_trace() {
    // A password's length is worth real bits to an attacker, and no
    // profiling question needs it.
    let collector = Collector::enabled();

    collector.record(|| Event::Withheld {
        at: 0,
        content: Redacted::new(PASSWORD.len(), Role::SecureField.privacy()),
    });

    let trace = export::aura(&collector);

    assert!(!trace.contains(PASSWORD), "{trace}");
    assert!(!trace.contains("\"length\""), "{trace}");
    assert!(trace.contains("\"sensitivity\":\"secret\""), "{trace}");
}

#[test]
fn a_protected_surfaces_size_reaches_a_trace_but_its_pixels_cannot() {
    // The size is a legitimate profiling fact -- a developer chasing a
    // memory question needs it -- and the pixels were never available to
    // the framework at all.
    let collector = Collector::enabled();

    collector.record(|| Event::Withheld {
        at: 0,
        content: Redacted::new(1920 * 1080 * 4, Privacy::Protected),
    });

    let trace = export::aura(&collector);

    assert!(trace.contains("\"length\":8294400"), "{trace}");
    assert!(trace.contains("\"sensitivity\":\"protected\""), "{trace}");
}

// ---------------------------------------------------------------------
// Propagation
// ---------------------------------------------------------------------

#[test]
fn one_secret_field_makes_a_whole_dialog_secret() {
    // The safety argument. Marking the password field is enough; nobody
    // has to remember to mark the dialog, the form, or the window.
    let login = Privacy::of([
        Role::StaticText.privacy(),  // "Password:"
        Role::SecureField.privacy(), // the field itself
        Role::Button.privacy(),      // "Sign in"
    ]);

    assert_eq!(login, Privacy::Secret);

    for audience in [Audience::Screenshot, Audience::Overview, Audience::Profiler] {
        assert!(
            !login.allows(audience),
            "the dialog must not reach {audience:?}"
        );
    }
}

#[test]
fn a_derived_value_of_a_secret_stays_secret_through_every_path() {
    // The classic leak: measure the password, publish the length. The
    // measurement is as secret as what was measured.
    let length = Classified::new(PASSWORD, Privacy::Secret).map(str::len);

    for audience in Audience::ALL {
        assert_eq!(
            length.reveal_to(audience).is_some(),
            audience == Audience::Render,
            "a secret's length leaked to {audience:?}"
        );
    }
}

#[test]
fn a_public_form_stays_public() {
    // The converse, again: a propagation rule that classified everything
    // as secret would be safe and useless.
    let form = Privacy::of([
        Role::StaticText.privacy(),
        Role::TextField.privacy(),
        Role::Button.privacy(),
    ]);

    assert_eq!(form, Privacy::Public);

    for audience in Audience::ALL {
        assert!(form.allows(audience));
    }
}
