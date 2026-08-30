//! Pre/post consumption, child/parent handoff, and overscroll for nested
//! scrollable regions.
//!
//! A scroll delta arriving at a nested scrollable (a horizontally
//! scrolling carousel inside a vertically scrolling page, a chat
//! transcript inside a resizable panel) needs a clear, deterministic
//! protocol for "who gets to consume how much of this," or one of two
//! failure modes results: the child eats every delta forever, starving
//! the parent (the exact bug this phase's `widget::Scrollable` fix
//! addressed for a single, non-nested case: a saturated scrollable kept
//! capturing events via a stale multi-second "transaction" window even
//! once it had nothing left to do with them); or the parent eats
//! everything, and the child never scrolls at all.
//!
//! [`Coordinator`] is the reusable protocol both failure modes are
//! avoided by: [`Coordinator::dispatch`] gives a child first crack at a
//! [`Delta`] ("pre-consumption" in the sense that the child, being
//! deeper in the tree, decides before its ancestors do), reports how
//! much of it the child could not use, tracks [`Coordinator::overscroll`]
//! for the rubber-band effect real scrollables use to communicate "you
//! have reached the end," and exposes exactly the remainder a parent
//! should apply to itself ("post-consumption").
use taim_core::Vector;

/// A scroll delta being routed through a nested scroll hierarchy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Delta {
    /// The requested movement.
    pub amount: Vector,
}

/// The result of offering a [`Delta`] to one level of a nested scroll
/// hierarchy via [`Coordinator::dispatch`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Consumption {
    /// How much of the offered delta this level actually consumed
    /// (applied to its own scroll offset).
    pub consumed: Vector,

    /// How much is left over for a parent (or, if this was already the
    /// outermost level, simply unconsumed) -- always `delta.amount -
    /// consumed`, provided as a field rather than something every caller
    /// has to recompute.
    pub remainder: Vector,
}

/// The scrollable extent of one level in a nested scroll hierarchy, in
/// both axes: how far its content can move before reaching either edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent {
    /// The minimum offset (typically `Vector::ZERO`, the start).
    pub min: Vector,
    /// The maximum offset the content can be scrolled to (the end).
    pub max: Vector,
}

impl Extent {
    /// Clamps `offset` to this [`Extent`]'s `[min, max]` range,
    /// component-wise.
    pub fn clamp(&self, offset: Vector) -> Vector {
        Vector::new(
            offset.x.clamp(self.min.x, self.max.x),
            offset.y.clamp(self.min.y, self.max.y),
        )
    }
}

/// One level of a nested scroll hierarchy: tracks its own current
/// offset and [`Extent`], consuming as much of a dispatched [`Delta`] as
/// its remaining scrollable range allows, and reporting the rest as a
/// remainder for a parent [`Coordinator`] to apply to itself.
///
/// A hierarchy of nested scrollables is simply a chain of
/// [`Coordinator`]s, each one's [`Coordinator::dispatch`] called with
/// whatever [`Consumption::remainder`] the level below it reported --
/// the child dispatches first (pre-consumption), then whatever it could
/// not use flows outward to each ancestor in turn (post-consumption),
/// exactly matching real touch-scrolling platforms' own nested-scroll
/// contracts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coordinator {
    offset: Vector,
    extent: Extent,
    overscroll: Vector,
}

impl Coordinator {
    /// Creates a [`Coordinator`] at zero offset and zero overscroll,
    /// for the given [`Extent`].
    pub fn new(extent: Extent) -> Self {
        Self {
            offset: Vector::ZERO,
            extent,
            overscroll: Vector::ZERO,
        }
    }

    /// Returns the current scroll offset.
    pub fn offset(&self) -> Vector {
        self.offset
    }

    /// Returns the current overscroll: how far past its own [`Extent`]
    /// this level has been dragged, component-wise (positive past
    /// `extent.max`, negative past `extent.min`, zero within bounds).
    ///
    /// A real scrollable widget uses this to render the familiar
    /// rubber-band stretch/bounce-back effect; [`Coordinator`] itself
    /// only tracks the value; how (or whether) it is animated back to
    /// zero on release is the caller's concern.
    pub fn overscroll(&self) -> Vector {
        self.overscroll
    }

    /// Updates the [`Extent`] this [`Coordinator`] scrolls within (e.g.
    /// after a resize or content-length change), clamping the current
    /// offset to stay valid.
    pub fn set_extent(&mut self, extent: Extent) {
        self.extent = extent;
        self.offset = extent.clamp(self.offset);
    }

    /// Dispatches `delta` to this level, consuming as much as its
    /// remaining scrollable range allows and reporting the
    /// [`Consumption`]: what was actually applied to
    /// [`Coordinator::offset`], and what remains for a parent level.
    ///
    /// `allow_overscroll` lets a caller enable the rubber-band case (the
    /// gesture is still in progress, an active touch drag past the edge
    /// should visually give a little) versus not (a released fling
    /// should stop cleanly at the boundary rather than accumulate
    /// overscroll it will never get to animate back from, since nothing
    /// is still dragging it).
    pub fn dispatch(&mut self, delta: Delta, allow_overscroll: bool) -> Consumption {
        let unclamped = self.offset + delta.amount;
        let clamped = self.extent.clamp(unclamped);

        let consumed = clamped - self.offset;
        let remainder = delta.amount - consumed;

        self.offset = clamped;

        if allow_overscroll {
            self.overscroll = unclamped - clamped;

            // Overscroll silently "consumes" the remainder that would
            // otherwise flow to the parent -- it is being spent on the
            // rubber-band stretch, not left over for anything else to
            // also apply.
            Consumption {
                consumed: delta.amount,
                remainder: Vector::ZERO,
            }
        } else {
            self.overscroll = Vector::ZERO;

            Consumption {
                consumed,
                remainder,
            }
        }
    }

    /// Releases any accumulated overscroll back to zero, snapping the
    /// offset to the nearest valid boundary -- what a caller should call
    /// once the gesture driving [`Coordinator::dispatch`]'s
    /// `allow_overscroll = true` case ends (a finger lifts mid
    /// rubber-band), before any bounce-back animation of its own takes
    /// over from here.
    pub fn settle_overscroll(&mut self) {
        self.offset = self.extent.clamp(self.offset);
        self.overscroll = Vector::ZERO;
    }
}

/// Routes `delta` through a full hierarchy of [`Coordinator`]s, child
/// first, applying [`Coordinator::dispatch`] to each of `chain` in
/// order and returning the final [`Consumption`] left over after every
/// level has had a chance to consume it (an outermost, unconsumed
/// remainder past every ancestor is simply lost, exactly as scrolling
/// past the top-level window's own scrollable content is today).
///
/// `chain` must be ordered innermost (the deepest child) first,
/// outermost (the topmost ancestor) last -- the same order pre/post
/// consumption naturally implies.
pub fn dispatch_chain(
    chain: &mut [&mut Coordinator],
    mut delta: Delta,
    allow_overscroll: bool,
) -> Consumption {
    let mut total_consumed = Vector::ZERO;

    for coordinator in chain.iter_mut() {
        let consumption = coordinator.dispatch(delta, allow_overscroll);

        total_consumed += consumption.consumed;
        delta = Delta {
            amount: consumption.remainder,
        };

        if delta.amount == Vector::ZERO {
            break;
        }
    }

    Consumption {
        consumed: total_consumed,
        remainder: delta.amount,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extent(max_y: f32) -> Extent {
        Extent {
            min: Vector::ZERO,
            max: Vector::new(0.0, max_y),
        }
    }

    #[test]
    fn a_delta_within_the_extent_is_fully_consumed() {
        let mut coordinator = Coordinator::new(extent(500.0));

        let consumption = coordinator.dispatch(
            Delta {
                amount: Vector::new(0.0, 100.0),
            },
            false,
        );

        assert_eq!(consumption.consumed, Vector::new(0.0, 100.0));
        assert_eq!(consumption.remainder, Vector::ZERO);
        assert_eq!(coordinator.offset(), Vector::new(0.0, 100.0));
    }

    #[test]
    fn a_delta_beyond_the_extent_reports_a_remainder_without_overscroll() {
        let mut coordinator = Coordinator::new(extent(50.0));

        let consumption = coordinator.dispatch(
            Delta {
                amount: Vector::new(0.0, 100.0),
            },
            false,
        );

        assert_eq!(consumption.consumed, Vector::new(0.0, 50.0));
        assert_eq!(consumption.remainder, Vector::new(0.0, 50.0));
        assert_eq!(coordinator.offset(), Vector::new(0.0, 50.0));
        assert_eq!(coordinator.overscroll(), Vector::ZERO);
    }

    #[test]
    fn a_delta_beyond_the_extent_with_overscroll_allowed_consumes_everything() {
        let mut coordinator = Coordinator::new(extent(50.0));

        let consumption = coordinator.dispatch(
            Delta {
                amount: Vector::new(0.0, 100.0),
            },
            true,
        );

        assert_eq!(consumption.consumed, Vector::new(0.0, 100.0));
        assert_eq!(consumption.remainder, Vector::ZERO);
        assert_eq!(coordinator.offset(), Vector::new(0.0, 50.0));
        assert_eq!(coordinator.overscroll(), Vector::new(0.0, 50.0));
    }

    #[test]
    fn settle_overscroll_clamps_back_and_clears_overscroll() {
        let mut coordinator = Coordinator::new(extent(50.0));
        let _ = coordinator.dispatch(
            Delta {
                amount: Vector::new(0.0, 100.0),
            },
            true,
        );

        coordinator.settle_overscroll();

        assert_eq!(coordinator.offset(), Vector::new(0.0, 50.0));
        assert_eq!(coordinator.overscroll(), Vector::ZERO);
    }

    #[test]
    fn dispatch_chain_hands_off_the_remainder_from_child_to_parent() {
        let mut child = Coordinator::new(extent(30.0));
        let mut parent = Coordinator::new(extent(500.0));

        let consumption = dispatch_chain(
            &mut [&mut child, &mut parent],
            Delta {
                amount: Vector::new(0.0, 100.0),
            },
            false,
        );

        assert_eq!(child.offset(), Vector::new(0.0, 30.0));
        assert_eq!(parent.offset(), Vector::new(0.0, 70.0));
        assert_eq!(consumption.consumed, Vector::new(0.0, 100.0));
        assert_eq!(consumption.remainder, Vector::ZERO);
    }

    #[test]
    fn dispatch_chain_reports_a_remainder_past_every_level() {
        let mut child = Coordinator::new(extent(10.0));
        let mut parent = Coordinator::new(extent(20.0));

        let consumption = dispatch_chain(
            &mut [&mut child, &mut parent],
            Delta {
                amount: Vector::new(0.0, 100.0),
            },
            false,
        );

        assert_eq!(child.offset(), Vector::new(0.0, 10.0));
        assert_eq!(parent.offset(), Vector::new(0.0, 20.0));
        assert_eq!(consumption.consumed, Vector::new(0.0, 30.0));
        assert_eq!(consumption.remainder, Vector::new(0.0, 70.0));
    }

    #[test]
    fn dispatch_chain_stops_early_once_fully_consumed() {
        let mut child = Coordinator::new(extent(500.0));
        let mut parent = Coordinator::new(extent(500.0));

        let _ = dispatch_chain(
            &mut [&mut child, &mut parent],
            Delta {
                amount: Vector::new(0.0, 20.0),
            },
            false,
        );

        assert_eq!(child.offset(), Vector::new(0.0, 20.0));
        // The parent never needed to consume anything, since the child
        // took all of it -- its offset stays at zero.
        assert_eq!(parent.offset(), Vector::ZERO);
    }

    #[test]
    fn a_negative_delta_beyond_the_minimum_also_reports_a_remainder() {
        let mut coordinator = Coordinator::new(extent(500.0));
        let _ = coordinator.dispatch(
            Delta {
                amount: Vector::new(0.0, 20.0),
            },
            false,
        );

        let consumption = coordinator.dispatch(
            Delta {
                amount: Vector::new(0.0, -100.0),
            },
            false,
        );

        assert_eq!(consumption.consumed, Vector::new(0.0, -20.0));
        assert_eq!(consumption.remainder, Vector::new(0.0, -80.0));
        assert_eq!(coordinator.offset(), Vector::ZERO);
    }

    #[test]
    fn set_extent_clamps_an_out_of_range_offset() {
        let mut coordinator = Coordinator::new(extent(500.0));
        let _ = coordinator.dispatch(
            Delta {
                amount: Vector::new(0.0, 400.0),
            },
            false,
        );

        coordinator.set_extent(extent(100.0));

        assert_eq!(coordinator.offset(), Vector::new(0.0, 100.0));
    }
}
