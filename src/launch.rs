//! Starting an application written against the TAIM view API.
use crate::{Element, Result};

use taim_view::host::Root;
use taim_view::{AnyView, Host, Scope};

use std::cell::RefCell;

/// Opens a window titled `title` and shows the view `build` returns.
///
/// `build` runs once. It creates the window's state with
/// [`Scope::signal`](taim_view::Scope::signal) and returns the root view;
/// from then on the view tree updates itself as that state changes.
///
/// ```no_run
/// use taim::prelude::*;
///
/// fn main() -> taim::Result {
///     taim::launch("Counter", |cx| {
///         let count = cx.signal(0);
///
///         column(views![
///             text(move || format!("Count: {}", count.get())),
///             button("+1").on_press(move || count.update(|n| *n += 1)),
///         ])
///     })
/// }
/// ```
pub fn launch<V>(title: &'static str, build: impl FnOnce(&mut Scope) -> V + 'static) -> Result
where
    V: Into<AnyView>,
{
    let build = RefCell::new(Some(build));

    crate::application(
        move || {
            let build = build
                .borrow_mut()
                .take()
                .expect("a TAIM window is built exactly once");

            RefCell::new(Root::new(build))
        },
        |_root: &mut RefCell<Root>, _: ()| {},
        show,
    )
    .title(title)
    .theme(|root: &RefCell<Root>| root.borrow().theme())
    .scale_factor(|root: &RefCell<Root>| root.borrow().scale())
    .run()
}

fn show(root: &RefCell<Root>) -> Element<'_, ()> {
    Host::new(root).into()
}

/// Everything an application written against the view API needs.
pub mod prelude {
    pub use taim_view::prelude::*;
}
