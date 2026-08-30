//! Phase 15's privacy rule: *"secure text, private notifications,
//! protected surfaces, clipboard contents, and raw images are redacted by
//! default"*.
//!
//! # What these tests are, and are not
//!
//! The real guarantee is not in this file. It is in the shape of
//! [`Event`]: there is no variant that can hold user content, and the
//! only free-form string it carries is a [`Label`], which is `&'static
//! str` and can therefore only have come from the program's own source.
//! A keystroke cannot be made into a `&'static str` without leaking an
//! allocation on purpose.
//!
//! That is worth stating because the obvious design — record everything,
//! then run a redaction pass before export — is the one that fails open.
//! Every new event kind is a leak until someone teaches the pass about
//! it, and nothing fails when they forget. Here, forgetting is a compile
//! error, because there is nowhere to put the content.
//!
//! So these tests do two things a type cannot:
//!
//! - Pin the classification rules that *are* runtime decisions (a
//!   password's length is withheld; an image's is not).
//! - Pin that a realistic frame of secret-adjacent activity produces a
//!   trace containing none of the secrets — the end-to-end claim, checked
//!   against the export rather than the type.

use taim_profiler::collector::Collector;
use taim_profiler::events::{Event, Label, Redacted, Sensitivity};
use taim_profiler::export;

const PASSWORD: &str = "hunter2-correct-horse-battery-staple";
const NOTIFICATION: &str = "Your test results are ready";
const CLIPBOARD: &str = "4111 1111 1111 1111";

#[test]
fn a_password_never_reaches_a_trace() {
    // The headline claim. Note what the test *cannot* do: there is no way
    // to write the password into an event in the first place, so this
    // exercises the path a caller would actually take -- record that
    // secure text was typed, and check the export.
    let collector = Collector::enabled();

    collector.record(|| Event::Withheld {
        at: 0,
        content: Redacted::new(PASSWORD.len(), Sensitivity::Secret),
    });

    let trace = export::taim(&collector);

    assert!(
        !trace.contains(PASSWORD),
        "the password reached the trace: {trace}"
    );
    assert!(
        !trace.contains("hunter2"),
        "part of the password reached the trace: {trace}"
    );
    assert!(
        trace.contains("\"sensitivity\":\"secret\""),
        "the trace must still say that something was withheld: {trace}"
    );
}

#[test]
fn a_passwords_length_never_reaches_a_trace_either() {
    // The length of a password is worth real bits to an attacker, and no
    // profiling question needs it. Ordinary content keeps its length; this
    // one does not.
    let collector = Collector::enabled();

    collector.record(|| Event::Withheld {
        at: 0,
        content: Redacted::new(PASSWORD.len(), Sensitivity::Secret),
    });

    let trace = export::taim(&collector);

    assert!(
        !trace.contains(&PASSWORD.len().to_string()),
        "the password's length ({}) reached the trace: {trace}",
        PASSWORD.len()
    );
    assert!(!trace.contains("\"length\""), "{trace}");
}

#[test]
fn a_private_notifications_body_never_reaches_a_trace() {
    let collector = Collector::enabled();

    collector.record(|| Event::Withheld {
        at: 0,
        content: Redacted::new(NOTIFICATION.len(), Sensitivity::Private),
    });

    let trace = export::taim(&collector);

    assert!(!trace.contains(NOTIFICATION), "{trace}");
    assert!(!trace.contains("test results"), "{trace}");
    assert!(trace.contains("\"sensitivity\":\"private\""), "{trace}");
}

#[test]
fn clipboard_contents_never_reach_a_trace() {
    let collector = Collector::enabled();

    collector.record(|| Event::Withheld {
        at: 0,
        content: Redacted::new(CLIPBOARD.len(), Sensitivity::Private),
    });

    let trace = export::taim(&collector);

    assert!(!trace.contains(CLIPBOARD), "{trace}");
    assert!(!trace.contains("4111"), "{trace}");
}

#[test]
fn a_protected_surfaces_pixels_never_reach_a_trace() {
    // A protected surface is DRM video. Its *size* is a legitimate
    // profiling fact -- a developer chasing a memory question needs it --
    // and its contents are never available to this crate at all.
    let collector = Collector::enabled();

    collector.record(|| Event::Withheld {
        at: 0,
        content: Redacted::new(1920 * 1080 * 4, Sensitivity::Protected),
    });

    let trace = export::taim(&collector);

    assert!(trace.contains("\"sensitivity\":\"protected\""), "{trace}");
    assert!(
        trace.contains("\"length\":8294400"),
        "a protected surface's size is not itself a secret: {trace}"
    );
}

#[test]
fn withholding_is_recorded_rather_than_omitted() {
    // "Nothing happened" and "something happened that you are not allowed
    // to see" must be distinguishable. An absent event would look like a
    // bug in the profiler, and send a developer hunting for it.
    let collector = Collector::enabled();

    collector.record(|| Event::Withheld {
        at: 5,
        content: Redacted::new(8, Sensitivity::Secret),
    });

    assert_eq!(collector.len(), 1, "the marker itself must survive");

    let trace = export::taim(&collector);

    assert!(trace.contains("\"name\":\"withheld\""), "{trace}");
}

#[test]
fn a_withholding_marker_is_a_diagnostic_and_survives_pressure() {
    // If overflow dropped these, a trace taken under load would silently
    // stop mentioning that anything was withheld -- and "no withholding
    // markers" reads as "nothing sensitive happened".
    let collector = Collector::with_capacity(4, true);

    collector.record(|| Event::Withheld {
        at: 0,
        content: Redacted::new(8, Sensitivity::Secret),
    });

    for at in 1..1_000 {
        collector.record(|| Event::Input {
            kind: Label::new("key_pressed"),
            at,
        });
    }

    assert!(
        collector.events().iter().any(|event| matches!(
            event,
            Event::Withheld {
                content,
                ..
            } if content.sensitivity() == Sensitivity::Secret
        )),
        "the withholding marker must survive a flood of telemetry"
    );
}

#[test]
fn typing_a_password_produces_a_trace_with_no_keystrokes_in_it() {
    // The end-to-end claim, over a realistic frame: a user types into a
    // secure field, the profiler records the shape of the work, and the
    // trace contains nothing of what they typed.
    //
    // Note that `Event::Input` carries a `Label` -- the event type's own
    // name, from the source -- and not the key. That is the design: there
    // is no field for the key to go in.
    let collector = Collector::enabled();

    for (at, _character) in PASSWORD.chars().enumerate() {
        collector.record(|| Event::Input {
            kind: Label::new("key_pressed"),
            at: at as u64,
        });
    }

    collector.record(|| Event::Withheld {
        at: 100,
        content: Redacted::new(PASSWORD.len(), Sensitivity::Secret),
    });

    let trace = export::taim(&collector);

    // Every distinctive run of the password, rather than every character.
    // A first draft checked each character in quotes and failed on `"s"`
    // -- which is Chrome's `"s":"t"` scope field, not a keystroke. Single
    // characters legitimately appear in JSON structure, so checking for
    // them detects the format, not a leak.
    for window in PASSWORD
        .as_bytes()
        .windows(4)
        .map(|window| std::str::from_utf8(window).unwrap())
    {
        assert!(
            !trace.contains(window),
            "{window:?} of the password reached the trace: {trace}"
        );
    }

    assert!(!trace.contains(PASSWORD), "{trace}");
    assert_eq!(
        trace.matches("key_pressed").count(),
        PASSWORD.len(),
        "the shape of the work must still be visible: {trace}"
    );
}

#[test]
fn every_sensitivity_exports_a_name_a_tool_can_act_on() {
    // A tool decides whether to offer "show contents" from this. An
    // unrecognised classification must not silently become "public".
    for (sensitivity, expected) in [
        (Sensitivity::Public, "public"),
        (Sensitivity::Private, "private"),
        (Sensitivity::Secret, "secret"),
        (Sensitivity::Protected, "protected"),
    ] {
        let trace = export::chrome(&[Event::Withheld {
            at: 0,
            content: Redacted::new(1, sensitivity),
        }]);

        assert!(
            trace.contains(&format!("\"sensitivity\":\"{expected}\"")),
            "{sensitivity:?} must export as {expected}: {trace}"
        );
    }
}

#[test]
fn a_redaction_cannot_be_built_from_content() {
    // Stated for the next person to read before adding a convenience
    // constructor. `Redacted::new` takes a *length*, not the content: a
    // caller cannot pass the secret in by mistake, because there is no
    // parameter for it.
    //
    // A test cannot assert the absence of a function, so this stands as
    // the note that the absence is deliberate.
    let redacted = Redacted::new(PASSWORD.len(), Sensitivity::Secret);

    assert_eq!(redacted.sensitivity(), Sensitivity::Secret);
    assert_eq!(redacted.length(), None);
}
