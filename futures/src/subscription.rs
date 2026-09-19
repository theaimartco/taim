//! Declarative, identity-stable streams for TAIM applications.
//!
//! Most subscriptions contain one recipe. TAIM keeps that recipe inline in
//! the subscription value, avoiding the extra `Vec` allocation traditionally
//! paid before the stream itself is even started.

mod plan;
mod tracker;

pub use tracker::Tracker;

use crate::core::event;
use crate::core::theme;
use crate::core::window;
use crate::futures::Stream;
use crate::{BoxStream, MaybeSend};

use smallvec::SmallVec;
use std::hash::Hash;

/// An event delivered to subscription recipes by the application runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A user interacted with a window.
    Interaction {
        /// Window receiving the interaction.
        window: window::Id,
        /// Normalized input or window event.
        event: event::Event,
        /// Whether a widget consumed the event.
        status: event::Status,
    },
    /// The operating-system theme changed.
    SystemThemeChanged(theme::Mode),
    /// An event emitted by a platform adapter.
    PlatformSpecific(PlatformSpecific),
}

/// Events whose payload is meaningful only on one platform family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformSpecific {
    /// A macOS application event.
    MacOS(MacOS),
}

/// Events emitted by the TAIM macOS adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacOS {
    /// The operating system opened a URL with the application.
    ReceivedUrl(String),
}

/// Runtime-event input available to every [`Recipe`].
pub type EventStream = BoxStream<Event>;

/// Stable, fast hasher used for recipe identities inside one process.
pub type Hasher = rustc_hash::FxHasher;

type RecipeBox<T> = Box<dyn Recipe<Output = T>>;
type RecipeStorage<T> = SmallVec<[RecipeBox<T>; 1]>;

/// A declarative set of long-lived streams.
///
/// Returning the same recipe identity on a later application update keeps its
/// existing stream alive. Omitting it cancels the stream. A single recipe is
/// stored inline; batches spill into contiguous heap storage only when needed.
#[must_use = "return the Subscription to the TAIM runtime for it to run"]
pub struct Subscription<T> {
    recipes: RecipeStorage<T>,
}

impl<T> Subscription<T> {
    /// Creates a subscription with no active streams and no allocation.
    pub fn none() -> Self {
        Self {
            recipes: SmallVec::new(),
        }
    }

    /// Builds a stream from a stateless function.
    pub fn run<S>(builder: fn() -> S) -> Self
    where
        S: Stream<Item = T> + MaybeSend + 'static,
        T: 'static,
    {
        from_recipe(plan::Source::new(
            builder,
            |builder: &fn() -> S, _: EventStream| builder(),
        ))
    }

    /// Builds a stream whose stable identity includes `data` and `builder`.
    pub fn run_with<Data, S>(data: Data, builder: fn(&Data) -> S) -> Self
    where
        Data: Hash + 'static,
        S: Stream<Item = T> + MaybeSend + 'static,
        T: 'static,
    {
        from_recipe(plan::Source::new(
            (data, builder),
            |source: &(Data, fn(&Data) -> S), _: EventStream| (source.1)(&source.0),
        ))
    }

    /// Flattens subscriptions into one contiguous recipe set.
    pub fn batch(subscriptions: impl IntoIterator<Item = Self>) -> Self {
        let mut recipes = RecipeStorage::new();

        for subscription in subscriptions {
            recipes.extend(subscription.recipes);
        }

        Self { recipes }
    }

    /// Adds a hashable context value to every recipe identity and output.
    pub fn with<Context>(self, value: Context) -> Subscription<(Context, T)>
    where
        T: 'static,
        Context: Hash + Clone + Send + Sync + 'static,
    {
        let recipes = self
            .recipes
            .into_iter()
            .map(|recipe| {
                Box::new(plan::With::new(recipe, value.clone())) as RecipeBox<(Context, T)>
            })
            .collect();

        Subscription { recipes }
    }

    /// Maps each stream output without changing stream lifetime.
    ///
    /// `mapper` must not capture state. Use [`Self::with`] when state must be
    /// included in the subscription identity.
    pub fn map<Mapper, Output>(self, mapper: Mapper) -> Subscription<Output>
    where
        T: 'static,
        Mapper: Fn(T) -> Output + MaybeSend + Clone + 'static,
        Output: 'static,
    {
        assert_zero_sized::<Mapper>();

        let recipes = self
            .recipes
            .into_iter()
            .map(|recipe| Box::new(plan::Map::new(recipe, mapper.clone())) as RecipeBox<Output>)
            .collect();

        Subscription { recipes }
    }

    /// Maps outputs and discards values for which `mapper` returns `None`.
    ///
    /// `mapper` must not capture state. Use [`Self::with`] when state must be
    /// included in the subscription identity.
    pub fn filter_map<Mapper, Output>(self, mapper: Mapper) -> Subscription<Output>
    where
        T: MaybeSend + 'static,
        Mapper: Fn(T) -> Option<Output> + MaybeSend + Clone + 'static,
        Output: MaybeSend + 'static,
    {
        assert_zero_sized::<Mapper>();

        let recipes = self
            .recipes
            .into_iter()
            .map(|recipe| {
                Box::new(plan::FilterMap::new(recipe, mapper.clone())) as RecipeBox<Output>
            })
            .collect();

        Subscription { recipes }
    }

    /// Returns the number of independently tracked streams.
    pub fn units(&self) -> usize {
        self.recipes.len()
    }
}

impl<T> Default for Subscription<T> {
    fn default() -> Self {
        Self::none()
    }
}

impl<T> std::fmt::Debug for Subscription<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Subscription")
            .field("units", &self.units())
            .finish()
    }
}

/// Wraps one custom recipe in an inline subscription.
pub fn from_recipe<T>(recipe: impl Recipe<Output = T> + 'static) -> Subscription<T> {
    let recipe: RecipeBox<T> = Box::new(recipe);

    Subscription {
        recipes: SmallVec::from_buf([recipe]),
    }
}

/// Consumes a subscription and yields its recipes without another allocation.
pub fn into_recipes<T>(
    subscription: Subscription<T>,
) -> impl ExactSizeIterator<Item = RecipeBox<T>> {
    subscription.recipes.into_iter()
}

/// Definition of a long-lived stream and its stable identity.
pub trait Recipe {
    /// Values produced by the stream.
    type Output;

    /// Adds every identity-bearing field to `state`.
    fn hash(&self, state: &mut Hasher);

    /// Materializes the stream with access to runtime events.
    fn stream(self: Box<Self>, input: EventStream) -> BoxStream<Self::Output>;
}

/// Creates an event-filtering subscription with a caller-defined identity.
pub fn filter_map<Identity, Filter, Output>(
    identity: Identity,
    filter: Filter,
) -> Subscription<Output>
where
    Identity: Hash + 'static,
    Filter: Fn(Event) -> Option<Output> + MaybeSend + 'static,
    Output: MaybeSend + 'static,
{
    from_recipe(plan::Source::new(
        identity,
        |_: &Identity, events: EventStream| {
            use crate::futures::StreamExt;
            use crate::futures::future;

            events.filter_map(move |event| future::ready(filter(event)))
        },
    ))
}

const fn assert_zero_sized<T>() {
    assert!(
        size_of::<T>() == 0,
        "subscription mappers cannot capture state; attach identity-bearing state with Subscription::with"
    );
}
