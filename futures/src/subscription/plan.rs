//! Internal recipe composition.

use super::{EventStream, Hasher, Recipe};
use crate::futures::{self, Stream, StreamExt as _};
use crate::{BoxStream, MaybeSend};

use std::any::TypeId;
use std::hash::Hash;

pub(super) struct Source<Identity, Build> {
    identity: Identity,
    build: Build,
}

impl<Identity, Build> Source<Identity, Build> {
    pub(super) fn new(identity: Identity, build: Build) -> Self {
        Self { identity, build }
    }
}

impl<Identity, Build, OutputStream, Output> Recipe for Source<Identity, Build>
where
    Identity: Hash + 'static,
    Build: FnOnce(&Identity, EventStream) -> OutputStream,
    OutputStream: Stream<Item = Output> + MaybeSend + 'static,
{
    type Output = Output;

    fn hash(&self, state: &mut Hasher) {
        TypeId::of::<Identity>().hash(state);
        self.identity.hash(state);
    }

    fn stream(self: Box<Self>, input: EventStream) -> BoxStream<Output> {
        crate::boxed_stream((self.build)(&self.identity, input))
    }
}

pub(super) struct With<Input, Context> {
    source: Box<dyn Recipe<Output = Input>>,
    context: Context,
}

impl<Input, Context> With<Input, Context> {
    pub(super) fn new(source: Box<dyn Recipe<Output = Input>>, context: Context) -> Self {
        Self { source, context }
    }
}

impl<Input, Context> Recipe for With<Input, Context>
where
    Input: 'static,
    Context: Hash + Clone + Send + Sync + 'static,
{
    type Output = (Context, Input);

    fn hash(&self, state: &mut Hasher) {
        TypeId::of::<Context>().hash(state);
        self.context.hash(state);
        self.source.hash(state);
    }

    fn stream(self: Box<Self>, input: EventStream) -> BoxStream<Self::Output> {
        let context = self.context;
        Box::pin(
            self.source
                .stream(input)
                .map(move |value| (context.clone(), value)),
        )
    }
}

pub(super) struct Map<Input, Output, Mapper>
where
    Mapper: Fn(Input) -> Output,
{
    source: Box<dyn Recipe<Output = Input>>,
    mapper: Mapper,
}

impl<Input, Output, Mapper> Map<Input, Output, Mapper>
where
    Mapper: Fn(Input) -> Output,
{
    pub(super) fn new(source: Box<dyn Recipe<Output = Input>>, mapper: Mapper) -> Self {
        Self { source, mapper }
    }
}

impl<Input, Output, Mapper> Recipe for Map<Input, Output, Mapper>
where
    Input: 'static,
    Output: 'static,
    Mapper: Fn(Input) -> Output + MaybeSend + 'static,
{
    type Output = Output;

    fn hash(&self, state: &mut Hasher) {
        TypeId::of::<Mapper>().hash(state);
        self.source.hash(state);
    }

    fn stream(self: Box<Self>, input: EventStream) -> BoxStream<Output> {
        Box::pin(self.source.stream(input).map(self.mapper))
    }
}

pub(super) struct FilterMap<Input, Output, Mapper>
where
    Mapper: Fn(Input) -> Option<Output>,
{
    source: Box<dyn Recipe<Output = Input>>,
    mapper: Mapper,
}

impl<Input, Output, Mapper> FilterMap<Input, Output, Mapper>
where
    Mapper: Fn(Input) -> Option<Output>,
{
    pub(super) fn new(source: Box<dyn Recipe<Output = Input>>, mapper: Mapper) -> Self {
        Self { source, mapper }
    }
}

impl<Input, Output, Mapper> Recipe for FilterMap<Input, Output, Mapper>
where
    Input: 'static,
    Output: MaybeSend + 'static,
    Mapper: Fn(Input) -> Option<Output> + MaybeSend + 'static,
{
    type Output = Output;

    fn hash(&self, state: &mut Hasher) {
        TypeId::of::<Mapper>().hash(state);
        self.source.hash(state);
    }

    fn stream(self: Box<Self>, input: EventStream) -> BoxStream<Output> {
        let mapper = self.mapper;
        Box::pin(
            self.source
                .stream(input)
                .filter_map(move |value| futures::future::ready(mapper(value))),
        )
    }
}
