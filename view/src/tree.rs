//! Walking a view tree: painting, routing events, focus and semantics.
//!
//! Every walk computes each view's window-space bounds from the frames
//! parents assigned during measuring, shifted by any scroll offset, and
//! tracks the path of child indices that identifies a view for focus.
use crate::cx::{DescribeCx, EventCx, Focus, PaintCx, Requests};
use crate::{Child, Color, Event, Handled, Point, Rectangle, Renderer, Theme, View};
use crate::{keyboard, mouse, semantics};

use taim_core::Renderer as _;

/// A view's place in the walk.
#[derive(Debug, Clone, Copy)]
struct Placement {
    bounds: Rectangle,
    /// What may receive the pointer: the intersection of every clipping
    /// ancestor, or `None` when the pointer is clipped away entirely.
    visible: Option<Rectangle>,
}

fn place(child: &Child, origin: Point, visible: Option<Rectangle>) -> Placement {
    let frame = child.frame();

    Placement {
        bounds: Rectangle::new(
            Point::new(origin.x + frame.x, origin.y + frame.y),
            frame.size(),
        ),
        visible,
    }
}

/// Where `view`'s children start and what of them is visible.
fn inner(view: &dyn View, at: Placement) -> (Point, Option<Rectangle>) {
    let offset = view.scroll_offset();
    let origin = Point::new(at.bounds.x - offset.x, at.bounds.y - offset.y);
    let visible = if view.clips() {
        at.visible
            .and_then(|visible| visible.intersection(&at.bounds))
    } else {
        at.visible
    };

    (origin, visible)
}

fn pointer_in(pointer: Option<Point>, visible: Option<Rectangle>) -> Option<Point> {
    pointer.filter(|at| visible.is_some_and(|visible| visible.contains(*at)))
}

/// Calls `visit` for every view, parents before children, with its bounds
/// and path.
pub(crate) fn walk(
    child: &mut Child,
    origin: Point,
    path: &mut Vec<u16>,
    visit: &mut dyn FnMut(&mut dyn View, Rectangle, &[u16]),
) {
    let at = place(child, origin, None);
    visit(child.view_mut(), at.bounds, path);

    let (origin, _) = inner(child.view(), at);
    let mut index = 0_u16;
    child.view_mut().children(&mut |grandchild| {
        path.push(index);
        walk(grandchild, origin, path, visit);
        let _ = path.pop();
        index += 1;
    });
}

/// Everything painting needs besides the tree.
pub(crate) struct Painter<'a> {
    pub renderer: &'a mut Renderer,
    pub theme: &'a Theme,
    pub pointer: Option<Point>,
    pub text_color: Color,
    pub focus: Option<&'a [u16]>,
    pub now: crate::Instant,
}

pub(crate) fn paint(
    child: &mut Child,
    origin: Point,
    visible: Option<Rectangle>,
    path: &mut Vec<u16>,
    painter: &mut Painter<'_>,
) {
    if !child.view().is_layer() {
        paint_view(child, origin, visible, path, painter);
        return;
    }

    let Painter {
        renderer,
        theme,
        pointer,
        text_color,
        focus,
        now,
    } = painter;
    let area = Rectangle::INFINITE;
    renderer.with_layer(area, |renderer| {
        paint_view(
            child,
            origin,
            visible,
            path,
            &mut Painter {
                renderer,
                theme,
                pointer: *pointer,
                text_color: *text_color,
                focus: *focus,
                now: *now,
            },
        );
    });
}

fn paint_view(
    child: &mut Child,
    origin: Point,
    visible: Option<Rectangle>,
    path: &mut Vec<u16>,
    painter: &mut Painter<'_>,
) {
    let at = place(child, origin, visible);

    child.view().paint(&mut PaintCx {
        renderer: &mut *painter.renderer,
        theme: painter.theme,
        bounds: at.bounds,
        pointer: pointer_in(painter.pointer, at.visible),
        text_color: painter.text_color,
        focused: painter.focus == Some(path.as_slice()),
        now: painter.now,
    });

    let (origin, visible) = inner(child.view(), at);
    let clip = child.view().clips().then_some(at.bounds);
    let focused = painter.focus == Some(path.as_slice());

    let mut paint_children = |painter: &mut Painter<'_>| {
        let mut index = 0_u16;
        child.view_mut().children(&mut |grandchild| {
            path.push(index);
            paint(grandchild, origin, visible, path, painter);
            let _ = path.pop();
            index += 1;
        });
    };

    match clip {
        Some(bounds) => {
            // `with_layer` hands the renderer back through a closure.
            let Painter {
                renderer,
                theme,
                pointer,
                text_color,
                focus,
                now,
            } = painter;
            renderer.with_layer(bounds, |renderer| {
                paint_children(&mut Painter {
                    renderer,
                    theme,
                    pointer: *pointer,
                    text_color: *text_color,
                    focus: *focus,
                    now: *now,
                });
            });
        }
        None => paint_children(painter),
    }

    child.view().paint_over(&mut PaintCx {
        renderer: &mut *painter.renderer,
        theme: painter.theme,
        bounds: at.bounds,
        pointer: pointer_in(painter.pointer, at.visible),
        text_color: painter.text_color,
        focused,
        now: painter.now,
    });
}

/// Everything routing needs besides the tree.
pub(crate) struct Router<'a> {
    pub now: crate::Instant,
    pub pointer: Option<Point>,
    pub requests: &'a mut Requests,
    pub focus: &'a mut Focus,
}

/// Offers `event` to every view, deepest first, until one handles it.
pub(crate) fn broadcast(
    child: &mut Child,
    origin: Point,
    visible: Option<Rectangle>,
    path: &mut Vec<u16>,
    event: &Event,
    router: &mut Router<'_>,
) -> Handled {
    let at = place(child, origin, visible);
    let (inner_origin, inner_visible) = inner(child.view(), at);

    // Topmost (last painted) children first, so overlays get input before
    // what they cover.
    let mut count = 0_u16;
    child.view_mut().children(&mut |_| count += 1);

    let mut handled = Handled::No;
    for target in (0..count).rev() {
        let mut index = 0_u16;
        child.view_mut().children(&mut |grandchild| {
            if index == target && handled == Handled::No {
                path.push(index);
                handled = broadcast(grandchild, inner_origin, inner_visible, path, event, router);
                let _ = path.pop();
            }
            index += 1;
        });

        if handled == Handled::Yes {
            break;
        }
    }

    if handled == Handled::Yes {
        return handled;
    }

    let handled = child.view_mut().event(
        &mut EventCx {
            now: router.now,
            bounds: at.bounds,
            pointer: pointer_in(router.pointer, at.visible),
            requests: router.requests,
            focus: router.focus,
            path,
        },
        event,
    );

    if handled == Handled::Yes && tracing_events() {
        eprintln!("{} handled {event:?}", child.view().debug_name());
    }

    handled
}

/// Whether `TAIM_VIEW_TRACE` asks for every handled event to be logged.
fn tracing_events() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("TAIM_VIEW_TRACE").is_some())
}

/// Delivers `event` to the view at `target`, bubbling up through its
/// ancestors until one handles it.
pub(crate) fn deliver(
    child: &mut Child,
    origin: Point,
    visible: Option<Rectangle>,
    path: &mut Vec<u16>,
    target: &[u16],
    event: &Event,
    router: &mut Router<'_>,
) -> Handled {
    let at = place(child, origin, visible);

    if let Some(&next) = target.get(path.len()) {
        let (inner_origin, inner_visible) = inner(child.view(), at);
        let mut handled = Handled::No;
        let mut index = 0_u16;

        child.view_mut().children(&mut |grandchild| {
            if index == next {
                path.push(index);
                handled = deliver(
                    grandchild,
                    inner_origin,
                    inner_visible,
                    path,
                    target,
                    event,
                    router,
                );
                let _ = path.pop();
            }
            index += 1;
        });

        if handled == Handled::Yes {
            return handled;
        }
    }

    child.view_mut().event(
        &mut EventCx {
            now: router.now,
            bounds: at.bounds,
            pointer: pointer_in(router.pointer, at.visible),
            requests: router.requests,
            focus: router.focus,
            path,
        },
        event,
    )
}

/// Whether any view is mid-animation at `now`.
pub(crate) fn any_animating(child: &mut Child, now: crate::Instant) -> bool {
    let mut animating = false;
    walk(child, Point::ORIGIN, &mut Vec::new(), &mut |view, _, _| {
        animating |= view.is_animating(now);
    });
    animating
}

/// Paths of every focusable view, in tree order.
pub(crate) fn focusables(child: &mut Child) -> Vec<Vec<u16>> {
    let mut found = Vec::new();
    walk(
        child,
        Point::ORIGIN,
        &mut Vec::new(),
        &mut |view, _, path| {
            if view.focusable() {
                found.push(path.to_vec());
            }
        },
    );
    found
}

/// The focus after a Tab (or Shift+Tab) press, given the current focus.
pub(crate) fn tab(
    order: &[Vec<u16>],
    current: Option<&[u16]>,
    backwards: bool,
) -> Option<Vec<u16>> {
    if order.is_empty() {
        return None;
    }

    let position = current.and_then(|current| order.iter().position(|path| path == current));
    let next = match (position, backwards) {
        (None, false) => 0,
        (None, true) => order.len() - 1,
        (Some(at), false) => (at + 1) % order.len(),
        (Some(at), true) => (at + order.len() - 1) % order.len(),
    };

    Some(order[next].clone())
}

/// Whether `event` is a Tab press, and if so whether Shift is held.
pub(crate) fn tab_press(event: &Event) -> Option<bool> {
    match event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Tab),
            modifiers,
            ..
        }) => Some(modifiers.shift()),
        _ => None,
    }
}

/// Events that go to the focused view rather than to every view.
pub(crate) fn is_focused_input(event: &Event) -> bool {
    matches!(event, Event::Keyboard(_) | Event::InputMethod(_))
}

/// Whether `event` is a primary-button press (which moves focus).
pub(crate) fn is_press(event: &Event) -> bool {
    matches!(event, Event::Mouse(mouse::Event::ButtonPressed(_)))
}

/// The innermost cursor preference under `pointer`.
pub(crate) fn cursor(
    child: &mut Child,
    origin: Point,
    pointer: Point,
) -> Option<mouse::Interaction> {
    let mut shown = None;
    let mut hidden_below: Vec<Rectangle> = Vec::new();

    walk(child, origin, &mut Vec::new(), &mut |view, bounds, path| {
        hidden_below.truncate(path.len());
        let clipped = hidden_below.iter().any(|clip| !clip.contains(pointer));

        if !clipped
            && bounds.contains(pointer)
            && let Some(preference) = view.cursor()
        {
            shown = Some(preference);
        }

        // Children are clipped to this view if it clips; record its bounds
        // at this depth so descendants can check them.
        hidden_below.push(if view.clips() {
            bounds
        } else {
            Rectangle::INFINITE
        });
    });

    shown
}

/// The semantic nodes of `child`'s subtree; views that do not describe
/// themselves are transparent.
pub(crate) fn describe(
    child: &mut Child,
    origin: Point,
    path: &mut Vec<u16>,
    focus: Option<&[u16]>,
    out: &mut Vec<semantics::Node>,
) {
    let at = place(child, origin, None);
    let own = child.view().describe(&DescribeCx {
        bounds: at.bounds,
        focused: focus == Some(path.as_slice()),
        _marker: std::marker::PhantomData,
    });

    let (origin, _) = inner(child.view(), at);
    let mut nested = Vec::new();
    let mut index = 0_u16;
    child.view_mut().children(&mut |grandchild| {
        path.push(index);
        describe(grandchild, origin, path, focus, &mut nested);
        let _ = path.pop();
        index += 1;
    });

    match own {
        Some(node) => out.push(nested.into_iter().fold(node, semantics::Node::with_child)),
        None => out.extend(nested),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_cycles_in_both_directions() {
        let order = vec![vec![0], vec![1, 0], vec![2]];

        assert_eq!(tab(&order, None, false), Some(vec![0]));
        assert_eq!(tab(&order, Some(&[0]), false), Some(vec![1, 0]));
        assert_eq!(tab(&order, Some(&[2]), false), Some(vec![0]));
        assert_eq!(tab(&order, Some(&[0]), true), Some(vec![2]));
        assert_eq!(tab(&[], Some(&[0]), true), None);
    }
}
