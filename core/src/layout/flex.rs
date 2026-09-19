//! Distribute elements using a flex-based layout.
// This code is heavily inspired by the [`druid`] codebase.
//
// [`druid`]: https://github.com/xi-editor/druid
//
// Copyright 2018 The xi-editor Authors, Héctor Ramón
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// NOTICE (Apache License 2.0 section 4(b)): this file was modified from
// its original `druid` derivation to (1) store per-item bookkeeping and
// the resulting nodes in stack-allocated `SmallVec`s instead of `Vec`s,
// avoiding a heap allocation for the common case of a small, fixed
// number of children, and (2) compute and attach a first-baseline offset
// to the produced `Node`, derived from its children's own baselines.
// The multi-pass constraint-solving algorithm itself is unchanged.
use crate::Element;

use crate::layout::{Limits, Node};
use crate::length;
use crate::widget;
use crate::{Alignment, Length, Padding, Size};

/// The number of children a [`resolve`] call can lay out without
/// spilling its bookkeeping storage onto the heap.
///
/// Chosen to comfortably cover the common case (a handful of buttons or
/// fields in a row/column); a container with more children than this
/// still works correctly, it simply allocates like the previous,
/// `Vec`-only implementation did unconditionally.
pub(super) const INLINE_CHILDREN: usize = 8;

/// The main axis of a flex layout.
#[derive(Debug)]
pub enum Axis {
    /// The horizontal axis
    Horizontal,

    /// The vertical axis
    Vertical,
}

impl Axis {
    fn main(&self, size: Size) -> f32 {
        match self {
            Axis::Horizontal => size.width,
            Axis::Vertical => size.height,
        }
    }

    fn cross(&self, size: Size) -> f32 {
        match self {
            Axis::Horizontal => size.height,
            Axis::Vertical => size.width,
        }
    }

    fn pack<T>(&self, main: T, cross: T) -> (T, T) {
        match self {
            Axis::Horizontal => (main, cross),
            Axis::Vertical => (cross, main),
        }
    }
}

/// [`resolve`], but reusing a cached [`Node`] from `cache` when a prior
/// call already computed the answer for equal `limits` and `fingerprint`.
///
/// This is the "fast path" containers like `Row`/`Column` should call
/// once they hold a persistent [`layout::Cache`](crate::layout::Cache) in
/// their widget state: unlike [`resolve`] itself (a pure function with no
/// memory of its own), a container's `Cache` survives across `layout`
/// calls in its `Tree`, so an unchanged container measured under
/// unchanged constraints skips the whole multi-pass solve.
///
/// `fingerprint` must fold in anything [`resolve_cached`] cannot see for
/// itself but that should still invalidate the cache -- most importantly
/// the identity/count of `items`, since a container whose children
/// changed cannot reuse a previous measurement no matter how the
/// [`Limits`] compare. A typical fingerprint combines the child count
/// with each child's own [`Widget::size`](crate::Widget::size) strategy
/// (cheap to read, and changes whenever a child's sizing intent does).
#[allow(clippy::too_many_arguments)]
pub fn resolve_cached<Message, Theme, Renderer>(
    cache: &mut super::Cache,
    fingerprint: super::cache::Fingerprint,
    axis: Axis,
    renderer: &Renderer,
    limits: &Limits,
    width: Length,
    height: Length,
    padding: Padding,
    spacing: f32,
    align_items: Alignment,
    items: &mut [Element<'_, Message, Theme, Renderer>],
    trees: &mut [widget::Tree],
) -> Node
where
    Renderer: crate::Renderer,
{
    // The multi-pass solver below has side effects on `trees` (each
    // child's own layout cache is populated as a side effect of being
    // measured), so a cache hit here must still be sound even though
    // `compute` will not run: children are only skipped when this
    // container's own result -- which already reflects their last
    // measurement -- is being reused unchanged, exactly mirroring
    // `widget::tree::Tree::layout_with_cache`'s same precondition.
    cache.get_or_insert_with(limits, fingerprint, || {
        resolve(
            axis,
            renderer,
            limits,
            width,
            height,
            padding,
            spacing,
            align_items,
            items,
            trees,
        )
    })
}

mod engine;

pub use engine::resolve;
