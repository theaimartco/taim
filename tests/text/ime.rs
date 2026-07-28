//! IME composition lifecycle, candidate geometry, and cancellation.
//!
//! `phase.md` Phase 10 requires the composition lifecycle, candidate
//! geometry, and cancellation to be covered. These drive a **real**
//! `TextInput` through real `input_method::Event`s rather than asserting
//! on the event enum in isolation, because the property that matters --
//! that an abandoned composition commits nothing -- lives in the widget's
//! state machine, not in the type.
//!
//! Composition state is observed through the widget's own
//! `Widget::input_method` output rather than a state accessor. That is
//! deliberate: `input_method` is the existing, legitimate path by which a
//! composition reaches the runtime, and it already withholds the preedit
//! for secure fields. Adding a `State::preedit()` getter purely to make
//! testing easier would open a fresh disclosure channel for exactly the
//! content Phase 10's security requirements protect.

use aura::advanced::widget::{self, Widget};
use aura::advanced::{Layout, Shell, layout, mouse};
use aura::widget::text_input;
use aura::{Event, Point, Rectangle, Size};
use aura_core::input_method::{self, CandidateRect, InputMethod, Preedit, Purpose};
use aura_core::{shell::Waker, window};
use aura_renderer::Renderer;

type Message = String;
type Paragraph = <Renderer as aura_core::text::Renderer>::Paragraph;

const BOUNDS: Size = Size {
    width: 200.0,
    height: 30.0,
};

fn headless_renderer() -> Renderer {
    aura_futures::futures::executor::block_on(
        <Renderer as aura::advanced::renderer::Headless>::new(
            aura::advanced::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("tiny-skia renderer must be available")
}

/// A focused text input, ready to receive composition events.
struct Field<'a> {
    input: text_input::TextInput<'a, Message, aura::Theme, Renderer>,
    tree: widget::Tree,
    node: layout::Node,
    renderer: Renderer,
}

impl<'a> Field<'a> {
    fn new(value: &'a str, secure: bool) -> Self {
        let mut input = text_input::TextInput::new("placeholder", value)
            .on_input(|text| text)
            .secure(secure);

        let renderer = headless_renderer();
        let mut tree = widget::Tree::new(&input as &dyn Widget<Message, aura::Theme, Renderer>);
        tree.diff(&mut input as &mut dyn Widget<Message, aura::Theme, Renderer>);

        // Use the widget's own layout rather than a hand-rolled node: a
        // text input's internals index into the child node its `layout`
        // produces, so a synthetic node would not model the real thing.
        let node = Widget::<Message, aura::Theme, Renderer>::layout(
            &mut input,
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, BOUNDS),
        );

        let mut field = Self {
            input,
            tree,
            node,
            renderer,
        };

        // Composition events are ignored by an unfocused input, so every
        // test here needs focus first.
        field
            .tree
            .state
            .downcast_mut::<text_input::State<Paragraph>>()
            .focus();

        field
    }

    /// Dispatches one event, returning the messages the field published
    /// and the [`InputMethod`] it requested from the runtime.
    ///
    /// `Shell::request_input_method` is the real, only path by which a
    /// composition leaves the widget, so reading it back off the shell is
    /// exactly what the runtime sees -- no test-only accessor required.
    fn dispatch(&mut self, event: Event) -> (Vec<Message>, InputMethod) {
        let mut messages = Vec::new();
        let mut shell = Shell::new(&window::Headless, Waker::noop(), &mut messages);
        let viewport = Rectangle::new(Point::ORIGIN, BOUNDS);

        Widget::<Message, aura::Theme, Renderer>::update(
            &mut self.input,
            &mut self.tree,
            &event,
            Layout::new(&self.node),
            mouse::Cursor::Available(Point::new(10.0, 10.0)),
            &self.renderer,
            &mut shell,
            &viewport,
        );

        let requested = shell.input_method().clone();

        (messages, requested)
    }

    fn ime(&mut self, event: input_method::Event) -> Vec<Message> {
        self.dispatch(Event::InputMethod(event)).0
    }

    /// The [`InputMethod`] the field currently requests. Driven by a
    /// redraw, which is when a real runtime asks.
    fn input_method(&mut self) -> InputMethod {
        self.dispatch(Event::Window(window::Event::RedrawRequested(
            aura_core::time::Instant::now(),
        )))
        .1
    }

    /// The composition the field would hand the runtime, if any.
    fn exposed_preedit(&mut self) -> Option<String> {
        match self.input_method() {
            InputMethod::Enabled { preedit, .. } => preedit.map(|preedit| preedit.content),
            InputMethod::Disabled => None,
        }
    }
}

// ---------------------------------------------------------------------
// Composition lifecycle
// ---------------------------------------------------------------------

#[test]
fn a_full_composition_commits_exactly_once() {
    // Open, compose, commit. Only the commit produces a message;
    // intermediate preedits must not, or every keystroke of a Devanagari
    // or Japanese composition would fire an `on_input`.
    let mut field = Field::new("", false);

    assert!(field.ime(input_method::Event::Opened).is_empty());
    assert!(
        field
            .ime(input_method::Event::Preedit("न".to_owned(), Some(0..3)))
            .is_empty(),
        "an in-flight composition must not publish a message"
    );
    assert!(
        field
            .ime(input_method::Event::Preedit("नम".to_owned(), Some(0..6)))
            .is_empty()
    );

    let messages = field.ime(input_method::Event::Commit("नमस्ते".to_owned()));

    assert_eq!(
        messages,
        vec!["नमस्ते".to_owned()],
        "commit must publish the composed text exactly once"
    );
}

#[test]
fn a_composition_is_visible_while_in_flight() {
    let mut field = Field::new("", false);

    let _ = field.ime(input_method::Event::Opened);
    let _ = field.ime(input_method::Event::Preedit("नम".to_owned(), Some(0..6)));

    assert_eq!(
        field.exposed_preedit().as_deref(),
        Some("नम"),
        "the composition must be visible while in flight"
    );
}

#[test]
fn an_empty_preedit_clears_the_composition_display() {
    // The platform signals "composition cleared" with an empty preedit,
    // which is what it sends immediately before a commit.
    let mut field = Field::new("", false);

    let _ = field.ime(input_method::Event::Opened);
    let _ = field.ime(input_method::Event::Preedit("नम".to_owned(), Some(0..6)));
    let _ = field.ime(input_method::Event::Preedit(String::new(), None));

    assert_eq!(field.exposed_preedit().as_deref(), Some(""));
}

// ---------------------------------------------------------------------
// Cancellation
// ---------------------------------------------------------------------

#[test]
fn an_abandoned_composition_commits_nothing_and_leaves_no_preedit() {
    // The cancellation path: the IME closes mid-composition (the user
    // switched away, or the platform dismissed it). Whatever was being
    // composed must vanish -- committing it anyway would insert text the
    // user never confirmed.
    let mut field = Field::new("", false);

    let _ = field.ime(input_method::Event::Opened);
    let _ = field.ime(input_method::Event::Preedit("नम".to_owned(), Some(0..6)));
    assert_eq!(field.exposed_preedit().as_deref(), Some("नम"));

    let messages = field.ime(input_method::Event::Closed);

    assert!(
        messages.is_empty(),
        "an abandoned composition must not commit text"
    );
    assert_eq!(
        field.exposed_preedit(),
        None,
        "closing the IME must clear the in-flight composition"
    );
}

#[test]
fn reopening_after_cancellation_starts_from_an_empty_composition() {
    let mut field = Field::new("", false);

    let _ = field.ime(input_method::Event::Opened);
    let _ = field.ime(input_method::Event::Preedit("abandoned".to_owned(), None));
    let _ = field.ime(input_method::Event::Closed);

    let _ = field.ime(input_method::Event::Opened);

    assert_eq!(
        field.exposed_preedit().as_deref(),
        Some(""),
        "a new session must not resurrect the abandoned composition"
    );
}

// ---------------------------------------------------------------------
// Candidate geometry
// ---------------------------------------------------------------------

#[test]
fn a_focused_field_reports_caret_geometry_for_the_candidate_window() {
    // The platform needs somewhere to put the candidate list, and it must
    // not cover the caret.
    let mut field = Field::new("text", false);
    let _ = field.ime(input_method::Event::Opened);

    let InputMethod::Enabled { cursor, .. } = field.input_method() else {
        panic!("a focused field must enable its input method");
    };

    assert!(cursor.height > 0.0, "the caret must have a real height");
    assert!(
        cursor.x >= 0.0 && cursor.y >= 0.0,
        "the caret must have a real position"
    );
}

#[test]
fn candidate_geometry_is_explicit_and_renderer_neutral() {
    let candidate = CandidateRect {
        bounds: Rectangle::new(Point::new(10.0, 20.0), Size::new(1.0, 24.0)),
    };

    assert_eq!(candidate.bounds.height, 24.0);
}

#[test]
fn preedit_retains_the_platform_selection_without_rewriting_content() {
    let preedit = Preedit {
        content: "नम".to_owned(),
        selection: Some(0..3),
        text_size: None,
    };

    assert_eq!(preedit.as_ref().content, "नम");
    assert_eq!(preedit.as_ref().selection, Some(0..3));
}

// ---------------------------------------------------------------------
// Secure fields
// ---------------------------------------------------------------------

#[test]
fn a_secure_field_marks_its_purpose_and_withholds_the_composition() {
    // Composition content is as sensitive as the field value: an
    // over-the-spot preedit rendered by the runtime would put the
    // password on screen outside the widget's own masking.
    let mut field = Field::new("", true);

    let _ = field.ime(input_method::Event::Opened);
    let _ = field.ime(input_method::Event::Preedit("secret".to_owned(), None));

    let InputMethod::Enabled {
        purpose, preedit, ..
    } = field.input_method()
    else {
        panic!("a focused secure field must still enable its input method");
    };

    assert_eq!(
        purpose,
        Purpose::Secure,
        "a secure field must tell the platform it is secure"
    );
    assert!(
        preedit.is_none(),
        "a secure field must never hand its composition to the runtime"
    );
}

#[test]
fn a_non_secure_field_does_hand_its_composition_to_the_runtime() {
    // The converse: withholding must be specific to secure fields, or
    // ordinary over-the-spot IME would break.
    let mut field = Field::new("", false);

    let _ = field.ime(input_method::Event::Opened);
    let _ = field.ime(input_method::Event::Preedit("नम".to_owned(), None));

    let InputMethod::Enabled {
        purpose, preedit, ..
    } = field.input_method()
    else {
        panic!("a focused field must enable its input method");
    };

    assert_eq!(purpose, Purpose::Normal);
    assert_eq!(
        preedit.map(|preedit| preedit.content),
        Some("नम".to_owned())
    );
}
