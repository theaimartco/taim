//! Build and reuse custom widgets using The Elm Architecture.
#![allow(deprecated)]
use crate::core::layout::Layout;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget;
use crate::core::widget::tree::Tree;
use crate::core::{self, Element, Length, Rectangle, Size};

use ouroboros::self_referencing;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

#[path = "component/implementation.rs"]
mod implementation;
#[path = "component/overlay.rs"]
mod overlay_runtime;

const RETAINED_EVENT_CAPACITY: usize = 16;
const MAX_RETAINED_EVENT_CAPACITY: usize = 64;

fn trim_event_buffer<T>(events: &mut Vec<T>) {
    debug_assert!(events.is_empty());

    if events.capacity() > MAX_RETAINED_EVENT_CAPACITY {
        events.shrink_to(RETAINED_EVENT_CAPACITY);
    }
}

/// A reusable, custom widget that uses The Elm Architecture.
///
/// A [`Component`] allows you to implement custom widgets as if they were
/// `taim` applications with encapsulated state.
///
/// In other words, a [`Component`] allows you to turn `taim` applications into
/// custom widgets and embed them without cumbersome wiring.
///
/// A [`Component`] produces widgets that may fire an [`Event`](Component::Event)
/// and update the internal state of the [`Component`].
///
/// Additionally, a [`Component`] is capable of producing a `Message` to notify
/// the parent application of any relevant interactions.
///
/// # State
/// A component can store its state in one of two ways: either as data within the
/// implementor of the trait, or in a type [`State`][Component::State] that is managed
/// by the runtime and provided to the trait methods. These two approaches are not
/// mutually exclusive and have opposite pros and cons.
///
/// For instance, if a piece of state is needed by multiple components that reside
/// in different branches of the tree, then it's more convenient to let a common
/// ancestor store it and pass it down.
///
/// On the other hand, if a piece of state is only needed by the component itself,
/// you can store it as part of its internal [`State`][Component::State].
#[cfg(feature = "lazy")]
#[deprecated(
    since = "0.13.0",
    note = "components introduce encapsulated state and hamper the use of a single source of truth. \
    Instead, leverage the Elm Architecture directly, or implement a custom widget"
)]
pub trait Component<Message, Theme = crate::Theme, Renderer = crate::Renderer> {
    /// The internal state of this [`Component`].
    type State: Default;

    /// The type of event this [`Component`] handles internally.
    type Event;

    /// Processes an [`Event`](Component::Event) and updates the [`Component`] state accordingly.
    ///
    /// It can produce a `Message` for the parent application.
    fn update(&mut self, state: &mut Self::State, event: Self::Event) -> Option<Message>;

    /// Produces the widgets of the [`Component`], which may trigger an [`Event`](Component::Event)
    /// on user interaction.
    fn view(&self, state: &Self::State) -> Element<'_, Self::Event, Theme, Renderer>;

    /// Update the [`Component`] state based on the provided [`Operation`](widget::Operation)
    ///
    /// By default, it does nothing.
    fn operate(
        &self,
        _bounds: Rectangle,
        _state: &mut Self::State,
        _operation: &mut dyn widget::Operation,
    ) {
    }

    /// Returns a [`Size`] hint for laying out the [`Component`].
    ///
    /// This hint may be used by some widget containers to adjust their sizing strategy
    /// during construction.
    fn size_hint(&self) -> Size<Length> {
        Size {
            width: Length::Shrink,
            height: Length::Shrink,
        }
    }
}

struct Tag<T>(T);

/// Turns an implementor of [`Component`] into an [`Element`] that can be
/// embedded in any application.
pub fn view<'a, C, Message, Theme, Renderer>(component: C) -> Element<'a, Message, Theme, Renderer>
where
    C: Component<Message, Theme, Renderer> + 'a,
    C::State: 'static,
    Message: 'a,
    Theme: 'a,
    Renderer: core::Renderer + 'a,
{
    Element::new(Instance {
        state: RefCell::new(Some(dormant_state(Box::new(component)))),
        tree: RefCell::new(Rc::new(RefCell::new(None))),
        events: Vec::new(),
    })
}

struct Instance<'a, Message, Theme, Renderer, Event, S> {
    state: RefCell<Option<State<'a, Message, Theme, Renderer, Event, S>>>,
    tree: RefCell<Rc<RefCell<Option<Tree>>>>,
    events: Vec<Event>,
}

type DynComponent<'a, Message, Theme, Renderer, Event, S> =
    dyn Component<Message, Theme, Renderer, Event = Event, State = S> + 'a;

#[self_referencing]
struct State<'a, Message: 'a, Theme: 'a, Renderer: 'a, Event: 'a, S: 'a> {
    component: Box<DynComponent<'a, Message, Theme, Renderer, Event, S>>,
    types: PhantomData<fn() -> (Message, S)>,

    #[borrows(component)]
    #[covariant]
    element: Option<Element<'this, Event, Theme, Renderer>>,
}

fn dormant_state<'a, Message, Theme, Renderer, Event, S>(
    component: Box<DynComponent<'a, Message, Theme, Renderer, Event, S>>,
) -> State<'a, Message, Theme, Renderer, Event, S> {
    StateBuilder {
        component,
        types: PhantomData,
        element_builder: |_| None,
    }
    .build()
}

impl<Message, Theme, Renderer, Event, S> Instance<'_, Message, Theme, Renderer, Event, S>
where
    S: Default + 'static,
    Renderer: renderer::Renderer,
{
    fn diff_self(&self) {
        let tree = Rc::clone(&self.tree.borrow());

        self.with_element_mut(|element| {
            tree.borrow_mut()
                .as_mut()
                .unwrap()
                .diff_children(std::slice::from_mut(element));
        });
    }

    fn rebuild(&self, operation: Option<(Rectangle, &mut dyn widget::Operation)>) {
        let inner = self.state.borrow_mut().take().unwrap();

        if inner.borrow_element().is_some() && operation.is_none() {
            *self.state.borrow_mut() = Some(inner);
            return;
        }

        let tree = Rc::clone(&self.tree.borrow());
        let heads = inner.into_heads();

        if let Some((bounds, operation)) = operation {
            heads.component.operate(
                bounds,
                tree.borrow_mut().as_mut().unwrap().state.downcast_mut(),
                operation,
            );
        }

        *self.state.borrow_mut() = Some(
            StateBuilder {
                component: heads.component,
                types: PhantomData,
                element_builder: |component| {
                    Some(
                        component.view(
                            self.tree
                                .borrow()
                                .borrow()
                                .as_ref()
                                .unwrap()
                                .state
                                .downcast_ref(),
                        ),
                    )
                },
            }
            .build(),
        );
        self.diff_self();
    }

    #[inline]
    fn rebuild_element_if_necessary(&self) {
        self.rebuild(None);
    }

    #[inline]
    fn rebuild_element_with_operation(
        &self,
        layout: Layout<'_>,
        operation: &mut dyn widget::Operation,
    ) {
        self.rebuild(Some((layout.bounds(), operation)));
    }

    fn with_element<T>(&self, f: impl FnOnce(&Element<'_, Event, Theme, Renderer>) -> T) -> T {
        self.with_element_mut(|element| f(element))
    }

    fn with_element_mut<T>(
        &self,
        f: impl FnOnce(&mut Element<'_, Event, Theme, Renderer>) -> T,
    ) -> T {
        self.rebuild_element_if_necessary();
        self.state
            .borrow_mut()
            .as_mut()
            .unwrap()
            .with_element_mut(|element| f(element.as_mut().unwrap()))
    }
}

struct Overlay<'a, 'b, Message, Theme, Renderer, Event, S>(
    Option<Inner<'a, 'b, Message, Theme, Renderer, Event, S>>,
);

#[self_referencing]
struct Inner<'a, 'b, Message, Theme, Renderer, Event, S> {
    instance: &'a mut Instance<'b, Message, Theme, Renderer, Event, S>,
    tree: Tree,
    types: PhantomData<(Message, Event, S)>,

    #[borrows(mut instance, mut tree)]
    #[not_covariant]
    overlay: Option<RefCell<overlay::Nested<'this, Event, Theme, Renderer>>>,
}

struct OverlayInstance<'a, 'b, Message, Theme, Renderer, Event, S> {
    overlay: Option<Overlay<'a, 'b, Message, Theme, Renderer, Event, S>>,
    events: Vec<Event>,
}
