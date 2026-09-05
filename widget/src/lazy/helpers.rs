use crate::core::{self, Element};
use crate::lazy::component;

use std::hash::Hash;

#[allow(deprecated)]
pub use crate::lazy::{Component, Lazy};

/// Creates a new [`Lazy`] widget with the given data `Dependency` and a
/// closure that can turn this data into a widget tree.
#[cfg(feature = "lazy")]
pub fn lazy<'a, Message, Theme, Renderer, Dependency, View>(
    dependency: Dependency,
    view: impl Fn(&Dependency) -> View + 'a,
) -> Lazy<'a, Message, Theme, Renderer, Dependency, View>
where
    Dependency: Hash + 'a,
    View: Into<Element<'static, Message, Theme, Renderer>>,
{
    Lazy::new(dependency, view)
}

/// Creates a lazily rebuilt subtree from a reactive source.
///
/// The subtree is reconstructed only when `source` advances its revision. An
/// ordinary application `view` may therefore be rebuilt while every unchanged
/// reactive subtree preserves its widget objects and state.
#[cfg(feature = "signal")]
pub fn reactive<'a, Message, Theme, Renderer, Source, View>(
    source: Source,
    view: impl Fn(Source::Value) -> View + 'a,
) -> crate::Reactive<'a, Message, Theme, Renderer, Source, View>
where
    Source: taim_signal::Readable + 'a,
    View: Into<Element<'static, Message, Theme, Renderer>>,
{
    crate::Reactive::new(source, view)
}

/// Creates a reactive subtree that promises to preserve layout and semantics.
///
/// Use this for visual-only values such as a fill colour or opacity. Violating
/// the geometry promise can reuse a stale layout; use [`reactive`] whenever a
/// source may affect size, structure, or semantic content.
#[cfg(feature = "signal")]
pub fn reactive_paint<'a, Message, Theme, Renderer, Source, View>(
    source: Source,
    view: impl Fn(Source::Value) -> View + 'a,
) -> crate::Reactive<'a, Message, Theme, Renderer, Source, View>
where
    Source: taim_signal::Readable + 'a,
    View: Into<Element<'static, Message, Theme, Renderer>>,
{
    crate::Reactive::new(source, view).paint_only()
}

/// Creates a fixed reactive subtree that reads its live source during paint.
///
/// Unlike [`reactive_paint`], this does not reconstruct or diff the subtree
/// after a source change. The view receives the source itself and must keep its
/// widget structure, geometry, and semantics independent of the current value.
#[cfg(feature = "signal")]
pub fn reactive_value<'a, Message, Theme, Renderer, Source, View>(
    source: Source,
    view: impl Fn(Source) -> View + 'a,
) -> crate::Reactive<'a, Message, Theme, Renderer, Source, View>
where
    Source: taim_signal::Readable + 'a,
    View: Into<Element<'static, Message, Theme, Renderer>>,
{
    crate::Reactive::value(source, view)
}

/// Turns an implementor of [`Component`] into an [`Element`] that can be
/// embedded in any application.
#[cfg(feature = "lazy")]
#[deprecated(
    since = "0.13.0",
    note = "components introduce encapsulated state and hamper the use of a single source of truth. \
    Instead, leverage the Elm Architecture directly, or implement a custom widget"
)]
#[allow(deprecated)]
pub fn component<'a, C, Message, Theme, Renderer>(
    component: C,
) -> Element<'a, Message, Theme, Renderer>
where
    C: Component<Message, Theme, Renderer> + 'a,
    C::State: 'static,
    Message: 'a,
    Theme: 'a,
    Renderer: core::Renderer + 'a,
{
    component::view(component)
}
