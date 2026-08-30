//! What the profiler records, and what it is structurally unable to
//! record.
//!
//! # Privacy is a type, not a filter
//!
//! `phase.md` requires that "secure text, private notifications,
//! protected surfaces, clipboard contents, and raw images are redacted by
//! default". A redaction *pass* — walk the buffer, blank the sensitive
//! fields — is the obvious design and the wrong one: it fails open. Every
//! new event kind is a leak until someone remembers to teach the pass
//! about it, and nothing fails when they forget.
//!
//! So an [`Event`] cannot hold user content at all. The only free-form
//! string it can carry is a [`Label`], and a [`Label`] is a `&'static
//! str` — it can only come from the program text, never from a
//! `TextInput`'s buffer, a clipboard offer, or a decoded image. There is
//! no `Label::from(String)`, deliberately.
//!
//! Where an event genuinely needs to name user data — the widget
//! inspector wants to show *which* text field — it carries a
//! [`Redacted`], which stores a length and a classification and never the
//! bytes.
//!
//! The [`export`](crate::export) tests treat "no user content can be
//! constructed into an event" as the invariant, rather than "no user
//! content survives export". The first is checkable by reading the type;
//! the second needs a test per field, forever.
//!
//! # No widget identities
//!
//! An earlier draft carried an `Event::Widget { id: Id, .. }`. It is gone
//! for three reasons that turned out to be the same reason: `Id` holds a
//! [`SmolStr`](taim_core::SmolStr), so it is not `Copy`, and cloning a
//! long one **allocates on the recording path** — which is exactly what a
//! profiler must not do. It is also a channel for content: `Id::new` takes
//! a `&'static str` today, but nothing about the type says a future
//! keyed-list identity could not be derived from user data.
//!
//! Nothing is lost. `phase.md` asks this module for "CPU/GPU/layout/cache/
//! input/semantics events" — a timeline of *phases and costs*. Which
//! widget did what is a question for the live tree in Taim Lab, which can
//! answer it without a per-frame ring buffer carrying identities.

/// A name that can only have come from the program's own source.
///
/// The whole privacy story rests on this being `&'static str`: a string
/// that lives for the program's lifetime cannot be a keystroke, a
/// clipboard paste, or a decoded pixel. Anything runtime-derived would
/// need an allocation, and there is nowhere to put one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Label(&'static str);

impl Label {
    /// Names something after a literal in the source.
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// The name.
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

impl From<&'static str> for Label {
    fn from(name: &'static str) -> Self {
        Self::new(name)
    }
}

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// How sensitive some content is.
///
/// Re-exported from [`taim_core::security`], not redefined. An earlier
/// draft of this module had its own `Sensitivity` enum with the same four
/// variants — which is exactly the failure `security`'s own docs warn
/// about: two vocabularies for one question is how five paths agree and
/// the sixth leaks. A profiler is one of the paths a classification has to
/// survive, not a place to invent a second one.
pub use taim_core::security::Privacy as Sensitivity;

/// A reference to user content that carries none of it.
///
/// Holds what a profiler legitimately needs — that there *is* content,
/// and roughly how much — and nothing a leak could be made of. There is
/// no constructor taking the content itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Redacted {
    /// How many bytes the content had.
    ///
    /// Withheld for [`Sensitivity::Secret`]: the length of a password is
    /// worth real bits to an attacker, and no profiling question needs it.
    length: Option<usize>,
    /// How sensitive it was.
    sensitivity: Sensitivity,
}

impl Redacted {
    /// Records that content of `length` bytes existed, at `sensitivity`.
    ///
    /// The content itself is not a parameter. That is the point: a caller
    /// cannot pass it in by mistake.
    pub fn new(length: usize, sensitivity: Sensitivity) -> Self {
        Self {
            // Narrower than `Privacy::allows(Audience::Profiler)`, and
            // deliberately so: that answers whether the *content* may be
            // shown, and the answer is no for everything but `Public`.
            // This is about the length, which is metadata. A notification's
            // size and a video frame's size are not secrets; a password's
            // is worth real bits to an attacker, and no profiling question
            // needs it.
            length: (sensitivity != Sensitivity::Secret).then_some(length),
            sensitivity,
        }
    }

    /// How many bytes the content had, when that is safe to say.
    pub fn length(&self) -> Option<usize> {
        self.length
    }

    /// How sensitive the content was.
    pub fn sensitivity(&self) -> Sensitivity {
        self.sensitivity
    }
}

/// Which part of a frame an event belongs to.
///
/// The exit gate — "every global metric can be traced to a phase/cause" —
/// is this enum's reason to exist: a frame time that cannot be attributed
/// to a phase is a number a developer can do nothing with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    /// Turning events into messages.
    Input,
    /// Rebuilding the view.
    View,
    /// Reconciling the widget tree.
    Diff,
    /// Measuring and placing widgets.
    Layout,
    /// Working out what changed on screen.
    Damage,
    /// Recording draw commands.
    Paint,
    /// Handing work to the GPU.
    Submit,
    /// Waiting for the GPU and the compositor.
    Present,
    /// Assembling the semantic tree.
    Semantics,
    /// Stepping animations.
    Animation,
}

impl Phase {
    /// The phase's name, for export.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Phase::Input => "input",
            Phase::View => "view",
            Phase::Diff => "diff",
            Phase::Layout => "layout",
            Phase::Damage => "damage",
            Phase::Paint => "paint",
            Phase::Submit => "submit",
            Phase::Present => "present",
            Phase::Semantics => "semantics",
            Phase::Animation => "animation",
        }
    }
}

/// Why a frame took longer than its budget.
///
/// A missed frame with no cause is a bug report nobody can act on, which
/// is why [`Missed`](Event::Missed) requires one rather than defaulting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// A phase overran.
    Phase(Phase),
    /// A cache miss forced work that is normally reused.
    CacheMiss(Resource),
    /// The GPU did not finish in time.
    GpuStall,
    /// The application's own update took too long.
    Application,
    /// The compositor did not hand back a buffer in time.
    Compositor,
}

/// A kind of resource a cache holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Resource {
    /// Shaped text.
    Text,
    /// A decoded image.
    Image,
    /// A retained layer.
    Layer,
    /// A rendered effect.
    Effect,
    /// A glyph atlas.
    Atlas,
    /// A GPU buffer.
    Buffer,
    /// A render pipeline.
    Pipeline,
}

impl Resource {
    /// The resource's name, for export.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Resource::Text => "text",
            Resource::Image => "image",
            Resource::Layer => "layer",
            Resource::Effect => "effect",
            Resource::Atlas => "atlas",
            Resource::Buffer => "buffer",
            Resource::Pipeline => "pipeline",
        }
    }
}

/// Nanoseconds since the collector started.
///
/// Relative, not wall-clock: an absolute timestamp is a fingerprint, and
/// a trace only ever needs to answer "how long" and "in what order".
pub type Nanos = u64;

/// One thing worth knowing about.
///
/// Deliberately `Copy` and small. A profiler that allocates to record is
/// a profiler that changes what it measures — and one that can hold an
/// owned `String` is one that can hold a password.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// A frame began.
    FrameBegin {
        /// Which frame.
        frame: u64,
        /// When.
        at: Nanos,
    },
    /// A frame ended.
    FrameEnd {
        /// Which frame.
        frame: u64,
        /// When.
        at: Nanos,
    },
    /// A phase began.
    PhaseBegin {
        /// Which phase.
        phase: Phase,
        /// When.
        at: Nanos,
    },
    /// A phase ended.
    PhaseEnd {
        /// Which phase.
        phase: Phase,
        /// When.
        at: Nanos,
    },
    /// A frame missed its budget.
    Missed {
        /// Which frame.
        frame: u64,
        /// By how much.
        over: Nanos,
        /// Why — required, because a missed frame with no cause is a bug
        /// report nobody can act on.
        cause: Cause,
    },
    /// A cache was asked for something it had.
    CacheHit {
        /// What kind.
        resource: Resource,
        /// When.
        at: Nanos,
    },
    /// A cache was asked for something it had to make.
    CacheMiss {
        /// What kind.
        resource: Resource,
        /// When.
        at: Nanos,
        /// How long making it took.
        cost: Nanos,
    },
    /// A cache gave something up under pressure.
    Evicted {
        /// What kind.
        resource: Resource,
        /// How many bytes came back.
        bytes: u64,
        /// When.
        at: Nanos,
    },
    /// Input arrived.
    Input {
        /// What kind — a `&'static str` from the event type's own name,
        /// never its contents.
        kind: Label,
        /// When.
        at: Nanos,
    },
    /// A semantic tree was assembled.
    Semantics {
        /// How many nodes it had.
        nodes: u32,
        /// How many were rebuilt rather than reused.
        rebuilt: u32,
        /// When.
        at: Nanos,
    },
    /// Content passed through a path a profiler can see, and was withheld.
    ///
    /// Recorded so a developer can tell "nothing happened" from "something
    /// happened that you are not allowed to see" — an absent event would
    /// look like a bug in the profiler.
    Withheld {
        /// Where.
        at: Nanos,
        /// What was withheld.
        content: Redacted,
    },
}

impl Event {
    /// When this event happened.
    pub fn at(&self) -> Nanos {
        match self {
            Event::FrameBegin { at, .. }
            | Event::FrameEnd { at, .. }
            | Event::PhaseBegin { at, .. }
            | Event::PhaseEnd { at, .. }
            | Event::CacheHit { at, .. }
            | Event::CacheMiss { at, .. }
            | Event::Evicted { at, .. }
            | Event::Input { at, .. }
            | Event::Semantics { at, .. }
            | Event::Withheld { at, .. } => *at,
            // A miss is reported against the frame it belongs to, and
            // ordering it by its own timestamp would put it after the
            // `FrameEnd` it explains.
            Event::Missed { .. } => 0,
        }
    }

    /// Which phase this event belongs to, when it belongs to one.
    pub fn phase(&self) -> Option<Phase> {
        match self {
            Event::PhaseBegin { phase, .. } | Event::PhaseEnd { phase, .. } => Some(*phase),
            Event::Missed {
                cause: Cause::Phase(phase),
                ..
            } => Some(*phase),
            Event::Semantics { .. } => Some(Phase::Semantics),
            Event::Input { .. } => Some(Phase::Input),
            _ => None,
        }
    }
}

/// The event schema's version, carried in every export.
///
/// A trace outlives the build that made it — a bug report from a user's
/// device is read weeks later by a different version of the tools. A
/// reader that cannot tell which schema it is looking at has to guess.
pub const SCHEMA_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_redaction_withholds_even_the_length() {
        // A password's length is worth real bits to an attacker, and no
        // profiling question needs it.
        let redacted = Redacted::new(12, Sensitivity::Secret);

        assert_eq!(redacted.length(), None);
        assert_eq!(redacted.sensitivity(), Sensitivity::Secret);
    }

    #[test]
    fn a_public_redaction_keeps_the_length_but_never_the_bytes() {
        // There is no constructor taking the content, so "never the
        // bytes" is a property of the type rather than of this test.
        let redacted = Redacted::new(12, Sensitivity::Public);

        assert_eq!(redacted.length(), Some(12));
    }

    #[test]
    fn private_and_protected_content_keeps_its_length() {
        // Unlike a password, the size of a notification or a video frame
        // is not itself a secret, and a developer chasing a memory
        // question needs it.
        for sensitivity in [Sensitivity::Private, Sensitivity::Protected] {
            assert_eq!(Redacted::new(4096, sensitivity).length(), Some(4096));
        }
    }

    #[test]
    fn no_classification_lets_content_itself_into_a_trace() {
        // The classification is shared with the rest of the framework, so
        // this is worth pinning here: a trace is not an audience any
        // non-public content is shown to.
        use taim_core::security::Audience;

        for sensitivity in [
            Sensitivity::Private,
            Sensitivity::Secret,
            Sensitivity::Protected,
        ] {
            assert!(
                !sensitivity.allows(Audience::Profiler),
                "{sensitivity:?} must not be shown to a trace"
            );
        }
    }

    #[test]
    fn a_miss_is_ordered_with_the_frame_it_explains() {
        // Ordering it by its own timestamp would place it after the
        // `FrameEnd` it is the explanation for.
        let missed = Event::Missed {
            frame: 7,
            over: 3_000_000,
            cause: Cause::GpuStall,
        };

        assert_eq!(missed.at(), 0);
    }

    #[test]
    fn events_report_the_phase_they_belong_to() {
        assert_eq!(
            Event::PhaseBegin {
                phase: Phase::Layout,
                at: 0
            }
            .phase(),
            Some(Phase::Layout)
        );

        assert_eq!(
            Event::Missed {
                frame: 1,
                over: 1,
                cause: Cause::Phase(Phase::Paint),
            }
            .phase(),
            Some(Phase::Paint),
            "a missed frame must be traceable to the phase that overran"
        );

        assert_eq!(
            Event::CacheHit {
                resource: Resource::Text,
                at: 0
            }
            .phase(),
            None,
            "a cache hit belongs to whatever phase asked; it does not claim one"
        );
    }

    #[test]
    fn an_event_is_small_and_copy_so_recording_never_allocates() {
        // A profiler that allocates to record changes what it measures.
        // The bound is generous; what matters is that it has one, and that
        // growing `Event` past it is a decision rather than an accident.
        assert!(
            size_of::<Event>() <= 64,
            "Event grew to {} bytes; recording must stay allocation-free \
             and cache-friendly",
            size_of::<Event>()
        );
    }

    #[test]
    fn a_label_can_only_come_from_the_source() {
        // The privacy story rests on this. If `Label` ever gains a
        // `From<String>`, a keystroke becomes one line away from a trace.
        let label = Label::from("pointer_moved");

        assert_eq!(label.as_str(), "pointer_moved");

        // This is the real assertion, and the compiler makes it: there is
        // no way to write `Label::new(user_text)` for a runtime `String`.
        // A test cannot check for the absence of an impl, so it is stated
        // here for the next person to read before adding one.
    }
}
