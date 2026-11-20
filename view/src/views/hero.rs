//! Shared elements: a [`hero`] on one page flies to the hero with the same
//! tag on the next page while the [`navigator`](super::navigator) moves
//! between them.
//!
//! A navigator puts each page it builds in a [`Stage`]: a board shared by
//! all of its pages that records where each hero was laid out and, during
//! a transition, which pages are moving and how far. A hero reads the
//! board when it is drawn. The hero on the page that is on top flies from
//! its own place to its partner's, as a scaled copy of itself, undoing its
//! page's movement and fading so it travels in window space; its partner
//! hides under it until the transition ends.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::cx::{DescribeCx, MeasureCx, PaintCx};
use crate::layout::{Constraints, Sizing};
use crate::{AnyView, Child, Point, Rectangle, Size, Vector, View, Visual, VisualCx};
use taim_core::semantics;

/// At most this many heroes are remembered per navigator; beyond it the
/// board is cleared, so a page with unbounded tags cannot grow it forever.
const SEEN_LIMIT: usize = 512;

/// One page of a navigator, as heroes on it see it.
#[derive(Debug)]
pub(crate) struct PageLook {
    pub(crate) id: u64,
    /// How the page is drawn this frame.
    pub(crate) visual: Cell<Visual>,
    /// The page's centre, which its scale is about.
    pub(crate) centre: Cell<Point>,
    /// How dark the page is shaded, under a page sliding over it.
    pub(crate) dim: Cell<f32>,
    /// How strongly the page casts a shadow, sliding over another.
    pub(crate) lift: Cell<f32>,
}

impl PageLook {
    pub(crate) fn new(id: u64) -> Rc<Self> {
        Rc::new(Self {
            id,
            visual: Cell::new(Visual::IDENTITY),
            centre: Cell::new(Point::ORIGIN),
            dim: Cell::new(0.0),
            lift: Cell::new(0.0),
        })
    }
}

/// Two pages moving against each other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Flight {
    /// The page drawn on top, whose heroes fly.
    pub(crate) upper: u64,
    /// The page underneath, whose heroes hide under the flying ones.
    pub(crate) lower: u64,
    /// 0 with heroes at their place on the upper page, 1 at their place on
    /// the lower page.
    pub(crate) travel: f32,
}

/// What a navigator's heroes share.
#[derive(Debug, Default)]
pub(crate) struct Board {
    pub(crate) flight: Cell<Option<Flight>>,
    /// Where each `(page, tag)` hero was last laid out, in window space.
    seen: RefCell<HashMap<(u64, u64), Rectangle>>,
}

impl Board {
    fn note(&self, page: u64, tag: u64, bounds: Rectangle) {
        let mut seen = self.seen.borrow_mut();
        if seen.len() >= SEEN_LIMIT && !seen.contains_key(&(page, tag)) {
            seen.clear();
        }
        let _ = seen.insert((page, tag), bounds);
    }

    fn find(&self, page: u64, tag: u64) -> Option<Rectangle> {
        self.seen.borrow().get(&(page, tag)).copied()
    }

    /// Forgets heroes on pages other than `pages`.
    pub(crate) fn keep_only(&self, pages: &[u64]) {
        self.seen
            .borrow_mut()
            .retain(|(page, _), _| pages.contains(page));
    }
}

/// The page a view is being built on, and its navigator's board.
#[derive(Clone)]
pub(crate) struct Stage {
    pub(crate) board: Rc<Board>,
    pub(crate) page: Rc<PageLook>,
}

thread_local! {
    static BUILDING: RefCell<Option<Stage>> = const { RefCell::new(None) };
}

/// Runs `build` with heroes made inside it belonging to `stage`.
pub(crate) fn on_stage<T>(stage: Stage, build: impl FnOnce() -> T) -> T {
    struct Restore(Option<Stage>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            BUILDING.with(|building| *building.borrow_mut() = previous);
        }
    }
    let _restore = Restore(BUILDING.with(|building| building.borrow_mut().replace(stage)));
    build()
}

/// Marks `content` as the same thing across pages: when a
/// [`navigator`](super::navigator) moves to a page with a hero of the same
/// `tag`, the content flies from where it was to where it is on the new
/// page, growing or shrinking on the way. A thumbnail opening into a
/// detail page, an avatar becoming a profile header.
///
/// Tags pair heroes one to one between the two pages, so give each hero on
/// a page its own (an item's id, say). Outside a navigator, or with no
/// partner, a hero is just its content. The flight scales the content
/// uniformly rather than laying it out again at each size, and it stays
/// inside the clips of scroll views around it.
///
/// ```ignore
/// // On the list page:
/// button(hero(("photo", id), image(thumb))).on_press(move || nav.push(Route::Photo(id)))
/// // On the photo page:
/// hero(("photo", id), image(full))
/// ```
pub fn hero(tag: impl Hash, content: impl Into<AnyView>) -> Hero {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    tag.hash(&mut hasher);
    Hero {
        content: Child::new(content),
        tag: hasher.finish(),
        stage: BUILDING.with(|building| building.borrow().clone()),
        flying: Cell::new(false),
    }
}

/// See [`hero`].
pub struct Hero {
    content: Child,
    tag: u64,
    stage: Option<Stage>,
    flying: Cell<bool>,
}

/// `a` towards `b` by `t`.
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

impl Hero {
    /// How to draw a hero at `own` on the upper page, travelling towards
    /// `partner` on the lower page, given how the page itself is drawn.
    fn flight(own: Rectangle, partner: Rectangle, travel: f32, page: &PageLook) -> Visual {
        let own_centre = own.center();
        let partner_centre = partner.center();
        let centre = Point::new(
            mix(own_centre.x, partner_centre.x, travel),
            mix(own_centre.y, partner_centre.y, travel),
        );
        // A uniform scale between the two sizes: the geometric mean of the
        // width and height ratios, so neither axis is far off.
        let ratio = |partner: f32, own: f32| (partner / own.max(1.0)).max(1.0e-3);
        let target = (ratio(partner.width, own.width) * ratio(partner.height, own.height)).sqrt();
        let scale = mix(1.0, target, travel);

        // The page draws a point `y` at `c + o + s (y - c)`. Undo that, so
        // the hero is where the flight wants it on screen.
        let shown = page.visual.get();
        let page_scale = shown.scale.max(1.0e-3);
        let page_centre = page.centre.get();
        let at = Point::new(
            page_centre.x + (centre.x - shown.offset.x - page_centre.x) / page_scale,
            page_centre.y + (centre.y - shown.offset.y - page_centre.y) / page_scale,
        );

        Visual {
            offset: Vector::new(at.x - own_centre.x, at.y - own_centre.y),
            scale: scale / page_scale,
            opacity: 1.0,
        }
    }
}

impl View for Hero {
    fn sizing(&self) -> Sizing {
        self.content.sizing()
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let size = self.content.measure(cx, constraints);
        self.content.place(Point::ORIGIN);
        size
    }

    fn paint(&self, _cx: &mut PaintCx<'_>) {}

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    fn visual(&self, cx: &VisualCx) -> Visual {
        self.flying.set(false);
        let Some(Stage { board, page }) = &self.stage else {
            return Visual::IDENTITY;
        };
        let own = cx.bounds();
        board.note(page.id, self.tag, own);

        let Some(flight) = board.flight.get() else {
            return Visual::IDENTITY;
        };
        if page.id == flight.upper {
            let Some(partner) = board.find(flight.lower, self.tag) else {
                return Visual::IDENTITY;
            };
            self.flying.set(true);
            Self::flight(own, partner, flight.travel.clamp(0.0, 1.0), page)
        } else if page.id == flight.lower && board.find(flight.upper, self.tag).is_some() {
            // The upper page's hero is flying over this one.
            Visual {
                opacity: 0.0,
                ..Visual::IDENTITY
            }
        } else {
            Visual::IDENTITY
        }
    }

    fn resets_opacity(&self) -> bool {
        self.flying.get()
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::space;

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
        Rectangle::new(Point::new(x, y), Size::new(width, height))
    }

    fn stage(board: &Rc<Board>, id: u64) -> Stage {
        Stage {
            board: board.clone(),
            page: PageLook::new(id),
        }
    }

    fn drawn(hero: &Hero, bounds: Rectangle) -> Visual {
        hero.visual(&VisualCx::new(crate::Instant::now(), bounds, bounds))
    }

    #[test]
    fn a_flying_hero_starts_over_its_partner_and_lands_at_its_own_place() {
        let board = Rc::new(Board::default());
        let list = on_stage(stage(&board, 1), || hero(7, space()));
        let detail = on_stage(stage(&board, 2), || hero(7, space()));
        let thumb = rect(20.0, 300.0, 50.0, 50.0);
        let banner = rect(0.0, 0.0, 200.0, 200.0);
        let _ = drawn(&list, thumb);

        // Pushed: the detail page is on top, and its hero starts over the
        // thumbnail.
        board.flight.set(Some(Flight {
            upper: 2,
            lower: 1,
            travel: 1.0,
        }));
        let start = drawn(&detail, banner);
        assert!(detail.resets_opacity());
        let centre = banner.center();
        let shown = Point::new(centre.x + start.offset.x, centre.y + start.offset.y);
        assert!((shown.x - thumb.center().x).abs() < 1.0e-3);
        assert!((shown.y - thumb.center().y).abs() < 1.0e-3);
        assert!((start.scale - 0.25).abs() < 1.0e-3);

        // The thumbnail hides under it.
        assert_eq!(drawn(&list, thumb).opacity, 0.0);

        // Landed.
        board.flight.set(Some(Flight {
            upper: 2,
            lower: 1,
            travel: 0.0,
        }));
        let end = drawn(&detail, banner);
        assert!(end.offset.x.abs() < 1.0e-3 && (end.scale - 1.0).abs() < 1.0e-3);

        board.flight.set(None);
        assert!(drawn(&detail, banner).is_identity() && drawn(&list, thumb).is_identity());
        assert!(!detail.resets_opacity());
    }

    #[test]
    fn a_hero_undoes_its_pages_movement_while_flying() {
        let board = Rc::new(Board::default());
        let lower = on_stage(stage(&board, 1), || hero("a", space()));
        let upper_stage = stage(&board, 2);
        let upper = on_stage(upper_stage.clone(), || hero("a", space()));
        let place = rect(100.0, 100.0, 40.0, 40.0);
        let _ = drawn(&lower, place);

        // The upper page slides 300 pixels right and shrinks a little.
        upper_stage.page.visual.set(Visual {
            offset: Vector::new(300.0, 0.0),
            scale: 0.9,
            opacity: 0.5,
        });
        upper_stage.page.centre.set(Point::new(200.0, 400.0));
        board.flight.set(Some(Flight {
            upper: 2,
            lower: 1,
            travel: 1.0,
        }));
        let hero = drawn(&upper, place);

        // Composing the hero's and the page's transforms puts the hero's
        // centre back on the partner's.
        let own = place.center();
        let inner = Point::new(own.x + hero.offset.x, own.y + hero.offset.y);
        let on_screen = Point::new(
            200.0 + 300.0 + 0.9 * (inner.x - 200.0),
            400.0 + 0.9 * (inner.y - 400.0),
        );
        assert!((on_screen.x - own.x).abs() < 1.0e-2 && (on_screen.y - own.y).abs() < 1.0e-2);
        assert!((hero.scale * 0.9 - 1.0).abs() < 1.0e-3);
    }

    #[test]
    fn a_hero_without_a_partner_or_a_navigator_is_just_its_content() {
        let board = Rc::new(Board::default());
        let alone = on_stage(stage(&board, 2), || hero("only", space()));
        board.flight.set(Some(Flight {
            upper: 2,
            lower: 1,
            travel: 0.5,
        }));
        assert!(drawn(&alone, rect(0.0, 0.0, 10.0, 10.0)).is_identity());
        assert!(drawn(&hero("free", space()), rect(0.0, 0.0, 10.0, 10.0)).is_identity());
    }
}
