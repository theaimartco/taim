//! Renderer-neutral damage request types.
//!
//! A [`Request`] describes _what_ became invalid and _why_, without
//! committing to any particular tracking or merging strategy. Phase 4
//! (`graphics/src/damage.rs`) builds the actual region-tracking,
//! clipping, and merge-heuristic system on top of these types; this
//! module only defines the vocabulary so widgets, [`Shell`], and the
//! runtime can start expressing damage precisely ahead of that.
//!
//! [`Shell`]: crate::Shell

use crate::Rectangle;

/// A request to redraw some region of the surface, or the whole of it.
///
/// [`Region`] intentionally has no merging, clipping, or budget logic;
/// see the invariants below and `graphics::damage` (Phase 4) for the
/// system built on top of this vocabulary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Region {
    /// A single rectangular area, in logical coordinates relative to the
    /// surface's origin.
    ///
    /// # Invariants
    /// A well-formed [`Region::Area`] is finite (no `NaN`/`±∞` components)
    /// and has non-negative width/height. Producers should prefer
    /// [`Region::area`] to construct one, which enforces this.
    Area(Rectangle),

    /// The entire surface is invalid and must be redrawn.
    ///
    /// A full-surface request is the safe fallback whenever a caller
    /// cannot (or should not, e.g. because there are too many scattered
    /// areas) express damage more precisely.
    Full,
}

impl Region {
    /// Creates a [`Region::Area`], normalizing away non-finite input by
    /// falling back to [`Region::Full`].
    ///
    /// A negative width/height (e.g. from an unintersected rectangle) is
    /// treated as empty and reported as [`None`], since there is nothing
    /// to redraw in that case, not a full-surface fallback.
    pub fn area(bounds: Rectangle) -> Option<Self> {
        if !bounds.width.is_finite() || !bounds.height.is_finite() {
            return Some(Self::Full);
        }

        if !bounds.x.is_finite() || !bounds.y.is_finite() {
            return Some(Self::Full);
        }

        if bounds.width < 0.0 || bounds.height < 0.0 {
            return None;
        }

        Some(Self::Area(bounds))
    }

    /// Returns the bounds of this [`Region`], if it is not [`Region::Full`].
    pub fn bounds(&self) -> Option<Rectangle> {
        match self {
            Region::Area(bounds) => Some(*bounds),
            Region::Full => None,
        }
    }

    /// Combines this [`Region`] with another, producing the smallest
    /// [`Region`] that covers both.
    ///
    /// If either side is [`Region::Full`], the result is
    /// [`Region::Full`].
    pub fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Region::Full, _) | (_, Region::Full) => Region::Full,
            (Region::Area(a), Region::Area(b)) => Region::Area(a.union(&b)),
        }
    }
}

/// Why some content is being reported as damaged.
///
/// [`Cause`] exists so diagnostics (`runtime::diagnostics`, Phase 2) and
/// later phases' full-screen-fallback heuristics (Phase 4) can explain
/// _why_ a frame redrew more than expected, instead of only reporting
/// _that_ it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// A widget's own appearance changed (e.g. a value it displays).
    Content,
    /// A widget moved, was resized, or was reparented.
    Layout,
    /// A widget gained, lost, or changed interaction state (hover, focus,
    /// press, etc.).
    Interaction,
    /// An in-progress animation produced this frame.
    Animation,
    /// The cause could not be attributed to a more specific category.
    Unknown,
}

bitflags::bitflags! {
    /// Every [`Cause`] that contributed to a frame's damage.
    ///
    /// A set rather than one [`Cause`], because a frame legitimately has
    /// several: a hover, a blinking cursor and an animation all in the
    /// same 16ms. Reporting only the first would attribute the whole
    /// frame's redraw to whichever widget happened to run first, which is
    /// worse than reporting nothing — a developer would go and look at it.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Causes: u8 {
        /// A widget's own appearance changed.
        const CONTENT = 1 << 0;
        /// A widget moved, was resized, or was reparented.
        const LAYOUT = 1 << 1;
        /// A widget's interaction state changed.
        const INTERACTION = 1 << 2;
        /// An animation produced this frame.
        const ANIMATION = 1 << 3;
        /// Something unattributed.
        const UNKNOWN = 1 << 4;
    }
}

impl Causes {
    /// The set containing just `cause`.
    pub fn of(cause: Cause) -> Self {
        match cause {
            Cause::Content => Self::CONTENT,
            Cause::Layout => Self::LAYOUT,
            Cause::Interaction => Self::INTERACTION,
            Cause::Animation => Self::ANIMATION,
            Cause::Unknown => Self::UNKNOWN,
        }
    }

    /// The causes in this set, in a stable order.
    ///
    /// Declaration order, not insertion order: a report that varied with
    /// the order widgets happened to run in could not be diffed between
    /// two frames, which is the main thing anyone does with one.
    pub fn iter_causes(self) -> impl Iterator<Item = Cause> {
        [
            (Self::CONTENT, Cause::Content),
            (Self::LAYOUT, Cause::Layout),
            (Self::INTERACTION, Cause::Interaction),
            (Self::ANIMATION, Cause::Animation),
            (Self::UNKNOWN, Cause::Unknown),
        ]
        .into_iter()
        .filter_map(move |(flag, cause)| self.contains(flag).then_some(cause))
    }
}

/// Why a frame fell back to redrawing the whole surface.
///
/// The two are not the same finding, which is why they are not the same
/// variant. A [`Fallback::Requested`] full redraw is a caller saying "I
/// cannot describe this precisely" — often correct, and nothing to fix.
/// A [`Fallback::Overflow`] is damage tracking *giving up*: the frame had
/// precise damage and the aggregator could not hold it, which is a
/// performance bug with a number attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fallback {
    /// A caller asked for a full redraw.
    Requested(Cause),

    /// Too many distinct areas to track, so precision was abandoned.
    Overflow {
        /// How many areas had accumulated when it gave up.
        areas: usize,
    },
}

/// A frame's damage, and why.
///
/// The thing [`Region`] alone could not say. A full redraw with no reason
/// attached is the single least actionable output a damage system can
/// produce: it is the case where something is definitely wrong and the
/// developer has nowhere to start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Attribution {
    /// What must be redrawn.
    pub region: Region,
    /// Every cause that contributed.
    pub causes: Causes,
    /// Why it became [`Region::Full`], if it did.
    ///
    /// `None` for a precise region — there was no fallback to explain.
    pub fallback: Option<Fallback>,
}

impl Attribution {
    /// Whether the whole surface is being redrawn.
    pub fn is_full(&self) -> bool {
        matches!(self.region, Region::Full)
    }
}

/// A single, attributed damage request.
///
/// This is the renderer-neutral unit later phases' retained-layer and
/// compositor code (Phase 4) consume; Phase 2 only defines its shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Request {
    /// The [`Region`] being damaged.
    pub region: Region,
    /// Why the region is being damaged.
    pub cause: Cause,
}

impl Request {
    /// Creates a new [`Request`] for the given [`Region`] and [`Cause`].
    pub const fn new(region: Region, cause: Cause) -> Self {
        Self { region, cause }
    }

    /// Creates a [`Request`] that damages the entire surface.
    pub const fn full(cause: Cause) -> Self {
        Self::new(Region::Full, cause)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point, Size};

    #[test]
    fn area_rejects_non_finite_bounds_as_full() {
        let bounds = Rectangle::new(Point::new(f32::NAN, 0.0), Size::new(10.0, 10.0));

        assert_eq!(Region::area(bounds), Some(Region::Full));
    }

    #[test]
    fn area_rejects_negative_size_as_none() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(-1.0, 10.0));

        assert_eq!(Region::area(bounds), None);
    }

    #[test]
    fn area_accepts_well_formed_bounds() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(10.0, 10.0));

        assert_eq!(Region::area(bounds), Some(Region::Area(bounds)));
    }

    #[test]
    fn merge_of_two_areas_is_their_union() {
        let a = Region::Area(Rectangle::new(Point::ORIGIN, Size::new(10.0, 10.0)));
        let b = Region::Area(Rectangle::new(Point::new(5.0, 5.0), Size::new(10.0, 10.0)));

        let merged = a.merge(b);

        assert_eq!(
            merged,
            Region::Area(Rectangle::new(Point::ORIGIN, Size::new(15.0, 15.0)))
        );
    }

    #[test]
    fn merge_with_full_is_always_full() {
        let area = Region::Area(Rectangle::new(Point::ORIGIN, Size::new(10.0, 10.0)));

        assert_eq!(area.merge(Region::Full), Region::Full);
        assert_eq!(Region::Full.merge(area), Region::Full);
    }

    #[test]
    fn bounds_of_full_is_none() {
        assert_eq!(Region::Full.bounds(), None);
    }
}
