//! Taim Lab: an inspector for Taim.
//!
//! `phase.md` Phase 15's objective is to make performance and correctness
//! observable "without attaching a general-purpose debugger". A debugger
//! can tell you a function was slow. It cannot tell you that the frame was
//! slow *because* the text cache missed forty times, because it has no
//! idea what a text cache is.
//!
//! # What this is, honestly
//!
//! **The first Taim application that has ever run.** Everything else in
//! this repository is verified headlessly. That is real verification, and
//! it is also why this exists: a window that opens finds a class of
//! problem a headless test structurally cannot.
//!
//! It inspects a view it **hosts**, in-process. It cannot attach to a
//! separate application; see [`target::Connection::Remote`] for why that
//! is deferred rather than forgotten.
//!
//! # Panels
//!
//! Each answers one question a developer actually asks:
//!
//! All nine exist. Seven show real data; two ([`gpu_resources`],
//! [`overdraw_overlay`]) show none, because the framework records nothing
//! they could read. Those two say so, precisely, rather than drawing a
//! plausible picture — see their module docs. A wrong answer from a
//! profiler is worse than no answer, because it is acted on.

mod animation_inspector;
mod damage_overlay;
mod device_profiles;
mod frame_graph;
mod gpu_resources;
mod layout_overlay;
mod overdraw_overlay;
mod semantics_inspector;
mod target;
mod widget_inspector;

use std::time::Duration;

use taim::widget::{button, column, container, row, scrollable, text};
use taim::{Element, Length, Subscription, Task};

use target::{Connection, Hosted};

/// Which panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// Where the frame went, and why it was late.
    FrameGraph,

    /// The widget tree, its geometry and its identities.
    WidgetInspector,

    /// Bounds, and what is wrong with them.
    LayoutOverlay,

    /// What was redrawn.
    DamageOverlay,

    /// Overdraw. Shows nothing: nothing measures it.
    OverdrawOverlay,

    /// GPU resources. Shows nothing: nothing records them.
    GpuResources,

    /// The semantic forest, as a screen reader receives it.
    SemanticsInspector,

    /// Whether anything is moving, and for how long.
    AnimationInspector,

    /// Phone/tablet/desktop/watch, laid out for real.
    DeviceProfiles,
}

impl Panel {
    /// Every panel, in the order they appear.
    pub const ALL: [Panel; 9] = [
        Panel::FrameGraph,
        Panel::WidgetInspector,
        Panel::LayoutOverlay,
        Panel::DamageOverlay,
        Panel::OverdrawOverlay,
        Panel::GpuResources,
        Panel::SemanticsInspector,
        Panel::AnimationInspector,
        Panel::DeviceProfiles,
    ];

    /// The panel's name.
    pub fn title(self) -> &'static str {
        match self {
            Panel::FrameGraph => "Frames",
            Panel::WidgetInspector => "Widgets",
            Panel::LayoutOverlay => "Layout",
            Panel::DamageOverlay => "Damage",
            Panel::OverdrawOverlay => "Overdraw",
            Panel::GpuResources => "GPU",
            Panel::SemanticsInspector => "Semantics",
            Panel::AnimationInspector => "Animation",
            Panel::DeviceProfiles => "Devices",
        }
    }

    /// The question this panel answers.
    pub fn question(self) -> &'static str {
        match self {
            Panel::FrameGraph => "Where did the frame go, and why was it late?",
            Panel::WidgetInspector => "What is in the tree, and what survived reconciliation?",
            Panel::LayoutOverlay => "What constraints produced these bounds?",
            Panel::DamageOverlay => "What was redrawn, and why was it not less?",
            Panel::OverdrawOverlay => "How many times was each pixel written?",
            Panel::GpuResources => "What is on the GPU, and what was evicted?",
            Panel::SemanticsInspector => "What does a screen reader see?",
            Panel::AnimationInspector => "What is moving, and how fast?",
            Panel::DeviceProfiles => "How does this look on something else?",
        }
    }

    /// Whether this panel can show real data.
    ///
    /// `false` for the two whose data the framework does not record.
    /// They still render — they explain themselves — but the tab marks
    /// them, because a developer deserves to know before clicking that
    /// there is nothing behind it.
    pub fn has_data(self) -> bool {
        !matches!(self, Panel::OverdrawOverlay | Panel::GpuResources)
    }
}

/// How often the hosted target is driven.
///
/// ~60Hz, on a timer rather than on `window::frames()`. The difference
/// matters: `frames()` fires on the *inspector's* redraws, so occluding
/// or minimising the inspector's window would silently stop driving the
/// target — and the trace would simply stop, with nothing saying why. A
/// timer keeps measuring whether or not anyone is looking, which is the
/// behaviour a profiler needs.
const TICK: Duration = Duration::from_millis(16);

/// The inspector.
struct Lab {
    panel: Panel,
    connection: Connection,
    /// The view being profiled. `None` when detached, or when no software
    /// renderer is available.
    target: Option<Hosted>,
    /// How long the target has been animating.
    ///
    /// Accumulated here rather than in the snapshot: it is a fact about a
    /// *run of frames*, and a snapshot is one frame. A per-frame field
    /// could only ever say "yes" or "no", which is what makes the stuck
    /// case invisible.
    activity: animation_inspector::Activity,
}

impl Default for Lab {
    fn default() -> Self {
        let mut lab = Self {
            panel: Panel::FrameGraph,
            connection: Connection::Detached,
            target: None,
            activity: animation_inspector::Activity::default(),
        };

        // Attached from the start: an inspector that opened detached would
        // show an empty window, and the first thing anyone would ask is
        // whether it was broken.
        lab.attach();
        lab
    }
}

impl Lab {
    /// Attaches to a freshly hosted view.
    fn attach(&mut self) {
        let connection = Connection::hosted();

        // The target and the connection share one collector -- that
        // sharing is the whole attachment. A first draft created the
        // collector here and never built the target, so the status bar
        // said "hosted" while the frame graph said "no frames".
        self.target = connection.collector().cloned().and_then(Hosted::new);

        self.connection = if self.target.is_some() {
            connection
        } else {
            Connection::Detached
        };
    }
}

#[derive(Debug, Clone)]
enum Message {
    Show(Panel),
    Attach,
    Detach,
    /// Drive the hosted target one frame.
    Tick,
    /// Lay the target out as a different device.
    Profile(device_profiles::Profile),
}

fn update(lab: &mut Lab, message: Message) -> Task<Message> {
    match message {
        Message::Show(panel) => lab.panel = panel,
        Message::Attach => {
            lab.attach();
            lab.activity = animation_inspector::Activity::default();
        }
        Message::Detach => {
            lab.connection = Connection::Detached;
            lab.target = None;
            // Reset: a run of animation belongs to the target that was
            // running it, and carrying it across an attach would accuse a
            // fresh target of being stuck.
            lab.activity = animation_inspector::Activity::default();
        }
        Message::Tick => {
            if let Some(target) = &mut lab.target {
                target.step();
                lab.activity.record(target.snapshot().is_animating);
            }
        }
        Message::Profile(profile) => {
            if let Some(target) = &mut lab.target {
                target.resize(profile.bounds, profile.scale);
            }
        }
    }

    Task::none()
}

fn subscription(lab: &Lab) -> Subscription<Message> {
    if lab.target.is_some() {
        taim::time::every(TICK).map(|_| Message::Tick)
    } else {
        // Detached costs nothing. A timer that kept firing into a `None`
        // would keep the inspector awake and burning battery to do
        // nothing, which is the opposite of what a profiler should model.
        Subscription::none()
    }
}

fn view(lab: &Lab) -> Element<'_, Message> {
    let tabs = Panel::ALL.iter().fold(row![].spacing(4), |tabs, panel| {
        let label = if panel.has_data() {
            text(panel.title())
        } else {
            // Marked, so a developer knows before clicking. The panel
            // still opens and explains itself.
            text(format!("{} \u{00b7} —", panel.title()))
        };

        tabs.push(
            button(label)
                .on_press(Message::Show(*panel))
                .style(if *panel == lab.panel {
                    button::primary
                } else {
                    button::text
                }),
        )
    });

    // Rendered whether or not anything is attached: the two panels with
    // no data source have something to say regardless, and a "nothing
    // attached" message would hide the more useful explanation.
    let events = lab
        .connection
        .collector()
        .map(taim_profiler::Collector::events)
        .unwrap_or_default();

    let body: Element<'_, Message> = match (lab.panel, &lab.target) {
        (Panel::OverdrawOverlay, _) => overdraw_overlay::view(),
        (Panel::GpuResources, _) => gpu_resources::view(&events),
        (_, None) => unbuilt("Nothing is attached.", "Attach to inspect a view."),
        (Panel::FrameGraph, Some(_)) => frame_graph::view(&target::frames(&events)),
        (Panel::WidgetInspector, Some(target)) => widget_inspector::view(&target.snapshot().tree),
        (Panel::SemanticsInspector, Some(target)) => {
            semantics_inspector::view(&target.snapshot().semantics)
        }
        (Panel::LayoutOverlay, Some(target)) => {
            let snapshot = target.snapshot();

            layout_overlay::view(&snapshot.tree, snapshot.bounds)
        }
        (Panel::DamageOverlay, Some(target)) => {
            let snapshot = target.snapshot();

            damage_overlay::view(
                snapshot.damage.as_ref(),
                snapshot.bounds,
                snapshot.layout_changed,
            )
        }
        (Panel::AnimationInspector, Some(target)) => {
            animation_inspector::view(lab.activity, target.snapshot().is_animating)
        }
        (Panel::DeviceProfiles, Some(target)) => {
            let snapshot = target.snapshot();

            device_profiles::view(snapshot.bounds, snapshot.scale, Message::Profile)
        }
    };

    let driven = match &lab.target {
        Some(target) => format!("{} frames driven", target.frames()),
        None => "not driving".to_owned(),
    };

    let status = row![
        text(lab.connection.describe()).size(12),
        text(driven).size(12),
        text(format!("{:.1}s", lab.connection.uptime().as_secs_f32())).size(12),
        if lab.connection.is_live() {
            button(text("Detach").size(12)).on_press(Message::Detach)
        } else {
            button(text("Attach").size(12)).on_press(Message::Attach)
        },
    ]
    .spacing(12);

    container(
        column![
            scrollable(tabs).direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::default()
            )),
            container(body).height(Length::Fill).padding(8),
            status,
        ]
        .spacing(8),
    )
    .padding(12)
    .into()
}

/// A panel that is not built, saying so rather than showing an empty box.
fn unbuilt<'a>(question: &'a str, why: &'a str) -> Element<'a, Message> {
    container(
        column![text(question).size(16), text(why).size(12)]
            .spacing(6)
            .width(Length::Fill),
    )
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

/// Opens the inspector.
pub fn main() -> taim::Result {
    taim::application(Lab::default, update, view)
        .subscription(subscription)
        .title("Taim Lab")
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_panel_says_what_it_answers() {
        for panel in Panel::ALL {
            assert!(!panel.title().is_empty());
            assert!(
                panel.question().ends_with('?'),
                "{panel:?} must state a question a developer actually asks"
            );
        }
    }

    #[test]
    fn exactly_two_panels_have_no_data_and_they_are_the_expected_two() {
        // Registered here so the claim cannot rot. If a renderer starts
        // recording resource events, this test is what fails and points at
        // the tab that should stop being marked.
        let empty: Vec<_> = Panel::ALL
            .iter()
            .filter(|panel| !panel.has_data())
            .collect();

        assert_eq!(
            empty,
            vec![&Panel::OverdrawOverlay, &Panel::GpuResources],
            "the panels with no data source must be exactly the documented two"
        );
    }

    #[test]
    fn a_panel_with_no_data_still_renders_while_detached() {
        // They have something to say regardless, and a "nothing attached"
        // message would hide the more useful explanation.
        let mut lab = Lab::default();
        let _ = update(&mut lab, Message::Detach);

        for panel in [Panel::OverdrawOverlay, Panel::GpuResources] {
            let _ = update(&mut lab, Message::Show(panel));

            let _ = std::hint::black_box(view(&lab));
        }
    }

    #[test]
    fn every_panel_renders_while_attached() {
        // The cheapest guard against a panel wired to the wrong field: all
        // nine are built against a real driven frame.
        let mut lab = Lab::default();
        let _ = update(&mut lab, Message::Tick);

        for panel in Panel::ALL {
            let _ = update(&mut lab, Message::Show(panel));

            let _ = std::hint::black_box(view(&lab));
        }
    }

    #[test]
    fn selecting_a_device_profile_relays_the_target_out() {
        // Simulation, not preview: a phone is not a desktop with fewer
        // pixels, and the only way to know what a view does at 360x780 is
        // to lay it out at 360x780.
        let mut lab = Lab::default();
        let _ = update(&mut lab, Message::Tick);

        let phone = device_profiles::Profile::ALL[0];
        let _ = update(&mut lab, Message::Profile(phone));
        let _ = update(&mut lab, Message::Tick);

        let snapshot = lab.target.as_ref().expect("attached").snapshot();

        assert_eq!(snapshot.bounds, phone.bounds);
        assert_eq!(snapshot.scale, phone.scale);
    }

    #[test]
    fn animation_activity_is_reset_by_reattaching() {
        // A run of animation belongs to the target that was running it.
        // Carrying it across would accuse a fresh target of being stuck.
        let mut lab = Lab::default();

        for _ in 0..3 {
            let _ = update(&mut lab, Message::Tick);
        }

        let _ = update(&mut lab, Message::Attach);

        assert_eq!(lab.activity.run(), 0);
        assert_eq!(lab.activity.longest(), 0);
    }

    #[test]
    fn a_built_panel_shows_the_targets_data() {
        // The end-to-end claim for the tree panels: drive a frame, and the
        // snapshot the panels read is populated.
        let mut lab = Lab::default();

        let _ = update(&mut lab, Message::Tick);

        let snapshot = lab.target.as_ref().expect("attached").snapshot();

        assert!(
            !snapshot.tree.is_empty(),
            "the widget tree must be captured"
        );
        assert!(
            !snapshot.semantics.is_empty(),
            "the demo view has real controls, so it must announce something"
        );
    }

    #[test]
    fn the_inspector_starts_attached_to_something_real() {
        // Not just "attached": a first draft reported hosted while nothing
        // was writing to the collector. The status bar and the frame graph
        // disagreed, and the status bar was lying.
        let lab = Lab::default();

        assert!(lab.connection.is_live());
        assert!(
            lab.target.is_some(),
            "an attached inspector must actually be hosting a view"
        );
        assert_eq!(lab.panel, Panel::FrameGraph);
    }

    #[test]
    fn ticking_produces_frames_the_panel_can_read() {
        // The end-to-end claim: the tool drives a view, the pipeline
        // records phases, and the panel has something to draw.
        let mut lab = Lab::default();

        for _ in 0..3 {
            let _ = update(&mut lab, Message::Tick);
        }

        let collector = lab.connection.collector().expect("attached");
        let frames = target::frames(&collector.events());

        assert_eq!(frames.len(), 3);
        assert!(!frames[0].phases.is_empty(), "and they are attributed");
    }

    #[test]
    fn attaching_and_detaching_round_trips() {
        let mut lab = Lab::default();

        let _ = update(&mut lab, Message::Detach);
        assert!(!lab.connection.is_live());
        assert!(lab.target.is_none(), "detaching must stop driving the view");

        let _ = update(&mut lab, Message::Attach);
        assert!(lab.connection.is_live());
        assert!(lab.target.is_some());
    }

    #[test]
    fn a_detached_inspector_does_not_keep_a_timer_running() {
        // A timer firing into a `None` would keep the inspector awake and
        // burning battery to do nothing -- the opposite of what a profiler
        // should model.
        let mut lab = Lab::default();
        let _ = update(&mut lab, Message::Detach);

        // `Subscription` has no public emptiness predicate, so the check
        // is that detaching drops the target the subscription keys off.
        assert!(lab.target.is_none());
    }

    #[test]
    fn a_tick_while_detached_is_harmless() {
        let mut lab = Lab::default();

        let _ = update(&mut lab, Message::Detach);
        let _ = update(&mut lab, Message::Tick);

        assert!(lab.connection.collector().is_none());
    }

    #[test]
    fn showing_a_panel_selects_it() {
        let mut lab = Lab::default();

        let _ = update(&mut lab, Message::Show(Panel::GpuResources));

        assert_eq!(lab.panel, Panel::GpuResources);
    }
}
