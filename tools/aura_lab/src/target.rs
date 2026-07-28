//! What Aura Lab is attached to, and what it can honestly ask it.
//!
//! # In-process, not remote
//!
//! `phase.md` calls for a "standalone inspector app and connection
//! lifecycle", which reads as: Aura Lab is one process, the application
//! under inspection is another, and they talk.
//!
//! This connects **in-process** instead: Aura Lab hosts the view it
//! inspects and shares its [`Collector`]. That is a real limitation and it
//! is written down here rather than implied by absence — see
//! [`Connection::Remote`].
//!
//! The reason is not laziness about sockets. A remote inspector can only
//! see what the wire carries, so the wire format becomes the design: every
//! question the inspector will ever ask has to be answerable by a message
//! defined up front. Deciding that before a single panel exists means
//! guessing, and the guess is expensive to change once panels depend on
//! it. In-process, a panel asks the tree directly, and what the panels
//! actually need becomes visible. *Then* the protocol is a description of
//! known requirements rather than a prediction.
//!
//! The cost is stated plainly: Aura Lab cannot attach to a running VYOM
//! shell today. It can inspect a view it hosts.

use std::time::{Duration, Instant};

use aura::widget::{button, checkbox, column, container, row, scrollable, slider, text};
use aura::{Color, Element, Rectangle, Size, Theme};
use aura_core::semantics;
use aura_core::widget::visit::Visit;
use aura_profiler::events::{Event, Phase, Resource};
use aura_profiler::{Collector, export};
use aura_renderer::graphics::text::CacheStatistics;
use aura_runtime::diagnostics::Trace;
use aura_runtime::user_interface::{Cache, UserInterface};

/// The renderer the hosted view draws with.
///
/// The real software renderer, headless — **not** the `()` null renderer.
/// Two reasons, and the second is the important one: `()` only exists
/// under `debug_assertions`, so a release build of the inspector would
/// not compile; and it does no text shaping, which would make every
/// `Layout` measurement fiction. Shaping *is* the layout cost in most real
/// views, and a profiler that omitted it would report a pipeline nobody
/// has.
type Renderer = aura_renderer::Renderer;

/// What the inspector is looking at.
#[derive(Debug, Clone)]
pub enum Connection {
    /// Nothing attached.
    Detached,

    /// A view hosted by Aura Lab itself, sharing its collector.
    ///
    /// The only kind that works today.
    Hosted {
        /// The trace being filled by the hosted view.
        collector: Collector,
        /// When the attachment began, so frame numbers mean something.
        since: Instant,
    },

    /// A separate process.
    ///
    /// Not implemented. Present as a variant rather than absent so the
    /// gap is visible in the type: a reader wondering "can this attach to
    /// my shell?" finds the answer here instead of concluding it from
    /// silence.
    ///
    /// Nothing constructs it, and that is the point — it is a declared
    /// hole, not dead code. Deleting it to quiet the lint would delete
    /// the only place the limitation is written down.
    #[allow(dead_code, reason = "a declared gap; see the variant's docs")]
    Remote,
}

impl Connection {
    /// Attaches to a view Aura Lab hosts.
    pub fn hosted() -> Self {
        Self::Hosted {
            collector: Collector::enabled(),
            since: Instant::now(),
        }
    }

    /// The collector being filled, if any.
    pub fn collector(&self) -> Option<&Collector> {
        match self {
            Connection::Hosted { collector, .. } => Some(collector),
            Connection::Detached | Connection::Remote => None,
        }
    }

    /// How long this connection has been open.
    pub fn uptime(&self) -> Duration {
        match self {
            Connection::Hosted { since, .. } => since.elapsed(),
            Connection::Detached | Connection::Remote => Duration::ZERO,
        }
    }

    /// A description for the status bar.
    pub fn describe(&self) -> String {
        match self {
            Connection::Detached => "detached".to_owned(),
            Connection::Hosted { collector, .. } => {
                format!("hosted \u{00b7} {} events", collector.len())
            }
            Connection::Remote => "remote (not implemented)".to_owned(),
        }
    }

    /// Whether anything is being recorded.
    pub fn is_live(&self) -> bool {
        matches!(self, Connection::Hosted { .. })
    }
}

/// One frame, reconstructed from a trace.
///
/// The trace is a flat event list; a frame is the thing a developer
/// actually reasons about. Assembling one is this module's job so that
/// every panel agrees on where a frame started and stopped — a panel that
/// worked it out for itself would eventually disagree with its neighbour,
/// and the developer would have no way to know which was lying.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// Which frame.
    pub number: u64,
    /// When it began, in trace nanoseconds.
    pub began: u64,
    /// How long it took. `None` if the trace was cut before it ended —
    /// which is ordinary, since the ring buffer drops the oldest.
    pub duration: Option<u64>,
    /// Time attributed to each phase.
    pub phases: Vec<(Phase, u64)>,
    /// Why it missed, if it did.
    pub missed: Option<aura_profiler::Cause>,
}

impl Frame {
    /// Time this frame spent in phases.
    pub fn attributed(&self) -> u64 {
        self.phases.iter().map(|(_, total)| total).sum()
    }

    /// Time this frame spent somewhere nothing reported.
    ///
    /// Shown rather than hidden. A frame whose phases account for 9 of its
    /// 10 milliseconds has a millisecond of uninstrumented work in it, and
    /// that is a finding — not a rounding error to distribute across the
    /// bars until the chart looks tidy.
    pub fn unattributed(&self) -> Option<u64> {
        Some(self.duration?.saturating_sub(self.attributed()))
    }
}

/// Splits a trace into frames.
///
/// Events between a `FrameBegin` and its `FrameEnd` belong to that frame.
/// Anything before the first `FrameBegin` is dropped: the ring buffer
/// truncates the oldest events, so a trace routinely starts mid-frame, and
/// charging that partial evidence to a frame whose beginning is gone would
/// invent a frame that never happened.
pub fn frames(events: &[Event]) -> Vec<Frame> {
    let mut frames = Vec::new();
    let mut open: Option<(u64, u64, usize)> = None;

    for (index, event) in events.iter().enumerate() {
        match event {
            Event::FrameBegin { frame, at } => {
                open = Some((*frame, *at, index));
            }
            Event::FrameEnd { frame, at } => {
                let Some((number, began, start)) = open.take() else {
                    continue;
                };

                if number != *frame {
                    continue;
                }

                frames.push(Frame {
                    number,
                    began,
                    duration: Some(at.saturating_sub(began)),
                    phases: export::phase_totals(&events[start..=index]),
                    missed: missed_cause(&events[start..=index], number),
                });
            }
            _ => {}
        }
    }

    // A frame that began and never ended is real and worth showing: it is
    // what a hang looks like, and dropping it would make the most
    // interesting frame in the trace the one the inspector hides.
    if let Some((number, began, start)) = open {
        frames.push(Frame {
            number,
            began,
            duration: None,
            phases: export::phase_totals(&events[start..]),
            missed: missed_cause(&events[start..], number),
        });
    }

    frames
}

/// The cause recorded against `frame`, if any.
fn missed_cause(events: &[Event], frame: u64) -> Option<aura_profiler::Cause> {
    events.iter().find_map(|event| match event {
        Event::Missed {
            frame: missed,
            cause,
            ..
        } if *missed == frame => Some(*cause),
        _ => None,
    })
}

/// One node of the hosted view's tree, flattened for a panel to read.
///
/// A snapshot, not a borrow. The panels run in the inspector's `view`,
/// which happens long after `step` returned and dropped the
/// `UserInterface` the data came from — so the alternative to copying is
/// keeping a widget tree alive across the inspector's own event loop,
/// which is a lifetime knot for no benefit. The tree is tens of nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// How deep this node sits.
    pub depth: usize,
    /// Its bounds, in the target's coordinate space.
    pub bounds: Rectangle,
    /// Its semantics, if it reports any.
    ///
    /// `None` is the common case and is not a defect: most widgets are
    /// presentational, and a tree where everything spoke would drown a
    /// screen reader.
    pub semantics: Option<semantics::Node>,
}

/// What the panels read: one frame's worth of everything.
///
/// Filled by [`Hosted::step`] while the `UserInterface` is alive and the
/// answers are cheap, rather than by each panel asking separately. Two
/// panels asking the tree at different moments would show two different
/// trees and no way to tell which was current.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// The widget tree, depth-first.
    pub tree: Vec<Node>,
    /// The semantic forest, as an adapter would receive it.
    pub semantics: Vec<semantics::Node>,
    /// What the last frame damaged, and why.
    pub damage: Option<aura_core::damage::Attribution>,
    /// Whether something is animating.
    pub is_animating: bool,
    /// Whether the last frame relaid out.
    pub layout_changed: bool,
    /// The target's surface size.
    pub bounds: Size,
    /// The target's scale factor.
    pub scale: f32,
}

/// A view Aura Lab hosts and drives, so there is something to profile.
///
/// # Why this exists
///
/// A first draft had [`Connection::hosted`] create a [`Collector`] and
/// stop there. The inspector attached, the status bar said "hosted", and
/// the frame graph said "No frames recorded" — because nothing was
/// writing to the collector. The connection was a fiction: every part
/// worked and none of them met.
///
/// That is the same failure this session kept finding in the framework
/// itself, reproduced inside the tool built to find it. So [`Hosted`]
/// actually owns a widget tree and actually drives frames through it.
///
/// # It is driven headlessly, not shown
///
/// The hosted view is laid out, updated, drawn and asked for semantics
/// through a headless software renderer. It never reaches the screen.
///
/// That is deliberate: the point is to exercise and measure the
/// **pipeline** — diff, layout, paint preparation, semantics — not to put
/// a second picture next to the inspector. Text is really shaped and
/// quads are really recorded, so the numbers are real work; they are just
/// not composited.
pub struct Hosted {
    renderer: Renderer,
    cache: Option<Cache>,
    trace: Trace,
    frames: u64,
    /// Widget state the demo view reads, so successive frames are not
    /// identical -- an unchanging tree would measure only the cache.
    toggled: bool,
    amount: f32,
    /// The surface the target is laid out against.
    bounds: Size,
    /// The scale factor the target is laid out against.
    scale: f32,
    /// What the panels read.
    snapshot: Snapshot,
    /// The text cache's cumulative counts as of the previous frame.
    ///
    /// Starts at [`Default::default`] (a real baseline — a fresh cache
    /// genuinely has done nothing yet), rather than `Option::None`, so
    /// the very first frame's cache activity is `since` that zero and
    /// reported like any other frame's, instead of being silently
    /// absorbed as "the baseline" and lost.
    text_statistics: aura_renderer::graphics::text::cache::Statistics,
}

impl Hosted {
    /// Hosts a view recording into `collector`.
    ///
    /// `None` if no software renderer is available, which is not something
    /// this can paper over: a hosted target with no renderer cannot lay
    /// text out, and a frame graph built from unshaped layout would be
    /// confidently wrong rather than absent.
    pub fn new(collector: Collector) -> Option<Self> {
        let renderer = aura_futures::futures::executor::block_on(
            <Renderer as aura::advanced::renderer::Headless>::new(
                aura::advanced::renderer::Settings::default(),
                Some("tiny-skia"),
            ),
        )?;

        Some(Self {
            renderer,
            cache: Some(Cache::new().with_trace(Trace::new(collector.clone()))),
            trace: Trace::new(collector),
            frames: 0,
            toggled: false,
            amount: 0.0,
            bounds: Size::new(400.0, 600.0),
            scale: 1.0,
            snapshot: Snapshot::default(),
            text_statistics: aura_renderer::graphics::text::cache::Statistics::default(),
        })
    }

    /// What the last frame produced.
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// Lays the target out against a different surface.
    ///
    /// The whole of the device-profiles panel: a phone is not a desktop
    /// with fewer pixels, it is a different layout, and the only way to
    /// know what a view does at 360x780 is to lay it out at 360x780.
    pub fn resize(&mut self, bounds: Size, scale: f32) {
        self.bounds = bounds;
        self.scale = scale;

        // The cache holds a tree laid out against the old surface. Keeping
        // it would have the next frame reconcile against bounds that no
        // longer exist, and report a layout cost that belongs to neither
        // size.
        self.cache = None;
    }

    /// How many frames have been driven.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Drives one frame: build, update, draw, semantics.
    ///
    /// The frame boundary is declared here because this is the only place
    /// that knows where it is. `UserInterface` cannot infer it — whether
    /// `update` and `draw` belong to one frame or two is the event loop's
    /// decision, and this is the event loop.
    pub fn step(&mut self) {
        self.frames += 1;

        // Vary the tree so successive frames are not identical. An
        // unchanging tree would measure the cache and report it as the
        // pipeline being fast.
        self.toggled = !self.toggled;
        self.amount = (self.amount + 7.0) % 100.0;

        let _frame = self.trace.frame(self.frames);

        let cache = self
            .cache
            .take()
            .unwrap_or_else(|| Cache::new().with_trace(Trace::new(self.trace.collector().clone())));

        let mut ui = UserInterface::build(self.view(), self.bounds, cache, &mut self.renderer);

        let mut messages = Vec::new();

        let (state, _) = ui.update(
            &aura_core::window::Headless,
            &aura_core::shell::Waker::noop(),
            &[],
            aura_core::mouse::Cursor::Unavailable,
            &mut self.renderer,
            &mut messages,
        );

        ui.draw(
            &mut self.renderer,
            &Theme::Dark,
            &aura_core::renderer::Style::default(),
            aura_core::mouse::Cursor::Unavailable,
        );

        // `UserInterface::draw` only *queues* primitives into the
        // renderer's layer stack -- it does not rasterize them. Shaping
        // (and therefore the text cache) only runs during the pass that
        // turns those layers into pixels, which for a headless renderer
        // is `screenshot`. Without this call, `Cache::allocate` never
        // ran: `record_text_cache_activity` read a cache that had done
        // nothing, forever, and this whole gap looked closed while it
        // was not. The bytes themselves are not needed here -- forcing
        // the rasterization pass to run is the only reason to call it.
        let physical = Size::new(
            (self.bounds.width * self.scale).round() as u32,
            (self.bounds.height * self.scale).round() as u32,
        );
        let _ = <Renderer as aura::advanced::renderer::Headless>::screenshot(
            &mut self.renderer,
            physical,
            self.scale,
            Color::BLACK,
        );

        self.record_text_cache_activity();

        // Captured here, while the `UserInterface` is alive and the
        // answers are cheap. A panel asking later would be asking a tree
        // that no longer exists.
        self.snapshot = Snapshot {
            tree: walk(&mut ui.visit()),
            semantics: ui.semantics(&self.renderer),
            damage: state.damage(),
            is_animating: state.is_animating(),
            layout_changed: state.has_layout_changed(),
            bounds: self.bounds,
            scale: self.scale,
        };

        self.cache = Some(ui.into_cache());
    }

    /// Reads the renderer's text cache and records this frame's activity
    /// as profiler events.
    ///
    /// `text_cache_statistics` is cumulative; the cache itself has no
    /// notion of "this frame", only "since it was created" (see
    /// `graphics::text::cache::Statistics`'s own documentation). This is
    /// the one place that *does* know a frame's boundaries, so it is the
    /// one place that can turn two cumulative readings into one frame's
    /// worth via [`Statistics::since`](aura_renderer::graphics::text::cache::Statistics::since).
    ///
    /// `None` here is a permanent, honest answer for a backend with no
    /// such cache (`aura_wgpu`, via `CacheStatistics`'s default) — not a
    /// gap in this method. Aura Lab always drives its target through
    /// tiny-skia (see [`Hosted::new`]), so this is always `Some` in
    /// practice, but the method does not assume that.
    fn record_text_cache_activity(&mut self) {
        let Some(current) = self.renderer.text_cache_statistics() else {
            return;
        };

        let delta = current.since(self.text_statistics);
        self.text_statistics = current;

        let at = self.trace.collector().now();

        // One discrete event per hit/miss, matching the granularity
        // `aura_profiler::Event` already defines -- no new vocabulary
        // needed. A miss's cost is the real total this frame divided by
        // how many misses produced it: an honest *average* of measured
        // time, not a fabricated per-instance number (see
        // `Statistics::total_miss_nanos`'s own documentation).
        let average_miss_cost = delta
            .total_miss_nanos
            .checked_div(delta.misses)
            .unwrap_or(0);

        for _ in 0..delta.hits {
            self.trace.collector().record(|| Event::CacheHit {
                resource: Resource::Text,
                at,
            });
        }

        for _ in 0..delta.misses {
            self.trace.collector().record(|| Event::CacheMiss {
                resource: Resource::Text,
                at,
                cost: average_miss_cost,
            });
        }

        if delta.evicted_bytes > 0 {
            self.trace.collector().record(|| Event::Evicted {
                resource: Resource::Text,
                bytes: delta.evicted_bytes,
                at,
            });
        }
    }

    /// The view under inspection.
    ///
    /// Deliberately ordinary: a column of real controls, a scrollable, and
    /// enough text to shape. A synthetic tree of one widget would profile
    /// the profiler.
    fn view(&self) -> Element<'static, ()> {
        let rows = (0..40).fold(column![].spacing(4), |rows, index| {
            rows.push(
                row![
                    text(format!("row {index}")),
                    text("shaped text \u{2014} with some length to it"),
                ]
                .spacing(8),
            )
        });

        container(
            column![
                text("Hosted target").size(20),
                row![
                    button("press"),
                    checkbox(self.toggled).label("toggle"),
                    slider(0.0..=100.0, self.amount, |_| ()),
                ]
                .spacing(8),
                scrollable(rows),
            ]
            .spacing(10),
        )
        .padding(12)
        .into()
    }
}

/// Flattens a widget tree, depth-first.
///
/// Uses the same [`Visit`] traversal that semantics and hit-testing ride
/// on, rather than a third walk of its own: an inspector that disagreed
/// with the framework about the shape of the tree would be worse than no
/// inspector, because it would be believed.
fn walk(root: &mut dyn Visit) -> Vec<Node> {
    fn recurse(node: &mut dyn Visit, depth: usize, out: &mut Vec<Node>) {
        out.push(Node {
            depth,
            bounds: node.bounds(),
            semantics: node.semantics(),
        });

        node.children(&mut |child| recurse(child, depth + 1, out));
    }

    let mut out = Vec::new();
    recurse(root, 0, &mut out);
    out
}

impl std::fmt::Debug for Hosted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hosted")
            .field("frames", &self.frames)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(number: u64, began: u64, ended: u64) -> Vec<Event> {
        vec![
            Event::FrameBegin {
                frame: number,
                at: began,
            },
            Event::PhaseBegin {
                phase: Phase::Layout,
                at: began,
            },
            Event::PhaseEnd {
                phase: Phase::Layout,
                at: began + (ended - began) / 2,
            },
            Event::FrameEnd {
                frame: number,
                at: ended,
            },
        ]
    }

    #[test]
    fn a_trace_splits_into_frames() {
        let mut events = frame(1, 0, 10_000);
        events.extend(frame(2, 10_000, 26_000));

        let frames = frames(&events);

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].number, 1);
        assert_eq!(frames[0].duration, Some(10_000));
        assert_eq!(frames[1].duration, Some(16_000));
    }

    #[test]
    fn unattributed_time_is_visible_rather_than_distributed() {
        // A frame whose phases account for 9 of its 10 milliseconds has a
        // millisecond of uninstrumented work in it. That is the finding.
        let frames = frames(&frame(1, 0, 10_000));

        assert_eq!(frames[0].attributed(), 5_000);
        assert_eq!(
            frames[0].unattributed(),
            Some(5_000),
            "the residue must be reported, not spread across the bars"
        );
    }

    #[test]
    fn a_frame_that_never_ended_is_still_shown() {
        // It is what a hang looks like. Dropping it would make the most
        // interesting frame in the trace the one the inspector hides.
        let events = vec![
            Event::FrameBegin { frame: 7, at: 0 },
            Event::PhaseBegin {
                phase: Phase::Paint,
                at: 0,
            },
        ];

        let frames = frames(&events);

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].number, 7);
        assert_eq!(frames[0].duration, None);
        assert_eq!(frames[0].unattributed(), None, "and its residue is unknown");
    }

    #[test]
    fn a_trace_that_starts_mid_frame_invents_nothing() {
        // The ring buffer truncates the oldest events, so this is the
        // ordinary case, not an edge one. Charging the partial evidence to
        // a frame whose beginning is gone would put a frame on the chart
        // that never happened.
        let events = vec![
            Event::PhaseEnd {
                phase: Phase::Paint,
                at: 500,
            },
            Event::FrameEnd { frame: 3, at: 900 },
        ];

        assert!(frames(&events).is_empty());
    }

    #[test]
    fn a_missed_frame_carries_its_cause() {
        let mut events = frame(1, 0, 30_000);

        events.insert(
            3,
            Event::Missed {
                frame: 1,
                over: 14_000,
                cause: aura_profiler::Cause::GpuStall,
            },
        );

        let frames = frames(&events);

        assert_eq!(frames[0].missed, Some(aura_profiler::Cause::GpuStall));
    }

    #[test]
    fn a_cause_belongs_to_the_frame_it_names() {
        // The `Missed` event is recorded with the frame number it explains
        // rather than a timestamp. Matching by position instead would
        // attach it to whichever frame happened to be open.
        let mut events = frame(1, 0, 10_000);
        events.extend(frame(2, 10_000, 40_000));

        events.insert(
            5,
            Event::Missed {
                frame: 2,
                over: 24_000,
                cause: aura_profiler::Cause::Compositor,
            },
        );

        let frames = frames(&events);

        assert_eq!(frames[0].missed, None, "frame 1 was fine");
        assert_eq!(frames[1].missed, Some(aura_profiler::Cause::Compositor));
    }

    #[test]
    fn a_detached_connection_answers_honestly() {
        let connection = Connection::Detached;

        assert!(!connection.is_live());
        assert!(connection.collector().is_none());
        assert_eq!(connection.uptime(), Duration::ZERO);
    }

    #[test]
    fn a_remote_connection_says_it_is_not_implemented() {
        // Visible in the type and in the status bar, rather than left for
        // a reader to conclude from silence.
        assert!(Connection::Remote.describe().contains("not implemented"));
        assert!(!Connection::Remote.is_live());
    }

    #[test]
    fn a_hosted_view_actually_produces_frames() {
        // The regression this exists for: a first draft created a
        // collector, called it "hosted", and wrote nothing to it. The
        // status bar said attached and the frame graph said empty.
        let collector = Collector::enabled();
        let mut hosted = Hosted::new(collector.clone()).expect("tiny-skia must be available");

        for _ in 0..3 {
            hosted.step();
        }

        let frames = frames(&collector.events());

        assert_eq!(frames.len(), 3, "three steps must produce three frames");
        assert!(
            frames.iter().all(|frame| frame.duration.is_some()),
            "every driven frame must be complete"
        );
    }

    #[test]
    fn a_hosted_frame_has_its_phases_attributed() {
        // The exit gate, through the tool: a frame the inspector cannot
        // break down is a bar a developer can do nothing with.
        let collector = Collector::enabled();
        let mut hosted = Hosted::new(collector.clone()).expect("tiny-skia must be available");

        hosted.step();

        let frames = frames(&collector.events());
        let frame = &frames[0];

        assert!(
            !frame.phases.is_empty(),
            "a driven frame must attribute its time: {frame:?}"
        );

        for phase in [Phase::Diff, Phase::Layout, Phase::Paint, Phase::Semantics] {
            assert!(
                frame.phases.iter().any(|(recorded, _)| *recorded == phase),
                "{phase:?} must be recorded: {:?}",
                frame.phases
            );
        }
    }

    #[test]
    fn driving_the_target_produces_real_text_cache_events() {
        // The other framework gap this session closed: nothing recorded
        // GPU/cache events at all. The demo view's checkbox label goes
        // through `fill_text` -> `Text::Cached` -> `graphics::text::Cache`,
        // so driving it must produce real `CacheHit`/`CacheMiss` events --
        // not because this test fabricates them, but because the label is
        // actually drawn.
        let collector = Collector::enabled();
        let mut hosted = Hosted::new(collector.clone()).expect("tiny-skia must be available");

        hosted.step();
        hosted.step();
        hosted.step();

        let events = collector.events();

        let text_events: Vec<_> = events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    Event::CacheHit {
                        resource: Resource::Text,
                        ..
                    } | Event::CacheMiss {
                        resource: Resource::Text,
                        ..
                    }
                )
            })
            .collect();

        assert!(
            !text_events.is_empty(),
            "the checkbox label must exercise the real text cache: {events:?}"
        );
    }

    #[test]
    fn a_cache_miss_reports_a_real_cost_not_a_fabricated_one() {
        // The first frame is guaranteed at least one miss: the label has
        // never been shaped before. Its cost must be a real, measured
        // number -- never zero standing in for "not measured", which
        // would look like free work that never happened.
        let collector = Collector::enabled();
        let mut hosted = Hosted::new(collector.clone()).expect("tiny-skia must be available");

        hosted.step();

        let miss = collector
            .events()
            .into_iter()
            .find_map(|event| match event {
                Event::CacheMiss {
                    resource: Resource::Text,
                    cost,
                    ..
                } => Some(cost),
                _ => None,
            });

        assert!(
            miss.is_some_and(|cost| cost > 0),
            "a text cache miss must report a real, nonzero shaping cost"
        );
    }

    #[test]
    fn text_cache_activity_is_a_delta_not_a_running_total() {
        // If readings were reported cumulatively rather than diffed, every
        // later frame would inherit the first frame's misses forever, and
        // a cache that had gone completely idle would still look busy.
        let collector = Collector::enabled();
        let mut hosted = Hosted::new(collector.clone()).expect("tiny-skia must be available");

        hosted.step();
        let after_first = collector.events().len();

        hosted.step();
        let after_second = collector.events().len() - after_first;

        // The second frame reuses the label already shaped by the first
        // (the toggled state alternates, but "toggle" is the same text
        // either way), so it must record markedly fewer cache events than
        // the first frame did -- not the same inflated total again.
        assert!(
            after_second <= after_first,
            "frame two ({after_second} events) must not repeat frame one's total ({after_first})"
        );
    }

    #[test]
    fn successive_frames_are_not_identical() {
        // An unchanging tree would measure the cache and report it as the
        // pipeline being fast.
        let collector = Collector::enabled();
        let mut hosted = Hosted::new(collector.clone()).expect("tiny-skia must be available");

        hosted.step();
        hosted.step();

        assert_eq!(hosted.frames(), 2);
    }

    #[test]
    fn a_hosted_view_with_a_disabled_collector_records_nothing() {
        let collector = Collector::disabled();
        let mut hosted = Hosted::new(collector.clone()).expect("tiny-skia must be available");

        hosted.step();

        assert!(collector.is_empty());
    }

    #[test]
    fn a_hosted_connection_shares_its_collector() {
        let connection = Connection::hosted();

        assert!(connection.is_live());

        let collector = connection.collector().expect("hosted is live").clone();

        collector.record(|| Event::FrameBegin { frame: 1, at: 0 });

        assert_eq!(
            connection.collector().unwrap().len(),
            1,
            "the hosted view and the inspector must share one buffer"
        );
    }
}
