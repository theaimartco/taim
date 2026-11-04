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
//! A native TAIM view application. Its panels inspect a hosted legacy
//! widget fixture, so the fixture can continue exercising the retained
//! widget/runtime contracts independently of the application facade.
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

// Local construction shorthands for native view lists.
macro_rules! column {
    ($($child:expr),* $(,)?) => { taim::view::views::column(taim::view::views![$($child),*]) };
}
macro_rules! row {
    ($($child:expr),* $(,)?) => { taim::view::views::row(taim::view::views![$($child),*]) };
}

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

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use taim::prelude::*;
use taim::view::{AnyView, Sender};

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
    timer: Option<Ticker>,
}

impl Default for Lab {
    fn default() -> Self {
        let mut lab = Self {
            panel: Panel::FrameGraph,
            connection: Connection::Detached,
            target: None,
            activity: animation_inspector::Activity::default(),
            timer: None,
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

fn update(lab: &mut Lab, message: Message) {
    match message {
        Message::Show(panel) => lab.panel = panel,
        Message::Attach => {
            lab.attach();
            lab.activity = animation_inspector::Activity::default();
        }
        Message::Detach => {
            lab.connection = Connection::Detached;
            lab.target = None;
            lab.timer = None;
            // Reset: a run of animation belongs to the target that was
            // running it, and carrying it across an attach would accuse a
            // fresh target of being stuck.
            lab.activity = animation_inspector::Activity::default();
        }
        Message::Tick => {
            if let Some(timer) = &lab.timer {
                timer.pending.store(false, Ordering::Release);
            }
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
}

#[cfg(test)]
fn view(lab: &Lab) -> AnyView {
    view_with(lab, Rc::new(|_| {}))
}

fn panel_tabs(selected: Panel, dispatch: Rc<dyn Fn(Message)>) -> AnyView {
    let tabs = Panel::ALL.iter().fold(row![].gap(4.0), |tabs, panel| {
        let label = if panel.has_data() {
            panel.title().to_owned()
        } else {
            // Marked, so a developer knows before clicking. The panel
            // still opens and explains itself.
            format!("{} \u{00b7} —", panel.title())
        };

        let dispatch = dispatch.clone();
        let panel = *panel;
        let control = button(label).on_press(move || dispatch(Message::Show(panel)));
        tabs.push(if panel == selected {
            control
        } else {
            control.ghost()
        })
    });

    tabs.into()
}

fn panel_body(lab: &Lab, dispatch: Rc<dyn Fn(Message)>) -> AnyView {
    // Rendered whether or not anything is attached: the two panels with
    // no data source have something to say regardless, and a "nothing
    // attached" message would hide the more useful explanation.
    let events = lab
        .connection
        .collector()
        .map(taim_profiler::Collector::events)
        .unwrap_or_default();

    let body: AnyView = match (lab.panel, &lab.target) {
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

            device_profiles::view(snapshot.bounds, snapshot.scale, {
                let dispatch = dispatch.clone();
                move |profile| dispatch(Message::Profile(profile))
            })
        }
    };

    body
}

#[cfg(test)]
fn view_with(lab: &Lab, dispatch: Rc<dyn Fn(Message)>) -> AnyView {
    let tabs = panel_tabs(lab.panel, dispatch.clone());
    let body = panel_body(lab, dispatch.clone());
    let driven = match &lab.target {
        Some(target) => format!("{} frames driven", target.frames()),
        None => "not driving".to_owned(),
    };

    let status = row![
        text(lab.connection.describe()).size(12),
        text(driven).size(12),
        text(format!("{:.1}s", lab.connection.uptime().as_secs_f32())).size(12),
        if lab.connection.is_live() {
            button("Detach").on_press({
                let dispatch = dispatch.clone();
                move || dispatch(Message::Detach)
            })
        } else {
            button("Attach").on_press({
                let dispatch = dispatch.clone();
                move || dispatch(Message::Attach)
            })
        },
    ]
    .gap(12.0);

    container(
        column![
            scroll(tabs).horizontal(),
            container(body).fill_height().padding(8),
            status,
        ]
        .gap(8.0),
    )
    .padding(12)
    .into()
}

/// A panel that is not built, saying so rather than showing an empty box.
fn unbuilt(question: &str, why: &str) -> AnyView {
    container(
        column![text(question).size(16), text(why).size(12)]
            .gap(6.0)
            .fill_width(),
    )
    .center_x()
    .fill_width()
    .center_y()
    .fill_height()
    .into()
}

/// Opens the inspector.
pub fn main() -> taim::Result {
    taim::launch("Taim Lab", build)
}

fn build(cx: &mut Scope) -> AnyView {
    let lab = Rc::new(RefCell::new(Lab::default()));
    let revision = cx.signal(0_u64);
    let selected = cx.signal(Panel::FrameGraph);
    let attached = cx.signal(lab.borrow().connection.is_live());
    let status = cx.signal(status_line(&lab.borrow()));
    let sender_slot = Rc::new(RefCell::new(None::<Sender<Message>>));
    let dispatch = cx.channel({
        let lab = lab.clone();
        let sender_slot = sender_slot.clone();
        move |message| {
            let ticking = matches!(message, Message::Tick);
            let restart = matches!(message, Message::Attach);
            let mut lab = lab.borrow_mut();
            update(&mut lab, message);
            if restart && lab.target.is_some() {
                lab.timer = Some(Ticker::new(sender_slot.borrow().as_ref().unwrap().clone()));
            }
            let _ = selected.set(lab.panel);
            let _ = attached.set(lab.connection.is_live());
            let _ = status.set(status_line(&lab));
            if !ticking
                || !matches!(
                    lab.panel,
                    Panel::DeviceProfiles | Panel::GpuResources | Panel::OverdrawOverlay
                )
            {
                revision.update(|value| *value = value.wrapping_add(1));
            }
        }
    });
    *sender_slot.borrow_mut() = Some(dispatch.clone());
    if lab.borrow().target.is_some() {
        lab.borrow_mut().timer = Some(Ticker::new(dispatch.clone()));
    }
    let dispatch: Rc<dyn Fn(Message)> = Rc::new(move |message| {
        let _ = dispatch.send(message);
    });
    let tabs = rebuild(selected, {
        let dispatch = dispatch.clone();
        move |panel| Some(panel_tabs(*panel, dispatch.clone()))
    });
    let body = rebuild(revision, {
        let dispatch = dispatch.clone();
        move |_| Some(panel_body(&lab.borrow(), dispatch.clone()))
    });
    let attachment = rebuild(attached, move |live| {
        let message = if *live {
            Message::Detach
        } else {
            Message::Attach
        };
        let dispatch = dispatch.clone();
        Some(
            button(if *live { "Detach" } else { "Attach" })
                .on_press(move || dispatch(message.clone()))
                .into(),
        )
    });
    container(
        column![
            scroll(tabs).horizontal(),
            container(scroll(body)).padding(8).fill_height(),
            row![text(move || status.get()).size(12), attachment].gap(12.0),
        ]
        .gap(8.0),
    )
    .padding(12)
    .into()
}

fn status_line(lab: &Lab) -> String {
    let driven = lab.target.as_ref().map_or_else(
        || "not driving".to_owned(),
        |target| format!("{} frames driven", target.frames()),
    );
    format!(
        "{} · {} · {:.1}s",
        lab.connection.describe(),
        driven,
        lab.connection.uptime().as_secs_f32()
    )
}

/// A dedicated timer keeps driving the hosted fixture while the inspector
/// is occluded. Detach stops it; closing disconnects the bounded mailbox.
struct Ticker {
    active: Arc<AtomicBool>,
    pending: Arc<AtomicBool>,
}

impl Ticker {
    fn new(sender: Sender<Message>) -> Self {
        let active = Arc::new(AtomicBool::new(true));
        let running = active.clone();
        let pending = Arc::new(AtomicBool::new(false));
        let queued = pending.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("taim-lab-clock".into())
            .spawn(move || {
                while running.load(Ordering::Acquire) {
                    if !queued.swap(true, Ordering::AcqRel) {
                        match sender.send(Message::Tick) {
                            Ok(()) => {}
                            Err(std::sync::mpsc::TrySendError::Full(_)) => {
                                queued.store(false, Ordering::Release);
                            }
                            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => break,
                        }
                    }
                    std::thread::sleep(TICK);
                }
            })
        {
            eprintln!("Taim Lab clock could not start: {error}");
        }
        Self { active, pending }
    }
}

impl Drop for Ticker {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_inspector_routes_controls_and_detaches_through_the_window_mailbox() {
        use taim_core::{
            Event, Point, Size, mouse,
            semantics::{Node, Role},
            time::Instant,
        };
        fn button_bounds(node: &Node, label: &str) -> Option<taim_core::Rectangle> {
            if node.role == Role::Button && node.name.as_deref() == Some(label) {
                return Some(node.bounds);
            }
            node.children
                .iter()
                .find_map(|child| button_bounds(child, label))
        }
        let renderer = taim_futures::futures::executor::block_on(
            <taim::Renderer as taim_core::renderer::Headless>::new(
                taim_core::renderer::Settings::default(),
                Some("tiny-skia"),
            ),
        )
        .expect("CPU renderer");
        let mut root = taim::view::Root::new(build);
        let window = Size::new(1200.0, 800.0);
        let _ = root.take_changes();
        root.layout(&renderer, window);
        for label in ["Widgets", "Semantics", "Devices", "watch", "Detach"] {
            let tree = root.describe(window);
            let bounds =
                button_bounds(&tree, label).unwrap_or_else(|| panic!("missing {label} button"));
            let at = Point::new(
                bounds.x + bounds.width / 2.0,
                bounds.y + bounds.height / 2.0,
            );
            for event in [
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            ] {
                let _ = root.handle(&event, Some(at), Instant::now());
            }
            let _ = root.take_changes();
            root.layout(&renderer, window);
        }
        assert!(button_bounds(&root.describe(window), "Attach").is_some());
        assert!(button_bounds(&root.describe(window), "Detach").is_none());
    }

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
        update(&mut lab, Message::Detach);

        for panel in [Panel::OverdrawOverlay, Panel::GpuResources] {
            update(&mut lab, Message::Show(panel));

            let _ = std::hint::black_box(view(&lab));
        }
    }

    #[test]
    fn every_panel_renders_while_attached() {
        // The cheapest guard against a panel wired to the wrong field: all
        // nine are built against a real driven frame.
        let mut lab = Lab::default();
        update(&mut lab, Message::Tick);

        for panel in Panel::ALL {
            update(&mut lab, Message::Show(panel));

            let _ = std::hint::black_box(view(&lab));
        }
    }

    #[test]
    fn selecting_a_device_profile_relays_the_target_out() {
        // Simulation, not preview: a phone is not a desktop with fewer
        // pixels, and the only way to know what a view does at 360x780 is
        // to lay it out at 360x780.
        let mut lab = Lab::default();
        update(&mut lab, Message::Tick);

        let phone = device_profiles::Profile::ALL[0];
        update(&mut lab, Message::Profile(phone));
        update(&mut lab, Message::Tick);

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
            update(&mut lab, Message::Tick);
        }

        update(&mut lab, Message::Attach);

        assert_eq!(lab.activity.run(), 0);
        assert_eq!(lab.activity.longest(), 0);
    }

    #[test]
    fn a_built_panel_shows_the_targets_data() {
        // The end-to-end claim for the tree panels: drive a frame, and the
        // snapshot the panels read is populated.
        let mut lab = Lab::default();

        update(&mut lab, Message::Tick);

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
            update(&mut lab, Message::Tick);
        }

        let collector = lab.connection.collector().expect("attached");
        let frames = target::frames(&collector.events());

        assert_eq!(frames.len(), 3);
        assert!(!frames[0].phases.is_empty(), "and they are attributed");
    }

    #[test]
    fn attaching_and_detaching_round_trips() {
        let mut lab = Lab::default();

        update(&mut lab, Message::Detach);
        assert!(!lab.connection.is_live());
        assert!(lab.target.is_none(), "detaching must stop driving the view");

        update(&mut lab, Message::Attach);
        assert!(lab.connection.is_live());
        assert!(lab.target.is_some());
    }

    #[test]
    fn a_detached_inspector_does_not_keep_a_timer_running() {
        // A timer firing into a `None` would keep the inspector awake and
        // burning battery to do nothing -- the opposite of what a profiler
        // should model.
        let mut lab = Lab::default();
        update(&mut lab, Message::Detach);

        // `Subscription` has no public emptiness predicate, so the check
        // is that detaching drops the target the subscription keys off.
        assert!(lab.target.is_none());
    }

    #[test]
    fn a_tick_while_detached_is_harmless() {
        let mut lab = Lab::default();

        update(&mut lab, Message::Detach);
        update(&mut lab, Message::Tick);

        assert!(lab.connection.collector().is_none());
    }

    #[test]
    fn showing_a_panel_selects_it() {
        let mut lab = Lab::default();

        update(&mut lab, Message::Show(Panel::GpuResources));

        assert_eq!(lab.panel, Panel::GpuResources);
    }
}
