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
    child.activate();
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

/// Axis-aligned platform placements with inherited clips and conservative semantic occlusion.
pub(crate) fn native_views(child: &mut Child) -> Vec<crate::NativeViewPlacement> {
    let mut slots = Vec::new();
    let mut painted = Vec::new();
    let mut modal = false;
    fn visit(
        child: &mut Child,
        origin: Point,
        clip: Option<Rectangle>,
        path: &mut Vec<u16>,
        slots: &mut Vec<(crate::NativeViewPlacement, Vec<u16>, usize)>,
        painted: &mut Vec<(Rectangle, Vec<u16>)>,
        modal: &mut bool,
    ) {
        let at = place(child, origin, clip);
        let view = child.view();
        *modal |= view.is_modal();
        if let Some(descriptor) = view.native_view() {
            let visible = at
                .visible
                .and_then(|clip| clip.intersection(&at.bounds))
                .filter(|r| r.width > 0.0 && r.height > 0.0);
            slots.push((
                crate::NativeViewPlacement {
                    view: descriptor,
                    bounds: at.bounds,
                    visible,
                },
                path.clone(),
                painted.len(),
            ));
        } else if view
            .describe(&DescribeCx {
                bounds: at.bounds,
                focused: false,
                _marker: std::marker::PhantomData,
            })
            .is_some()
        {
            painted.push((at.bounds, path.clone()));
        }
        let (origin, clip) = inner(child.view(), at);
        let mut index = 0_u16;
        child.view_mut().children(&mut |nested| {
            path.push(index);
            visit(nested, origin, clip, path, slots, painted, modal);
            let _ = path.pop();
            index += 1;
        });
    }
    visit(
        child,
        Point::ORIGIN,
        Some(Rectangle::INFINITE),
        &mut Vec::new(),
        &mut slots,
        &mut painted,
        &mut modal,
    );
    slots
        .into_iter()
        .map(|(mut slot, path, position)| {
            if modal
                || slot.visible.is_some_and(|visible| {
                    painted[position..].iter().any(|(bounds, other)| {
                        !path.starts_with(other)
                            && visible
                                .intersection(bounds)
                                .is_some_and(|r| r.width > 0.0 && r.height > 0.0)
                    })
                })
            {
                slot.visible = None;
            }
            slot
        })
        .collect()
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

/// How far outside its bounds a view may still draw: shadows, focus rings.
const BLEED: f32 = 32.0;

pub(crate) fn paint(
    child: &mut Child,
    origin: Point,
    visible: Option<Rectangle>,
    path: &mut Vec<u16>,
    painter: &mut Painter<'_>,
) {
    // A view entirely outside what its clipping ancestors show cannot be
    // seen: the rows of a long list scrolled out of view are not painted.
    // `visible` is only narrower than the window inside a clipping view, so
    // nothing outside a clip is ever skipped. Layers are drawn above their
    // ancestors' clips, so they are always painted.
    if let Some(visible) = visible
        && !child.view().is_layer()
        && !place(child, origin, None)
            .bounds
            .expand(BLEED)
            .intersects(&visible)
    {
        return;
    }

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
    let bounds = place(child, origin, visible).bounds;
    let visual = child.view().visual(&crate::VisualCx {
        now: painter.now,
        frame: child.frame(),
        bounds,
    });
    let resets = child.view().resets_opacity();
    if visual.is_identity() && !resets {
        paint_placed(child, origin, visible, path, painter);
        return;
    }

    if visual.opacity <= 0.0 {
        return;
    }
    let centre = bounds.center();
    let scale = visual.scale.max(0.0);
    let transformation = taim_core::Transformation::translate(
        centre.x + visual.offset.x,
        centre.y + visual.offset.y,
    ) * taim_core::Transformation::scale(scale)
        * taim_core::Transformation::translate(-centre.x, -centre.y);

    let Painter {
        renderer,
        theme,
        pointer,
        text_color,
        focus,
        now,
    } = painter;
    let opacity = visual.opacity.min(1.0);
    {
        use taim_core::Renderer as _;
        if resets {
            renderer.start_opacity_reset(opacity);
        } else {
            renderer.start_opacity(opacity);
        }
    }
    {
        renderer.with_transformation(transformation, |renderer| {
            paint_placed(
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
    taim_core::Renderer::end_opacity(*renderer);
}

fn paint_placed(
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

    // Children inherit the view's text color for the duration of their paint.
    let inherited = painter.text_color;
    if let Some(color) = child.view().foreground(painter.theme) {
        painter.text_color = color;
    }

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
    painter.text_color = inherited;

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
    pub renderer: Option<&'a Renderer>,
    pub now: crate::Instant,
    pub pointer: Option<Point>,
    /// The pointer is a finger.
    pub touch: bool,
    pub requests: &'a mut Requests,
    pub focus: &'a mut Focus,
}

/// Cancellation is a lifecycle notification, not an interceptable gesture.
pub(crate) fn is_cancellation(event: &Event) -> bool {
    matches!(
        event,
        Event::Window(taim_core::window::Event::Unfocused)
            | Event::Touch(taim_core::touch::Event::FingerLost { .. })
    )
}

/// Offers events deepest first until captured; cancellation always reaches all views.
pub(crate) fn broadcast(
    child: &mut Child,
    origin: Point,
    visible: Option<Rectangle>,
    path: &mut Vec<u16>,
    event: &Event,
    router: &mut Router<'_>,
) -> Handled {
    let at = place(child, origin, visible);
    if let Event::Touch(taim_core::touch::Event::FingerPressed { position, .. }) = event
        && !at.visible.is_some_and(|area| area.contains(*position))
    {
        return Handled::No;
    }
    // A release over clipped content must cancel its pending activation,
    // even if the point is still inside the child's unclipped bounds.
    let clipped_release = match event {
        Event::Touch(taim_core::touch::Event::FingerLifted {
            id,
            position,
            metadata,
        }) if !at.visible.is_some_and(|area| area.contains(*position)) => {
            Some(Event::Touch(taim_core::touch::Event::FingerLost {
                id: *id,
                position: *position,
                metadata: *metadata,
            }))
        }
        _ => None,
    };
    let event = clipped_release.as_ref().unwrap_or(event);
    let notify_all = is_cancellation(event);

    let intercepted = child.view_mut().intercept(
        &mut EventCx {
            renderer: router.renderer,
            now: router.now,
            bounds: at.bounds,
            pointer: pointer_in(router.pointer, at.visible),
            requests: router.requests,
            focus: router.focus,
            path,
            touch: router.touch,
        },
        event,
    );

    if intercepted == Handled::Yes {
        cancel_children(child, at, path, router);
        return Handled::Yes;
    }

    let (inner_origin, inner_visible) = inner(child.view(), at);

    // Topmost (last painted) children first, so overlays get input before
    // what they cover.
    let mut count = 0_u16;
    child.view_mut().children(&mut |_| count += 1);

    let mut handled = Handled::No;
    for target in (0..count).rev() {
        let mut index = 0_u16;
        child.view_mut().children(&mut |grandchild| {
            if index == target && (notify_all || handled == Handled::No) {
                path.push(index);
                if broadcast(grandchild, inner_origin, inner_visible, path, event, router)
                    == Handled::Yes
                {
                    handled = Handled::Yes;
                }
                let _ = path.pop();
            }
            index += 1;
        });

        if handled == Handled::Yes && !notify_all {
            break;
        }
    }

    if handled == Handled::Yes && !notify_all {
        return handled;
    }

    let own_handled = child.view_mut().event(
        &mut EventCx {
            renderer: router.renderer,
            now: router.now,
            bounds: at.bounds,
            pointer: pointer_in(router.pointer, at.visible),
            requests: router.requests,
            focus: router.focus,
            path,
            touch: router.touch,
        },
        event,
    );

    if own_handled == Handled::Yes && tracing_events() {
        eprintln!("{} handled {event:?}", child.view().debug_name());
    }

    Handled::from(handled == Handled::Yes || own_handled == Handled::Yes)
}

/// Tells every view under `child` that the pointer gesture they may have
/// started is over: the pointer leaves, then the button is released with
/// no pointer anywhere, which no view treats as a click.
fn cancel_children(child: &mut Child, at: Placement, path: &mut Vec<u16>, router: &mut Router<'_>) {
    let (inner_origin, inner_visible) = inner(child.view(), at);
    let pointer = router.pointer.take();

    for event in [
        Event::Mouse(mouse::Event::CursorLeft),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    ] {
        let mut index = 0_u16;
        child.view_mut().children(&mut |grandchild| {
            path.push(index);
            let _ = broadcast_all(
                grandchild,
                inner_origin,
                inner_visible,
                path,
                &event,
                router,
            );
            let _ = path.pop();
            index += 1;
        });
    }

    router.pointer = pointer;
}

/// Offers `event` to every view in the subtree, whether or not one handles
/// it: a cancellation must reach every view that might be holding a press.
fn broadcast_all(
    child: &mut Child,
    origin: Point,
    visible: Option<Rectangle>,
    path: &mut Vec<u16>,
    event: &Event,
    router: &mut Router<'_>,
) -> Handled {
    let at = place(child, origin, visible);
    let (inner_origin, inner_visible) = inner(child.view(), at);

    let mut index = 0_u16;
    child.view_mut().children(&mut |grandchild| {
        path.push(index);
        let _ = broadcast_all(grandchild, inner_origin, inner_visible, path, event, router);
        let _ = path.pop();
        index += 1;
    });

    child.view_mut().event(
        &mut EventCx {
            renderer: router.renderer,
            now: router.now,
            bounds: at.bounds,
            pointer: None,
            requests: router.requests,
            focus: router.focus,
            path,
            touch: router.touch,
        },
        event,
    )
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
            renderer: router.renderer,
            now: router.now,
            bounds: at.bounds,
            pointer: pointer_in(router.pointer, at.visible),
            requests: router.requests,
            focus: router.focus,
            path,
            touch: router.touch,
        },
        event,
    )
}

fn requested_action(request: &semantics::Request) -> semantics::Actions {
    use semantics::{Actions, Request};
    match request {
        Request::Activate => Actions::ACTIVATE,
        Request::Focus => Actions::FOCUS,
        Request::Increment => Actions::INCREMENT,
        Request::Decrement => Actions::DECREMENT,
        Request::SetValue(_) => Actions::SET_VALUE,
        Request::ToggleExpanded => Actions::TOGGLE_EXPANDED,
        Request::ScrollIntoView => Actions::SCROLL_INTO_VIEW,
        Request::Dismiss => Actions::DISMISS,
    }
}

pub(crate) fn accessibility_action(
    child: &mut Child,
    origin: Point,
    path: &mut Vec<u16>,
    action: &semantics::Action,
    scope: Option<&[u16]>,
    router: &mut Router<'_>,
) -> Option<Handled> {
    let at = place(child, origin, None);
    let descriptor = child.view().describe(&DescribeCx {
        bounds: at.bounds,
        focused: router.focus.current.as_deref() == Some(path.as_slice()),
        _marker: std::marker::PhantomData,
    });
    if let Some(node) = descriptor
        && node.id == action.target
    {
        if scope.is_some_and(|scope| !path.starts_with(scope))
            || node.state.contains(semantics::State::DISABLED)
            || !node.actions.contains(requested_action(&action.request))
        {
            return Some(Handled::No);
        }
        return Some(child.view_mut().accessibility_action(
            &mut EventCx {
                renderer: router.renderer,
                now: router.now,
                bounds: at.bounds,
                pointer: None,
                requests: router.requests,
                focus: router.focus,
                path,
                touch: router.touch,
            },
            &action.request,
        ));
    }
    let (origin, _) = inner(child.view(), at);
    let mut found = None;
    let mut index = 0_u16;
    child.view_mut().children(&mut |nested| {
        if found.is_none() {
            path.push(index);
            found = accessibility_action(nested, origin, path, action, scope, router);
            let _ = path.pop();
        }
        index += 1;
    });
    found
}

/// Whether any view is mid-animation at `now`.
pub(crate) fn any_animating(child: &mut Child, now: crate::Instant) -> bool {
    let mut animating = false;
    walk(child, Point::ORIGIN, &mut Vec::new(), &mut |view, _, _| {
        animating |= view.is_animating(now);
    });
    animating
}

/// The active modal ancestry of the last-painted modal subtree.
pub(crate) fn modal_scopes(
    child: &mut Child,
    path: &mut Vec<u16>,
    ancestors: &mut Vec<(taim_core::widget::Id, Vec<u16>)>,
) -> Vec<(taim_core::widget::Id, Vec<u16>)> {
    let modal = child.view().is_modal();
    if modal {
        ancestors.push((child.identity.clone(), path.clone()));
    }
    let mut selected = if modal { ancestors.clone() } else { Vec::new() };
    let mut index = 0_u16;
    child.view_mut().children(&mut |nested| {
        path.push(index);
        let candidate = modal_scopes(nested, path, ancestors);
        if !candidate.is_empty() {
            selected = candidate;
        }
        let _ = path.pop();
        index += 1;
    });
    if modal {
        let _ = ancestors.pop();
    }
    selected
}

pub(crate) fn cancel_outside_scope(child: &mut Child, scope: &[u16]) {
    walk(
        child,
        Point::ORIGIN,
        &mut Vec::new(),
        &mut |view, _, path| {
            if !path.starts_with(scope) {
                view.cancel_interaction();
            }
        },
    );
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

/// Identity of the actual control at a focus path, never its positional replacement.
pub(crate) fn focus_identity(child: &mut Child, path: &[u16]) -> Option<taim_core::widget::Id> {
    let Some((&next, rest)) = path.split_first() else {
        return child.view().focusable().then(|| child.identity.clone());
    };
    let mut found = None;
    let mut index = 0_u16;
    child.view_mut().children(&mut |nested| {
        if index == next {
            found = focus_identity(nested, rest);
        }
        index += 1;
    });
    found
}

/// Finds a retained control after its parent has laid out or rebuilt children.
pub(crate) fn focus_path(
    child: &mut Child,
    identity: &taim_core::widget::Id,
    path: &mut Vec<u16>,
) -> Option<Vec<u16>> {
    if &child.identity == identity && child.view().focusable() {
        return Some(path.clone());
    }
    let mut found = None;
    let mut index = 0_u16;
    child.view_mut().children(&mut |nested| {
        if found.is_none() {
            path.push(index);
            found = focus_path(nested, identity, path);
            let _ = path.pop();
        }
        index += 1;
    });
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
        }) if !modifiers.intersects(
            keyboard::Modifiers::CTRL | keyboard::Modifiers::ALT | keyboard::Modifiers::LOGO,
        ) =>
        {
            Some(modifiers.shift())
        }
        _ => None,
    }
}

/// Events that go to the focused view rather than to every view.
pub(crate) fn is_focused_input(event: &Event) -> bool {
    matches!(event, Event::Keyboard(_) | Event::InputMethod(_))
}

/// Whether `event` is a mouse or touch press that may move focus.
pub(crate) fn is_press(event: &Event) -> bool {
    matches!(
        event,
        Event::Mouse(mouse::Event::ButtonPressed(_))
            | Event::Touch(taim_core::touch::Event::FingerPressed { .. })
    )
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

/// Describes only an active modal subtree, preserving window-space bounds.
pub(crate) fn describe_scope(
    child: &mut Child,
    origin: Point,
    path: &mut Vec<u16>,
    scope: &[u16],
    focus: Option<&[u16]>,
    out: &mut Vec<semantics::Node>,
) {
    let Some((&next, rest)) = scope.split_first() else {
        describe(child, origin, path, focus, out);
        return;
    };
    let at = place(child, origin, None);
    let (origin, _) = inner(child.view(), at);
    let mut index = 0_u16;
    child.view_mut().children(&mut |nested| {
        if index == next {
            path.push(index);
            describe_scope(nested, origin, path, rest, focus, out);
            let _ = path.pop();
        }
        index += 1;
    });
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
    describe_clipped(child, origin, Some(Rectangle::INFINITE), path, focus, out);
}
fn describe_clipped(
    child: &mut Child,
    origin: Point,
    clip: Option<Rectangle>,
    path: &mut Vec<u16>,
    focus: Option<&[u16]>,
    out: &mut Vec<semantics::Node>,
) {
    let at = place(child, origin, clip);
    let mut own = child.view().describe(&DescribeCx {
        bounds: at.bounds,
        focused: focus == Some(path.as_slice()),
        _marker: std::marker::PhantomData,
    });
    if let Some(node) = own.as_mut() {
        if let Some(visible) = at
            .visible
            .and_then(|clip| clip.intersection(&at.bounds))
            .filter(|r| r.width > 0.0 && r.height > 0.0)
        {
            node.bounds = visible;
        } else {
            node.state.insert(semantics::State::HIDDEN);
            node.state.remove(semantics::State::FOCUSED);
        }
    }
    let (origin, clip) = inner(child.view(), at);
    let mut nested = Vec::new();
    let mut index = 0_u16;
    child.view_mut().children(&mut |grandchild| {
        path.push(index);
        describe_clipped(grandchild, origin, clip, path, focus, &mut nested);
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
