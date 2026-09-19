#![allow(clippy::await_holding_refcell_ref, clippy::type_complexity)]
pub(crate) mod helpers;

pub mod component;

#[allow(deprecated)]
pub use component::Component;

mod cache;
mod implementation;
mod overlay_runtime;

use crate::core::Element;
use crate::core::layout::Layout;
use crate::core::overlay;
use crate::core::widget::tree::Tree;
use crate::core::{self};

use ouroboros::self_referencing;
use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;

/// A widget that only rebuilds its contents when necessary.
#[cfg(feature = "lazy")]
pub struct Lazy<'a, Message, Theme, Renderer, Dependency, View> {
    dependency: Dependency,
    view: Box<dyn Fn(&Dependency) -> View + 'a>,
    element: RefCell<Option<Rc<RefCell<Option<Element<'static, Message, Theme, Renderer>>>>>>,
}

impl<'a, Message, Theme, Renderer, Dependency, View>
    Lazy<'a, Message, Theme, Renderer, Dependency, View>
where
    Dependency: Hash + 'a,
    View: Into<Element<'static, Message, Theme, Renderer>>,
{
    /// Creates a new [`Lazy`] widget with the given data `Dependency` and a
    /// closure that can turn this data into a widget tree.
    pub fn new(dependency: Dependency, view: impl Fn(&Dependency) -> View + 'a) -> Self {
        Self {
            dependency,
            view: Box::new(view),
            element: RefCell::new(None),
        }
    }

    pub(crate) fn replace_dependency(&mut self, dependency: Dependency) {
        self.dependency = dependency;
    }

    fn with_element<T>(&self, f: impl FnOnce(&Element<'_, Message, Theme, Renderer>) -> T) -> T {
        f(self
            .element
            .borrow()
            .as_ref()
            .unwrap()
            .borrow()
            .as_ref()
            .unwrap())
    }

    fn with_element_mut<T>(
        &self,
        f: impl FnOnce(&mut Element<'_, Message, Theme, Renderer>) -> T,
    ) -> T {
        f(self
            .element
            .borrow()
            .as_ref()
            .unwrap()
            .borrow_mut()
            .as_mut()
            .unwrap())
    }

    /// Reconnects a freshly rebuilt lazy widget to the element retained in
    /// its persistent tree state.
    ///
    /// Most containers keep one widget value alive from diff through draw.
    /// Virtualized containers intentionally rebuild lightweight item widgets
    /// for each phase, so their new `Lazy` value has an empty bridge even
    /// though its [`Tree`] still owns the cached element. Reattaching here
    /// makes lazy and reactive items valid virtual-list children without
    /// rebuilding their subtrees.
    fn ensure_element(&self, tree: &Tree)
    where
        Message: 'static,
        Theme: 'static,
        Renderer: 'static,
    {
        if self.element.borrow().is_some() {
            return;
        }

        let current = tree
            .state
            .downcast_ref::<Internal<Message, Theme, Renderer>>();
        let _ = self.element.replace(Some(current.element.clone()));
    }
}

struct Internal<Message, Theme, Renderer> {
    element: Rc<RefCell<Option<Element<'static, Message, Theme, Renderer>>>>,
    hash: u64,
    layer_key: Option<u64>,
}

#[self_referencing]
struct Inner<'a, Message: 'a, Theme: 'a, Renderer: 'a> {
    cell: Rc<RefCell<Option<Element<'static, Message, Theme, Renderer>>>>,
    element: Element<'static, Message, Theme, Renderer>,
    tree: &'a mut Tree,
    layout: Layout<'a>,

    #[borrows(mut element, mut tree, layout)]
    #[not_covariant]
    overlay: Option<RefCell<overlay::Nested<'this, Message, Theme, Renderer>>>,
}

struct Overlay<'a, Message, Theme, Renderer>(Option<Inner<'a, Message, Theme, Renderer>>);

impl<'a, Message, Theme, Renderer, Dependency, View>
    From<Lazy<'a, Message, Theme, Renderer, Dependency, View>>
    for Element<'a, Message, Theme, Renderer>
where
    View: Into<Element<'static, Message, Theme, Renderer>> + 'static,
    Renderer: core::Renderer + 'static,
    Message: 'static,
    Theme: 'static,
    Dependency: Hash + 'a,
{
    fn from(lazy: Lazy<'a, Message, Theme, Renderer, Dependency, View>) -> Self {
        Self::new(lazy)
    }
}
