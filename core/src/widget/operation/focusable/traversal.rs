use super::{Count, Focusable};
use crate::Rectangle;
use crate::widget::Id;
use crate::widget::operation::{self, Operation, Outcome};

pub(super) struct SetFocus {
    target: Option<Id>,
}

impl SetFocus {
    pub(super) fn target(target: Id) -> Self {
        Self {
            target: Some(target),
        }
    }

    pub(super) fn clear() -> Self {
        Self { target: None }
    }
}

impl<T> Operation<T> for SetFocus {
    fn focusable(&mut self, id: Option<&Id>, _bounds: Rectangle, state: &mut dyn Focusable) {
        if id
            .zip(self.target.as_ref())
            .is_some_and(|(id, target)| id == target)
        {
            state.focus();
        } else {
            state.unfocus();
        }
    }

    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation<T>)) {
        visit(self);
    }
}

pub(super) struct Counter(Count);

impl Counter {
    pub(super) fn new() -> Self {
        Self(Count::default())
    }
}

impl Operation<Count> for Counter {
    fn focusable(&mut self, _id: Option<&Id>, _bounds: Rectangle, state: &mut dyn Focusable) {
        if state.is_focused() {
            self.0.focused = Some(self.0.total);
        }

        self.0.total = self.0.total.saturating_add(1);
    }

    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation<Count>)) {
        visit(self);
    }

    fn finish(&self) -> Outcome<Count> {
        Outcome::Some(self.0)
    }
}

#[derive(Clone, Copy)]
pub(super) enum Direction {
    Forward,
    Backward,
}

pub(super) fn move_focus<T>(
    count: impl Operation<Count> + 'static,
    direction: Direction,
) -> impl Operation<T>
where
    T: Send + 'static,
{
    let build: fn(Count) -> FocusMove = match direction {
        Direction::Forward => |count| FocusMove::new(count, Direction::Forward),
        Direction::Backward => |count| FocusMove::new(count, Direction::Backward),
    };

    operation::then(count, build)
}

/// Executes at most two mutations: clear the old slot and select the new one.
/// Once both are seen, traversal stops before entering any later subtree.
struct FocusMove {
    select: Option<usize>,
    clear: Option<usize>,
    index: usize,
    pending: u8,
}

impl FocusMove {
    const SELECT: u8 = 1 << 0;
    const CLEAR: u8 = 1 << 1;

    fn new(count: Count, direction: Direction) -> Self {
        let select = match (count.focused, direction) {
            (None, Direction::Forward) => (count.total > 0).then_some(0),
            (None, Direction::Backward) => count.total.checked_sub(1),
            (Some(index), Direction::Forward) => {
                index.checked_add(1).filter(|next| *next < count.total)
            }
            (Some(index), Direction::Backward) => index.checked_sub(1),
        };

        let pending = u8::from(select.is_some()) * Self::SELECT
            | u8::from(count.focused.is_some()) * Self::CLEAR;

        Self {
            select,
            clear: count.focused,
            index: 0,
            pending,
        }
    }
}

impl<T> Operation<T> for FocusMove {
    fn focusable(&mut self, _id: Option<&Id>, _bounds: Rectangle, state: &mut dyn Focusable) {
        if self.pending == 0 {
            return;
        }

        if self.select == Some(self.index) {
            state.focus();
            self.pending &= !Self::SELECT;
        }

        if self.clear == Some(self.index) {
            state.unfocus();
            self.pending &= !Self::CLEAR;
        }

        self.index = self.index.saturating_add(1);
    }

    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation<T>)) {
        if self.pending != 0 {
            visit(self);
        }
    }
}

pub(super) struct FindFocused {
    result: Option<Id>,
}

impl FindFocused {
    pub(super) fn new() -> Self {
        Self { result: None }
    }
}

impl Operation<Id> for FindFocused {
    fn focusable(&mut self, id: Option<&Id>, _bounds: Rectangle, state: &mut dyn Focusable) {
        if self.result.is_none() && state.is_focused() {
            self.result = id.cloned();
        }
    }

    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation<Id>)) {
        if self.result.is_none() {
            visit(self);
        }
    }

    fn finish(&self) -> Outcome<Id> {
        self.result.clone().map_or(Outcome::None, Outcome::Some)
    }
}

pub(super) struct FocusQuery {
    target: Id,
    result: Option<bool>,
}

impl FocusQuery {
    pub(super) fn new(target: Id) -> Self {
        Self {
            target,
            result: None,
        }
    }
}

impl Operation<bool> for FocusQuery {
    fn focusable(&mut self, id: Option<&Id>, _bounds: Rectangle, state: &mut dyn Focusable) {
        if id == Some(&self.target) {
            self.result = Some(state.is_focused());
        }
    }

    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation<bool>)) {
        if self.result.is_none() {
            visit(self);
        }
    }

    fn finish(&self) -> Outcome<bool> {
        self.result.map_or(Outcome::None, Outcome::Some)
    }
}
