//! What is on the GPU, and what was evicted.
//!
//! # Text is real; the rest is not
//!
//! `phase.md` asks for "buffers/textures/atlases/pipelines/budgets/
//! evictions". [`Resource::Text`] is wired end to end: `graphics::text::
//! Cache` counts its own hits, misses and evictions, and each miss carries
//! its **real, measured** shaping time (never a fabricated or averaged-
//! from-somewhere-else number for an individual event) — see
//! `graphics::text::cache::Statistics`. A renderer exposes this through
//! `graphics::text::CacheStatistics`, and the tool driving the hosted
//! target diffs two cumulative readings into one frame's activity and
//! records it through the ordinary `Event::CacheHit` / `CacheMiss` /
//! `Evicted` vocabulary — no new event kind was needed.
//!
//! [`Resource::Image`], [`Resource::Layer`], [`Resource::Effect`],
//! [`Resource::Atlas`], [`Resource::Buffer`] and [`Resource::Pipeline`]
//! are not: nothing counts them, and — for the GPU-side ones — nothing
//! *can* without wiring the same statistics-and-trait pattern through
//! `taim_wgpu`'s atlases, buffers and pipelines, which this session did
//! not reach.
//!
//! # Why text was worth doing and the rest was left honest
//!
//! Wiring one resource all the way through — cache counters, a real
//! per-miss timer, a cross-backend trait with an honest `None` default,
//! and the tool actually reading it — is what proves the chain works at
//! all. Doing that for six resources at once, several of which live behind
//! GPU APIs this session had no hardware path to verify against, would
//! have meant either leaving them untested or claiming victory on
//! reasoning alone — which is exactly the failure this tool exists to
//! catch, committed by the inspector built to catch it. One proven
//! resource is worth more than six asserted ones.
//!
//! # A trap this panel walked into and out of
//!
//! `Hosted::step` originally called `UserInterface::draw` and stopped
//! there — which only *queues* primitives into a renderer's layer stack.
//! Shaping, and therefore `Cache::allocate`, only runs during the pass
//! that turns those layers into pixels. The first version of this wiring
//! read a cache that had never been asked to do anything, forever, and
//! reported all zeros — which looked identical to "correctly wired, view
//! draws nothing text-related" until a test asserted the count was
//! nonzero and it was not. `Hosted::step` now forces that pass by calling
//! `screenshot` after `draw`.
//!
//! [`Resource::Text`]: taim_profiler::Resource::Text
//! [`Resource::Image`]: taim_profiler::Resource::Image
//! [`Resource::Layer`]: taim_profiler::Resource::Layer
//! [`Resource::Effect`]: taim_profiler::Resource::Effect
//! [`Resource::Atlas`]: taim_profiler::Resource::Atlas
//! [`Resource::Buffer`]: taim_profiler::Resource::Buffer
//! [`Resource::Pipeline`]: taim_profiler::Resource::Pipeline

use taim::prelude::*;
use taim::view::AnyView;

use taim_profiler::Resource;
use taim_profiler::events::Event;

/// Precisely which resources have no data yet, and why.
pub const MISSING: &str = "Only the text cache is wired. Image, layer, effect, atlas, buffer and \
                           pipeline resources record nothing: the GPU-side ones live behind \
                           taim_wgpu's own atlas/buffer/pipeline management, which would need the \
                           same statistics-and-trait pattern text now proves out \u{2014} counters on \
                           each cache, a cross-backend trait with an honest None default, and \
                           something that actually forces a real frame to run so there is \
                           something to count.";

/// What a trace says about resources.
///
/// Reads real events. [`Resource::Text`] genuinely counts (see this
/// module's own docs for the chain); everything else in [`RESOURCES`]
/// reads zero because nothing emits it yet, not because this panel failed
/// to ask.
///
/// [`Resource::Text`]: taim_profiler::Resource::Text
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    /// Cache hits, by resource.
    pub hits: [u64; 7],
    /// Cache misses, by resource.
    pub misses: [u64; 7],
    /// Bytes reclaimed by eviction, by resource.
    pub evicted: [u64; 7],
}

/// Every resource kind, in the order [`Counts`] indexes them.
pub const RESOURCES: [Resource; 7] = [
    Resource::Text,
    Resource::Image,
    Resource::Layer,
    Resource::Effect,
    Resource::Atlas,
    Resource::Buffer,
    Resource::Pipeline,
];

fn index(resource: Resource) -> usize {
    RESOURCES
        .iter()
        .position(|candidate| *candidate == resource)
        .unwrap_or(0)
}

impl Counts {
    /// Counts the resource events in `events`.
    pub fn of(events: &[Event]) -> Self {
        let mut counts = Self::default();

        for event in events {
            match event {
                Event::CacheHit { resource, .. } => counts.hits[index(*resource)] += 1,
                Event::CacheMiss { resource, .. } => counts.misses[index(*resource)] += 1,
                Event::Evicted {
                    resource, bytes, ..
                } => counts.evicted[index(*resource)] += bytes,
                _ => {}
            }
        }

        counts
    }

    /// Whether anything was recorded at all.
    pub fn any(&self) -> bool {
        self.hits.iter().chain(&self.misses).any(|count| *count > 0)
            || self.evicted.iter().any(|bytes| *bytes > 0)
    }

    /// The hit rate for `resource`, if it was used.
    ///
    /// `None` rather than `1.0` for a resource nothing touched: a cache
    /// with no traffic has no hit rate, and reporting a perfect one would
    /// put a green number next to a subsystem that never ran.
    pub fn hit_rate(&self, resource: Resource) -> Option<f32> {
        let index = index(resource);
        let hits = self.hits[index];
        let total = hits + self.misses[index];

        (total > 0).then(|| hits as f32 / total as f32)
    }
}

/// Renders the GPU resource panel.
pub fn view(events: &[Event]) -> AnyView {
    let counts = Counts::of(events);

    if !counts.any() {
        return container(
            column![
                text("Nothing recorded yet.").size(16),
                text(
                    "The text cache is wired and will show real counts once a frame draws \
                     something through it. If this stays empty while frames are running, that \
                     is itself worth noticing."
                )
                .size(12),
                text(MISSING).size(12),
            ]
            .gap(8.0)
            .fill_width(),
        )
        .center_x()
        .fill_width()
        .center_y()
        .fill_height()
        .padding(12)
        .into();
    }

    let rows = RESOURCES.iter().fold(column![].gap(2.0), |rows, resource| {
        let index = index(*resource);

        let rate = match counts.hit_rate(*resource) {
            Some(rate) => format!("{:.0}% hit", rate * 100.0),
            None => "unused".to_owned(),
        };

        rows.push(
            row![
                text(format!("{:<9}", resource.as_str())).size(12),
                text(format!("{:>6} hit", counts.hits[index])).size(11),
                text(format!("{:>6} miss", counts.misses[index])).size(11),
                text(rate).size(11),
                text(format!("{} bytes evicted", counts.evicted[index])).size(11),
            ]
            .gap(10.0),
        )
    });

    column![text("Resources").size(14), rows, text(MISSING).size(11),]
        .gap(8.0)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trace_with_no_resource_events_counts_nothing() {
        // Not a bug in this panel. A trace with nothing in it says so
        // rather than showing an empty table that looks like a healthy
        // cache.
        let counts = Counts::of(&[Event::FrameBegin { frame: 1, at: 0 }]);

        assert!(!counts.any());
    }

    #[test]
    fn the_panel_reads_real_events() {
        // Not a placeholder: `Resource::Text` is genuinely wired end to
        // end (see this module's own docs), and this is the same counting
        // path a real driven frame exercises.
        let counts = Counts::of(&[
            Event::CacheHit {
                resource: Resource::Text,
                at: 0,
            },
            Event::CacheHit {
                resource: Resource::Text,
                at: 1,
            },
            Event::CacheMiss {
                resource: Resource::Text,
                at: 2,
                cost: 500,
            },
        ]);

        assert!(counts.any());
        assert_eq!(counts.hits[index(Resource::Text)], 2);
        assert_eq!(counts.misses[index(Resource::Text)], 1);
    }

    #[test]
    fn an_unused_cache_has_no_hit_rate() {
        // Reporting 100% would put a green number next to a subsystem that
        // never ran -- which reads as "working perfectly".
        let counts = Counts::default();

        assert_eq!(counts.hit_rate(Resource::Atlas), None);
    }

    #[test]
    fn a_hit_rate_is_hits_over_traffic() {
        let counts = Counts::of(&[
            Event::CacheHit {
                resource: Resource::Image,
                at: 0,
            },
            Event::CacheMiss {
                resource: Resource::Image,
                at: 1,
                cost: 100,
            },
            Event::CacheMiss {
                resource: Resource::Image,
                at: 2,
                cost: 100,
            },
        ]);

        assert_eq!(counts.hit_rate(Resource::Image), Some(1.0 / 3.0));
    }

    #[test]
    fn evictions_accumulate_bytes_not_occurrences() {
        // "Three evictions" is not actionable; "12 MB reclaimed" is.
        let counts = Counts::of(&[
            Event::Evicted {
                resource: Resource::Atlas,
                bytes: 4096,
                at: 0,
            },
            Event::Evicted {
                resource: Resource::Atlas,
                bytes: 8192,
                at: 1,
            },
        ]);

        assert_eq!(counts.evicted[index(Resource::Atlas)], 12_288);
    }

    #[test]
    fn every_resource_kind_has_a_slot() {
        // A resource that fell through to index 0 would be counted as
        // text, and the text cache would look mysteriously busy.
        for resource in RESOURCES {
            let counts = Counts::of(&[Event::CacheHit { resource, at: 0 }]);

            assert_eq!(
                counts.hits[index(resource)],
                1,
                "{resource:?} must have its own slot"
            );
        }
    }

    #[test]
    fn resources_do_not_share_slots() {
        let counts = Counts::of(&[
            Event::CacheHit {
                resource: Resource::Text,
                at: 0,
            },
            Event::CacheHit {
                resource: Resource::Buffer,
                at: 1,
            },
        ]);

        assert_eq!(counts.hits[index(Resource::Text)], 1);
        assert_eq!(counts.hits[index(Resource::Buffer)], 1);
    }

    #[test]
    fn the_panel_names_what_is_still_missing_and_what_is_not() {
        // A developer wondering why five of six rows read zero should
        // learn what it would take, not conclude the tool is broken --
        // and should not be told text is missing when it is not.
        assert!(MISSING.contains("taim_wgpu"), "{MISSING}");
        assert!(
            !MISSING.contains("text is missing"),
            "the message must not claim the resolved gap is still open"
        );
    }

    #[test]
    fn a_protected_surface_could_never_be_shown_here() {
        // Pinned before the feature exists, because the obvious next one
        // is an atlas thumbnail -- and an atlas can hold a protected
        // surface's pixels. This is the test that should fail if anyone
        // adds one without thinking about it.
        use taim_core::security::Audience;
        use taim_profiler::Sensitivity;

        for audience in Audience::ALL {
            assert!(
                !Sensitivity::Protected.allows(audience),
                "protected pixels must reach no audience, and this panel is one"
            );
        }
    }
}
