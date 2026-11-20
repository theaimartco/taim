use crate::cx::{DescribeCx, MeasureCx, PaintCx};
use crate::layout::{Constraints, Sizing};
use crate::state::{self, IntoProp, Invalidation, Prop};
use crate::views::hero::{self, Stage};
use crate::views::stack::{Stack, column};
use crate::{AnyView, Child, Point, Size, View, semantics};

use std::sync::Arc;

/// Shows `view()` while `condition` holds, and nothing otherwise.
///
/// `view` runs again each time the condition becomes true, so the shown
/// subtree starts fresh.
pub fn when<V: Into<AnyView>>(
    condition: impl IntoProp<bool>,
    view: impl Fn() -> V + 'static,
) -> Rebuild<bool> {
    rebuild(
        condition,
        move |shown| {
            if *shown { Some(view().into()) } else { None }
        },
    )
}

/// One view per item of `items`, stacked vertically. Items that did not
/// change keep their views (and those views' state).
pub fn each<T, V>(items: impl IntoProp<Vec<T>>, view: impl Fn(&T) -> V + 'static) -> Each<T>
where
    T: Clone + PartialEq + Send + Sync + 'static,
    V: Into<AnyView>,
{
    Each {
        items: items.into_prop(),
        view: Box::new(move |item| view(item).into()),
        shown: Vec::new(),
        stack: column([]),
        window: state::building(),
        stage: hero::current(),
    }
}

/// Rebuilds its child from `build(&key)` whenever `key` changes.
pub fn rebuild<K>(
    key: impl IntoProp<K>,
    build: impl Fn(&K) -> Option<AnyView> + 'static,
) -> Rebuild<K>
where
    K: Clone + PartialEq + Send + Sync + 'static,
{
    Rebuild {
        key: key.into_prop(),
        build: Box::new(build),
        built_for: None,
        child: None,
        window: state::building(),
        stage: hero::current(),
    }
}

/// See [`rebuild`] and [`when`].
pub struct Rebuild<K> {
    key: Prop<K>,
    build: Box<dyn Fn(&K) -> Option<AnyView>>,
    built_for: Option<K>,
    child: Option<Child>,
    window: Option<Arc<Invalidation>>,
    stage: Option<Stage>,
}

impl<K: Clone + PartialEq + Send + Sync + 'static> Rebuild<K> {
    fn refresh(&mut self) {
        let key = self.key.get();
        if self.built_for.as_ref() == Some(&key) {
            return;
        }

        self.child = state::rebuilding(self.window.as_ref(), || {
            hero::restaged(self.stage.as_ref(), || (self.build)(&key))
        })
        .map(Child::new);
        self.built_for = Some(key);
    }
}

impl<K: Clone + PartialEq + Send + Sync + 'static> View for Rebuild<K> {
    fn sizing(&self) -> Sizing {
        self.child.as_ref().map_or(Sizing::HUG, Child::sizing)
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        self.refresh();

        match &mut self.child {
            Some(child) => {
                let size = child.measure(cx, constraints);
                child.place(Point::ORIGIN);
                size
            }
            None => Size::ZERO,
        }
    }

    fn paint(&self, _cx: &mut PaintCx<'_>) {}

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        if let Some(child) = &mut self.child {
            visit(child);
        }
    }
}

/// See [`each`].
pub struct Each<T> {
    items: Prop<Vec<T>>,
    view: Box<dyn Fn(&T) -> AnyView>,
    shown: Vec<T>,
    stack: Stack,
    window: Option<Arc<Invalidation>>,
    stage: Option<Stage>,
}

impl<T: Clone + PartialEq + Send + Sync + 'static> Each<T> {
    /// Space between items.
    pub fn gap(mut self, gap: f32) -> Self {
        self.stack = self.stack.gap(gap);
        self
    }

    fn refresh(&mut self) {
        let items = self.items.get();
        if items == self.shown {
            return;
        }

        let previous = std::mem::take(&mut self.shown);
        let mut kept = self.stack.take_children().into_iter();
        let mut children = Vec::with_capacity(items.len());

        for (index, item) in items.iter().enumerate() {
            let reused = kept.next().filter(|_| previous.get(index) == Some(item));
            children.push(reused.unwrap_or_else(|| {
                Child::new(state::rebuilding(self.window.as_ref(), || {
                    hero::restaged(self.stage.as_ref(), || (self.view)(item))
                }))
            }));
        }

        self.stack.set_children(children);
        self.shown = items;
    }
}

impl<T: Clone + PartialEq + Send + Sync + 'static> View for Each<T> {
    fn sizing(&self) -> Sizing {
        self.stack.sizing()
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        self.refresh();
        self.stack.measure(cx, constraints)
    }

    fn paint(&self, _cx: &mut PaintCx<'_>) {}

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        self.stack.children(visit);
    }

    fn children_rev(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        self.stack.children_rev(visit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Scope;
    use crate::views::text;

    #[test]
    fn when_builds_only_while_the_condition_holds() {
        let mut scope = Scope::new(Arc::default());
        let open = scope.signal(false);
        let mut view = scope.build(|_| when(open, || text("details")));

        view.refresh();
        assert!(view.child.is_none());

        let _ = open.set(true);
        view.refresh();
        assert!(view.child.is_some());
    }

    #[test]
    fn each_keeps_views_of_unchanged_items() {
        let mut scope = Scope::new(Arc::default());
        let items = scope.signal(vec!["a", "b"]);
        let mut list = scope.build(|_| each(items, |item: &&str| text(*item)));

        list.refresh();
        let first = list.stack.children_ptrs();

        items.update(|items| items.push("c"));
        list.refresh();
        let second = list.stack.children_ptrs();

        assert_eq!(second.len(), 3);
        assert_eq!(first[..], second[..2], "a and b were reused");
    }
}
