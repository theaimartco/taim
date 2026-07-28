//! Privacy classification, and who is allowed to see what.
//!
//! # The problem this solves
//!
//! A password is not secret because the text field remembers to hide it.
//! It is secret in the screenshot, in the window-overview thumbnail, in
//! the profiler trace, in the accessibility tree, on the lock screen, and
//! in the clipboard — and every one of those is a different subsystem,
//! written by someone who was not thinking about passwords that day.
//!
//! Phase 17 asks for "privacy classification propagated to render/tool/
//! platform paths". [`Privacy`] is that classification, and [`Audience`]
//! is the set of paths it has to survive. One vocabulary, so that "can
//! this be shown?" is asked and answered the same way everywhere instead
//! of being re-decided, differently, in six places.
//!
//! # Why the default is the *un*safe direction, and why that is fine
//!
//! [`Privacy::Public`] is [`Default`]. That looks wrong for a security
//! type — surely the default should be the restrictive one? — but the
//! alternative is worse in practice: if unclassified content defaulted to
//! secret, an application that never classified anything would show blank
//! screenshots, blank thumbnails, and an empty accessibility tree, and the
//! first thing anyone would do is turn the whole mechanism off.
//!
//! The safety comes from the other end: classification **propagates
//! upward with the most restrictive winning** ([`Privacy::max`]), so a
//! single secret anywhere in a subtree makes the whole subtree secret.
//! Marking one field is enough; forgetting to mark the container is not a
//! hole.
//!
//! # The rule that has no exceptions
//!
//! > Never expose protected pixels through fallback tooling.
//!
//! [`Privacy::Protected`] is compositor-protected content — DRM video.
//! Unlike everything else here, the framework does not *have* those
//! pixels: the compositor never handed them over. A tool that "falls back"
//! to reading them anyway is not degraded, it is broken, which is why
//! [`Privacy::allows`] refuses every visual audience for it
//! unconditionally rather than consulting a policy.

/// How sensitive some content is.
///
/// Ordered from least to most restrictive, so [`Ord`] means "at least as
/// restrictive as" and [`Privacy::max`] is propagation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Privacy {
    /// Ordinary content. Anyone may see it.
    #[default]
    Public,

    /// Content withheld from a context the user is not present for — a
    /// notification body on a lock screen, a message preview in an
    /// overview thumbnail.
    ///
    /// Still available to assistive technology and to the user's own
    /// tools: it is *contextually* private, not secret. A screen reader
    /// serving the person holding the unlocked device must read it, or
    /// the feature is inaccessible rather than private.
    Private,

    /// Secure text — a password, a recovery phrase.
    ///
    /// Never leaves the widget that owns it. Not to a screenshot, not to
    /// a trace, not to a screen reader, not to the clipboard. The one
    /// audience it survives is [`Audience::Render`]: the field has to draw
    /// *something*, and that something is the widget's own business (a
    /// row of dots).
    Secret,

    /// Compositor-protected pixels — DRM video.
    ///
    /// The framework does not hold this content: the compositor never
    /// handed it over. No fallback path can produce it, and any that
    /// appears to is a bug rather than a feature.
    Protected,
}

impl Privacy {
    /// The more restrictive of two classifications.
    ///
    /// This is how classification propagates up a tree: a container is as
    /// private as its most private descendant. That direction is the whole
    /// safety argument — marking the password field is enough, and
    /// forgetting to mark the dialog around it is not a hole.
    #[must_use]
    pub fn max(self, other: Self) -> Self {
        Ord::max(self, other)
    }

    /// The most restrictive classification among `items`.
    ///
    /// [`Privacy::Public`] for an empty iterator: a container of nothing
    /// hides nothing.
    pub fn of(items: impl IntoIterator<Item = Self>) -> Self {
        items.into_iter().fold(Self::Public, Self::max)
    }

    /// Whether content of this classification may be shown to `audience`.
    ///
    /// The single question every consumer asks. Written as one table
    /// rather than six scattered `if`s, because the failure mode of the
    /// scattered version is that five of them agree and the sixth leaks.
    pub fn allows(self, audience: Audience) -> bool {
        match self {
            // Nothing is withheld from anyone.
            Privacy::Public => true,

            Privacy::Private => match audience {
                // The user is present and looking at their own screen.
                Audience::Render => true,
                // The user is present and listening to their own screen
                // reader. Withholding here would make the feature
                // inaccessible rather than private -- the distinction
                // `Private` exists to draw.
                Audience::Semantics => true,
                // The user asked for this, explicitly, right now.
                Audience::Clipboard => true,
                // Nobody is looking. That is what `Private` means.
                Audience::LockScreen => false,
                // A screenshot outlives the moment and the room.
                Audience::Screenshot => false,
                // A thumbnail is shown in a switcher, over the shoulder.
                Audience::Overview => false,
                // A trace is a file that gets attached to a bug report.
                Audience::Profiler => false,
            },

            // The owning widget draws it (as dots); nothing else sees it.
            Privacy::Secret => audience == Audience::Render,

            // The framework does not have these pixels. There is no
            // audience to consult a policy for -- see the module docs.
            Privacy::Protected => false,
        }
    }

    /// Whether this classification withholds anything from anyone.
    pub fn is_public(self) -> bool {
        self == Privacy::Public
    }
}

/// Somewhere content might end up.
///
/// Not "a permission level" — these are the actual paths in the system
/// that can put content in front of someone, and they differ in *who* is
/// looking and *for how long*, which is what makes the answers differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Audience {
    /// Drawn to the surface the user is looking at.
    Render,

    /// Drawn to the lock screen, which the user is by definition not
    /// present for.
    LockScreen,

    /// Captured to an image that outlives the moment.
    Screenshot,

    /// A window-overview or task-switcher thumbnail.
    Overview,

    /// A profiler trace — a file that ends up attached to a bug report.
    Profiler,

    /// The accessibility tree.
    Semantics,

    /// The clipboard.
    Clipboard,
}

impl Audience {
    /// Every audience, for exhaustive checks.
    pub const ALL: [Audience; 7] = [
        Audience::Render,
        Audience::LockScreen,
        Audience::Screenshot,
        Audience::Overview,
        Audience::Profiler,
        Audience::Semantics,
        Audience::Clipboard,
    ];
}

/// Content with its classification attached.
///
/// The point is that the two travel together. A `String` that was secret
/// where it came from is just a `String` by the time it reaches a
/// screenshot encoder, and the encoder has no way to know — so the
/// classification has to be carried, not remembered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Classified<T> {
    content: T,
    privacy: Privacy,
}

impl<T> Classified<T> {
    /// Classifies `content`.
    pub fn new(content: T, privacy: Privacy) -> Self {
        Self { content, privacy }
    }

    /// Classifies `content` as [`Privacy::Public`].
    pub fn public(content: T) -> Self {
        Self::new(content, Privacy::Public)
    }

    /// Classifies `content` as [`Privacy::Secret`].
    pub fn secret(content: T) -> Self {
        Self::new(content, Privacy::Secret)
    }

    /// How sensitive this is.
    pub fn privacy(&self) -> Privacy {
        self.privacy
    }

    /// The content, if `audience` may see it.
    ///
    /// The only way to get it out. There is deliberately no
    /// `Classified::into_inner`: an escape hatch that skips the check is
    /// the thing every leak is eventually traced back to, and "just this
    /// once" is how it gets used.
    pub fn reveal_to(&self, audience: Audience) -> Option<&T> {
        self.privacy.allows(audience).then_some(&self.content)
    }

    /// Applies `f` to the content, keeping the classification.
    ///
    /// For a transformation that cannot leak — measuring a length,
    /// counting lines. Whatever comes out is as private as what went in,
    /// because a derived value of a secret is a secret.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Classified<U> {
        Classified {
            content: f(self.content),
            privacy: self.privacy,
        }
    }

    /// Makes this at least as restrictive as `privacy`.
    ///
    /// Only ever tightens. There is no `declassify`: content does not
    /// become less secret by being moved, and a method that said otherwise
    /// would be used to work around the type rather than with it.
    #[must_use]
    pub fn restrict(mut self, privacy: Privacy) -> Self {
        self.privacy = self.privacy.max(privacy);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_propagates_upward_with_the_most_restrictive_winning() {
        // The whole safety argument: marking the password field is enough,
        // and forgetting to mark the dialog around it is not a hole.
        let dialog = Privacy::of([Privacy::Public, Privacy::Secret, Privacy::Public]);

        assert_eq!(dialog, Privacy::Secret);
    }

    #[test]
    fn a_container_of_nothing_hides_nothing() {
        assert_eq!(Privacy::of([]), Privacy::Public);
    }

    #[test]
    fn protected_outranks_everything() {
        assert_eq!(
            Privacy::of([Privacy::Secret, Privacy::Protected]),
            Privacy::Protected
        );
        assert_eq!(Privacy::Public.max(Privacy::Protected), Privacy::Protected);
    }

    #[test]
    fn a_secret_reaches_no_audience_but_the_widget_drawing_it() {
        // A password field still has to draw a row of dots. Everything
        // else -- screenshot, thumbnail, trace, screen reader, clipboard --
        // sees nothing.
        for audience in Audience::ALL {
            assert_eq!(
                Privacy::Secret.allows(audience),
                audience == Audience::Render,
                "a secret must not reach {audience:?}"
            );
        }
    }

    #[test]
    fn protected_pixels_reach_nobody_at_all() {
        // The rule with no exceptions. Note this includes `Render`: the
        // framework does not have these pixels, so there is nothing for it
        // to draw. The compositor composites them itself.
        for audience in Audience::ALL {
            assert!(
                !Privacy::Protected.allows(audience),
                "protected content must not reach {audience:?} -- there is no fallback path \
                 that can produce pixels the compositor never handed over"
            );
        }
    }

    #[test]
    fn private_content_is_withheld_from_the_lock_screen_but_not_from_its_owner() {
        // The distinction `Private` exists to draw: nobody is looking at a
        // lock screen, but the person holding the unlocked device is
        // looking at -- or listening to -- their own.
        assert!(!Privacy::Private.allows(Audience::LockScreen));
        assert!(Privacy::Private.allows(Audience::Render));
    }

    #[test]
    fn private_content_reaches_a_screen_reader() {
        // Withholding here would make the feature inaccessible rather than
        // private. A notification a sighted user can read and a blind user
        // cannot is not a privacy win.
        assert!(Privacy::Private.allows(Audience::Semantics));
    }

    #[test]
    fn private_content_never_reaches_a_trace_or_a_thumbnail() {
        // A trace is a file that gets attached to a bug report; a
        // thumbnail is shown in a switcher, over someone's shoulder.
        assert!(!Privacy::Private.allows(Audience::Profiler));
        assert!(!Privacy::Private.allows(Audience::Overview));
        assert!(!Privacy::Private.allows(Audience::Screenshot));
    }

    #[test]
    fn public_content_reaches_everyone() {
        for audience in Audience::ALL {
            assert!(Privacy::Public.allows(audience));
        }
    }

    #[test]
    fn a_secret_cannot_be_read_without_naming_an_audience() {
        // There is no `into_inner`. The only way out is a check, which is
        // what stops a leak from being one careless line.
        let password = Classified::secret("hunter2");

        assert_eq!(password.reveal_to(Audience::Render), Some(&"hunter2"));
        assert_eq!(password.reveal_to(Audience::Screenshot), None);
        assert_eq!(password.reveal_to(Audience::Profiler), None);
        assert_eq!(password.reveal_to(Audience::Clipboard), None);
    }

    #[test]
    fn a_derived_value_of_a_secret_is_a_secret() {
        // The classic leak: measure the password, publish the length.
        let length = Classified::secret("hunter2").map(str::len);

        assert_eq!(length.privacy(), Privacy::Secret);
        assert_eq!(length.reveal_to(Audience::Profiler), None);
    }

    #[test]
    fn restricting_only_ever_tightens() {
        // There is no `declassify`. Content does not become less secret by
        // being moved, and a method that said otherwise would be used to
        // work around the type rather than with it.
        let secret = Classified::secret("hunter2").restrict(Privacy::Public);

        assert_eq!(
            secret.privacy(),
            Privacy::Secret,
            "restricting to Public must not declassify"
        );

        let escalated = Classified::public("hello").restrict(Privacy::Protected);

        assert_eq!(escalated.privacy(), Privacy::Protected);
    }

    #[test]
    fn ordering_means_at_least_as_restrictive_as() {
        // `Ord` is load-bearing: `max` is propagation, and a reordering of
        // the variants would silently invert every policy in the system.
        assert!(Privacy::Public < Privacy::Private);
        assert!(Privacy::Private < Privacy::Secret);
        assert!(Privacy::Secret < Privacy::Protected);
    }

    #[test]
    fn the_default_is_public() {
        // Deliberate, and argued in the module docs: defaulting to secret
        // would give an unclassified application blank screenshots and an
        // empty accessibility tree, and the mechanism would be turned off
        // within a day. Safety comes from upward propagation instead.
        assert_eq!(Privacy::default(), Privacy::Public);
        assert!(Privacy::default().is_public());
    }
}
