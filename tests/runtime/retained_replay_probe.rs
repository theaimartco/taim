//! Live-path retained replay regressions.

use taim::advanced::widget::{self, Widget};
use taim::advanced::{Layout, mouse, renderer};
use taim::{Element, Length, Rectangle, Size, Theme};
use taim_renderer::Renderer;
use taim_runtime::user_interface::{self, UserInterface};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

struct Quiet(Arc<AtomicUsize>);

impl<Message> Widget<Message, Theme, Renderer> for Quiet {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(10.0), Length::Fixed(10.0))
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        _limits: &taim::advanced::layout::Limits,
    ) -> taim::advanced::layout::Node {
        taim::advanced::layout::Node::new(Size::new(10.0, 10.0))
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use taim::advanced::renderer::Renderer as _;
        let _ = self.0.fetch_add(1, Ordering::Relaxed);
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle::new([0.0, 0.0].into(), Size::new(10.0, 10.0)),
                ..Default::default()
            },
            taim::Color::BLACK,
        );
    }
}

fn headless_renderer() -> Renderer {
    taim_futures::futures::executor::block_on(
        <Renderer as taim::advanced::renderer::Headless>::new(
            taim::advanced::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("tiny-skia renderer must be available")
}

#[test]
fn an_unchanged_interface_replays_without_reentering_widget_paint() {
    use taim::advanced::renderer::Renderer as _;

    let mut renderer = headless_renderer();
    let cache = user_interface::Cache::new();
    let bounds = Size::new(100.0, 100.0);

    let paint_calls = Arc::new(AtomicUsize::new(0));
    let element: Element<'_, (), Theme, Renderer> = Element::new(Quiet(Arc::clone(&paint_calls)));
    let mut ui = UserInterface::build(element, bounds, cache, &mut renderer);

    for _ in 0..5 {
        ui.draw(
            &mut renderer,
            &Theme::Light,
            &renderer::Style::default(),
            mouse::Cursor::Unavailable,
        );
    }

    let stats = renderer.retained_layer_statistics();
    assert_eq!(stats.misses, 1, "only the first draw should record");
    assert_eq!(stats.hits, 4, "every later draw should replay");
    assert_eq!(paint_calls.load(Ordering::Relaxed), 1);
}

/// A widget whose drawn colour changes every frame. If retained replay
/// hits here, the framework is presenting a stale frame.
struct Changing(f32);

impl<Message> Widget<Message, Theme, Renderer> for Changing {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(10.0), Length::Fixed(10.0))
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        _limits: &taim::advanced::layout::Limits,
    ) -> taim::advanced::layout::Node {
        taim::advanced::layout::Node::new(Size::new(10.0, 10.0))
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use taim::advanced::renderer::Renderer as _;
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle::new([0.0, 0.0].into(), Size::new(10.0, 10.0)),
                ..Default::default()
            },
            taim::Color::from_rgb(self.0, 0.0, 0.0),
        );
    }
}

struct LayoutProbe {
    layouts: Arc<AtomicUsize>,
    paints: Arc<AtomicUsize>,
}

struct LiveValueProbe {
    signal: taim::Signal<u32>,
    layouts: Arc<AtomicUsize>,
    paints: Arc<AtomicUsize>,
}

impl<Message> Widget<Message, Theme, Renderer> for LiveValueProbe {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(10.0), Length::Fixed(10.0))
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        _limits: &taim::advanced::layout::Limits,
    ) -> taim::advanced::layout::Node {
        let _ = self.layouts.fetch_add(1, Ordering::Relaxed);
        taim::advanced::layout::Node::new(Size::new(10.0, 10.0))
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use taim::advanced::renderer::Renderer as _;

        let _ = self.paints.fetch_add(1, Ordering::Relaxed);
        let red = self.signal.get() as f32 / 10.0;
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle::new([0.0, 0.0].into(), Size::new(10.0, 10.0)),
                ..Default::default()
            },
            taim::Color::from_rgb(red, 0.0, 0.0),
        );
    }
}

impl<Message> Widget<Message, Theme, Renderer> for LayoutProbe {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(10.0), Length::Fixed(10.0))
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        _limits: &taim::advanced::layout::Limits,
    ) -> taim::advanced::layout::Node {
        let _ = self.layouts.fetch_add(1, Ordering::Relaxed);
        taim::advanced::layout::Node::new(Size::new(10.0, 10.0))
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let _ = self.paints.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn rebuilding_changed_content_never_replays_stale_pixels() {
    use taim::advanced::renderer::Renderer as _;

    let mut renderer = headless_renderer();
    let mut cache = user_interface::Cache::new();
    let bounds = Size::new(100.0, 100.0);

    for frame in 0..5 {
        let element: Element<'_, (), Theme, Renderer> = Element::new(Changing(frame as f32 / 5.0));
        let mut ui = UserInterface::build(element, bounds, cache, &mut renderer);
        ui.draw(
            &mut renderer,
            &Theme::Light,
            &renderer::Style::default(),
            mouse::Cursor::Unavailable,
        );
        cache = ui.into_cache();
    }

    let stats = renderer.retained_layer_statistics();
    assert_eq!(
        stats.hits, 0,
        "content changed every frame, so replay must never hit"
    );
}

#[test]
fn a_reactive_subtree_rebuilds_only_when_its_signal_changes() {
    let signal = taim::Signal::new(0.0_f32);
    let builds = Arc::new(AtomicUsize::new(0));
    let layouts = Arc::new(AtomicUsize::new(0));
    let paints = Arc::new(AtomicUsize::new(0));
    let mut renderer = headless_renderer();
    let mut cache = user_interface::Cache::new();
    let bounds = Size::new(100.0, 100.0);

    let element = |signal: taim::Signal<f32>| -> Element<'static, (), Theme, Renderer> {
        let builds = Arc::clone(&builds);
        let layouts = Arc::clone(&layouts);
        let paints = Arc::clone(&paints);
        taim::widget::reactive(signal, move |_value| {
            let _ = builds.fetch_add(1, Ordering::Relaxed);
            Element::new(LayoutProbe {
                layouts: Arc::clone(&layouts),
                paints: Arc::clone(&paints),
            })
        })
        .into()
    };

    for _ in 0..3 {
        let mut ui = UserInterface::build(element(signal.clone()), bounds, cache, &mut renderer);
        ui.draw(
            &mut renderer,
            &Theme::Light,
            &renderer::Style::default(),
            mouse::Cursor::Unavailable,
        );
        cache = ui.into_cache();
    }
    assert_eq!(builds.load(Ordering::Relaxed), 1);
    assert_eq!(layouts.load(Ordering::Relaxed), 1);
    assert_eq!(paints.load(Ordering::Relaxed), 1);

    assert!(signal.set(0.5));
    let mut ui = UserInterface::build(element(signal), bounds, cache, &mut renderer);
    ui.draw(
        &mut renderer,
        &Theme::Light,
        &renderer::Style::default(),
        mouse::Cursor::Unavailable,
    );
    let _cache = ui.into_cache();

    assert_eq!(builds.load(Ordering::Relaxed), 2);
    assert_eq!(layouts.load(Ordering::Relaxed), 2);
    assert_eq!(paints.load(Ordering::Relaxed), 2);
}

#[test]
fn a_signal_wakes_and_updates_a_live_interface_without_rebuilding_the_root() {
    let signal = taim::Signal::new(0_u32);
    let builds = Arc::new(AtomicUsize::new(0));
    let layouts = Arc::new(AtomicUsize::new(0));
    let paints = Arc::new(AtomicUsize::new(0));
    let wakes = Arc::new(AtomicUsize::new(0));
    let routed_source = Arc::new(Mutex::new(None));

    let element: Element<'static, (), Theme, Renderer> = {
        let builds = Arc::clone(&builds);
        let layouts = Arc::clone(&layouts);
        let paints = Arc::clone(&paints);
        taim::widget::reactive(signal.clone(), move |_value| {
            let _ = builds.fetch_add(1, Ordering::Relaxed);
            Element::new(LayoutProbe {
                layouts: Arc::clone(&layouts),
                paints: Arc::clone(&paints),
            })
        })
        .into()
    };

    let mut renderer = headless_renderer();
    let mut ui = UserInterface::build(
        element,
        Size::new(100.0, 100.0),
        user_interface::Cache::new(),
        &mut renderer,
    );
    ui.draw(
        &mut renderer,
        &Theme::Light,
        &renderer::Style::default(),
        mouse::Cursor::Unavailable,
    );

    let window = taim_core::window::Headless;
    let observed_wakes = Arc::clone(&wakes);
    let observed_source = Arc::clone(&routed_source);
    let waker = taim_core::shell::Waker::new_tagged(move |source| {
        let _ = observed_wakes.fetch_add(1, Ordering::Relaxed);
        *observed_source.lock().expect("routed source lock poisoned") = source;
    });
    let mut messages = Vec::new();

    // The first runtime event gives the reactive widget the event-loop waker.
    let _ = ui.update(
        &window,
        &waker,
        &[taim::Event::Waken],
        mouse::Cursor::Unavailable,
        &mut renderer,
        &mut messages,
    );

    assert!(signal.set(1));
    assert_eq!(wakes.load(Ordering::Relaxed), 1);
    let source = routed_source
        .lock()
        .expect("routed source lock poisoned")
        .expect("signal wake must carry its source");

    let (state, _) = ui.update(
        &window,
        &waker,
        &[taim::Event::Reactive(source)],
        mouse::Cursor::Unavailable,
        &mut renderer,
        &mut messages,
    );
    assert!(matches!(
        state,
        user_interface::State::Updated {
            redraw_request: taim_core::window::RedrawRequest::NextFrame,
            ..
        }
    ));

    ui.draw(
        &mut renderer,
        &Theme::Light,
        &renderer::Style::default(),
        mouse::Cursor::Unavailable,
    );

    assert_eq!(builds.load(Ordering::Relaxed), 2);
    assert_eq!(layouts.load(Ordering::Relaxed), 2);
    assert_eq!(paints.load(Ordering::Relaxed), 2);
}

#[test]
fn a_paint_only_signal_reuses_layout() {
    let signal = taim::Signal::new(0_u32);
    let layouts = Arc::new(AtomicUsize::new(0));
    let paints = Arc::new(AtomicUsize::new(0));
    let element: Element<'static, (), Theme, Renderer> = {
        let layouts = Arc::clone(&layouts);
        let paints = Arc::clone(&paints);
        taim::widget::reactive_paint(signal.clone(), move |_value| {
            Element::new(LayoutProbe {
                layouts: Arc::clone(&layouts),
                paints: Arc::clone(&paints),
            })
        })
        .into()
    };

    let mut renderer = headless_renderer();
    let mut ui = UserInterface::build(
        element,
        Size::new(100.0, 100.0),
        user_interface::Cache::new(),
        &mut renderer,
    );
    ui.draw(
        &mut renderer,
        &Theme::Light,
        &renderer::Style::default(),
        mouse::Cursor::Unavailable,
    );

    let routed = Arc::new(Mutex::new(None));
    let observed = Arc::clone(&routed);
    let waker = taim_core::shell::Waker::new_tagged(move |source| {
        *observed.lock().expect("routed source lock poisoned") = source;
    });
    let window = taim_core::window::Headless;
    let mut messages = Vec::new();
    let _ = ui.update(
        &window,
        &waker,
        &[taim::Event::Waken],
        mouse::Cursor::Unavailable,
        &mut renderer,
        &mut messages,
    );

    assert!(signal.set(1));
    let source = routed
        .lock()
        .expect("routed source lock poisoned")
        .expect("signal wake must carry its source");
    let _ = ui.update(
        &window,
        &waker,
        &[taim::Event::Reactive(source)],
        mouse::Cursor::Unavailable,
        &mut renderer,
        &mut messages,
    );
    ui.draw(
        &mut renderer,
        &Theme::Light,
        &renderer::Style::default(),
        mouse::Cursor::Unavailable,
    );

    assert_eq!(layouts.load(Ordering::Relaxed), 1);
    assert_eq!(paints.load(Ordering::Relaxed), 2);
}

#[test]
fn a_value_signal_reuses_its_widget_tree_and_layout() {
    let signal = taim::Signal::new(0_u32);
    let builds = Arc::new(AtomicUsize::new(0));
    let layouts = Arc::new(AtomicUsize::new(0));
    let paints = Arc::new(AtomicUsize::new(0));
    let element: Element<'static, (), Theme, Renderer> = {
        let builds = Arc::clone(&builds);
        let layouts = Arc::clone(&layouts);
        let paints = Arc::clone(&paints);

        taim::widget::reactive_value(signal.clone(), move |live| {
            let _ = builds.fetch_add(1, Ordering::Relaxed);
            Element::new(LiveValueProbe {
                signal: live,
                layouts: Arc::clone(&layouts),
                paints: Arc::clone(&paints),
            })
        })
        .into()
    };

    let mut renderer = headless_renderer();
    let mut ui = UserInterface::build(
        element,
        Size::new(100.0, 100.0),
        user_interface::Cache::new(),
        &mut renderer,
    );
    ui.draw(
        &mut renderer,
        &Theme::Light,
        &renderer::Style::default(),
        mouse::Cursor::Unavailable,
    );

    let routed = Arc::new(Mutex::new(None));
    let observed = Arc::clone(&routed);
    let waker = taim_core::shell::Waker::new_tagged(move |source| {
        *observed.lock().expect("routed source lock poisoned") = source;
    });
    let window = taim_core::window::Headless;
    let mut messages = Vec::new();
    let _ = ui.update(
        &window,
        &waker,
        &[taim::Event::Waken],
        mouse::Cursor::Unavailable,
        &mut renderer,
        &mut messages,
    );

    assert!(signal.set(5));
    let source = routed
        .lock()
        .expect("routed source lock poisoned")
        .expect("signal wake must carry its source");
    let _ = ui.update(
        &window,
        &waker,
        &[taim::Event::Reactive(source)],
        mouse::Cursor::Unavailable,
        &mut renderer,
        &mut messages,
    );
    ui.draw(
        &mut renderer,
        &Theme::Light,
        &renderer::Style::default(),
        mouse::Cursor::Unavailable,
    );

    assert_eq!(builds.load(Ordering::Relaxed), 1);
    assert_eq!(layouts.load(Ordering::Relaxed), 1);
    assert_eq!(paints.load(Ordering::Relaxed), 2);
}
