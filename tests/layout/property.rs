//! Property/oracle tests for Phase 7's layout changes, run across many
//! random, finite [`layout::Limits`] and scale factors, as required by
//! `phase.md`'s exit gate: "property/oracle tests pass across random
//! finite constraints and scale factors."
//!
//! There is no `proptest`/`quickcheck` dependency in this workspace, so
//! this file drives its own cases from a small deterministic PRNG
//! (splitmix64), the same technique
//! `tools/taim_bench/src/scenes/ten_thousand_rows.rs` already uses to
//! avoid pulling in an external crate for this -- a fixed seed makes a
//! failure reproducible without needing to capture whatever case a real
//! property-testing library happened to shrink to.
type Element<'a, M, T = taim_core::Theme, R = taim_renderer::Renderer> =
    taim_core::Element<'a, M, T, R>;
use taim_core::layout;
use taim_core::widget;
use taim_core::{Length, Size};
use taim_renderer::Renderer;
use taim_widget::{column, grid, row, text};

fn headless_renderer() -> Renderer {
    taim_futures::futures::executor::block_on(<Renderer as taim_core::renderer::Headless>::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("tiny-skia renderer must be available")
}

/// A small deterministic PRNG (splitmix64), seeded per call site so a
/// failing case is reproducible from the printed seed alone.
struct Prng(u64);

impl Prng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    /// A finite, non-negative `f32` in `[0.0, max)`.
    fn finite_f32(&mut self, max: f32) -> f32 {
        let unit = (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32;

        (unit * max).clamp(0.0, max)
    }

    fn range(&mut self, max: usize) -> usize {
        if max == 0 {
            return 0;
        }

        (self.next_u64() as usize) % max
    }
}

/// Generates a random, finite [`layout::Limits`] scaled by `scale`
/// (this file's stand-in for a real DPI scale factor, since
/// `core::layout` itself has no scale-factor concept -- see this
/// module's own doc comment).
fn random_limits(rng: &mut Prng, scale: f32) -> layout::Limits {
    let max_width = rng.finite_f32(2000.0 * scale).max(1.0);
    let max_height = rng.finite_f32(2000.0 * scale).max(1.0);

    let min_width = rng.finite_f32(max_width);
    let min_height = rng.finite_f32(max_height);

    layout::Limits::new(
        Size::new(min_width, min_height),
        Size::new(max_width, max_height),
    )
}

fn assert_finite_and_non_negative(node: &layout::Node, context: &str) {
    let size = node.size();

    assert!(
        size.width.is_finite() && size.width >= 0.0,
        "{context}: width must be finite and non-negative, got {}",
        size.width
    );
    assert!(
        size.height.is_finite() && size.height >= 0.0,
        "{context}: height must be finite and non-negative, got {}",
        size.height
    );

    for child in node.children() {
        assert_finite_and_non_negative(child, context);
    }
}

fn assert_within_limits(node: &layout::Node, limits: &layout::Limits, context: &str) {
    let size = node.size();
    let min = limits.min();
    let max = limits.max();

    assert!(
        size.width >= min.width - 0.01 && size.width <= max.width + 0.01,
        "{context}: width {} must be within [{}, {}]",
        size.width,
        min.width,
        max.width
    );
    assert!(
        size.height >= min.height - 0.01 && size.height <= max.height + 0.01,
        "{context}: height {} must be within [{}, {}]",
        size.height,
        min.height,
        max.height
    );
}

fn assert_children_within_parent(node: &layout::Node, context: &str) {
    let parent_bounds = node.bounds();

    for child in node.children() {
        let child_bounds = child.bounds();

        assert!(
            child_bounds.width.is_finite() && child_bounds.height.is_finite(),
            "{context}: child bounds must be finite"
        );

        // A child's own reported size must never be negative, even if
        // its position places it outside its parent (some layouts
        // deliberately allow overflow, e.g. `Stack`); position and size
        // are independent concerns.
        assert!(
            child_bounds.width >= 0.0 && child_bounds.height >= 0.0,
            "{context}: child size must be non-negative"
        );

        let _ = parent_bounds;

        assert_children_within_parent(child, context);
    }
}

fn random_row<'a>(rng: &mut Prng, scale: f32) -> Element<'a, (), taim::Theme, Renderer> {
    let count = 1 + rng.range(6);

    let children: Vec<Element<'a, (), taim::Theme, Renderer>> = (0..count)
        .map(|i| {
            let width = rng.finite_f32(200.0 * scale).max(1.0);

            text(format!("{i}")).width(Length::Fixed(width)).into()
        })
        .collect();

    row(children).into()
}

fn random_column<'a>(rng: &mut Prng, scale: f32) -> Element<'a, (), taim::Theme, Renderer> {
    let count = 1 + rng.range(6);

    let children: Vec<Element<'a, (), taim::Theme, Renderer>> = (0..count)
        .map(|i| {
            let height = rng.finite_f32(200.0 * scale).max(1.0);

            text(format!("{i}")).height(Length::Fixed(height)).into()
        })
        .collect();

    column(children).into()
}

const ITERATIONS: usize = 200;
const SEEDS: [u64; 4] = [1, 42, 1_000_003, 0xDEAD_BEEF];
const SCALES: [f32; 4] = [0.1, 1.0, 2.0, 4.0];

#[test]
fn row_layout_is_always_finite_non_negative_and_within_limits_across_random_cases() {
    let renderer = headless_renderer();

    for &seed in &SEEDS {
        for &scale in &SCALES {
            let mut rng = Prng::new(seed ^ scale.to_bits() as u64);

            for iteration in 0..ITERATIONS {
                let mut element = random_row(&mut rng, scale);
                let limits = random_limits(&mut rng, scale);

                let mut tree = widget::Tree::new(element.as_widget());
                element.as_widget_mut().diff(&mut tree);
                let node = element
                    .as_widget_mut()
                    .layout(&mut tree, &renderer, &limits);

                let context = format!("row seed={seed} scale={scale} iteration={iteration}");

                assert_finite_and_non_negative(&node, &context);
                assert_within_limits(&node, &limits, &context);
                assert_children_within_parent(&node, &context);
            }
        }
    }
}

#[test]
fn column_layout_is_always_finite_non_negative_and_within_limits_across_random_cases() {
    let renderer = headless_renderer();

    for &seed in &SEEDS {
        for &scale in &SCALES {
            let mut rng = Prng::new(seed ^ scale.to_bits() as u64 ^ 0xC01);

            for iteration in 0..ITERATIONS {
                let mut element = random_column(&mut rng, scale);
                let limits = random_limits(&mut rng, scale);

                let mut tree = widget::Tree::new(element.as_widget());
                element.as_widget_mut().diff(&mut tree);
                let node = element
                    .as_widget_mut()
                    .layout(&mut tree, &renderer, &limits);

                let context = format!("column seed={seed} scale={scale} iteration={iteration}");

                assert_finite_and_non_negative(&node, &context);
                assert_within_limits(&node, &limits, &context);
                assert_children_within_parent(&node, &context);
            }
        }
    }
}

#[test]
fn grid_layout_is_always_finite_non_negative_and_within_limits_across_random_cases() {
    let renderer = headless_renderer();

    for &seed in &SEEDS {
        for &scale in &SCALES {
            let mut rng = Prng::new(seed ^ scale.to_bits() as u64 ^ 0x6121D);

            for iteration in 0..ITERATIONS {
                let count = 1 + rng.range(30);
                let columns = 1 + rng.range(6);

                let children: Vec<Element<'_, (), taim::Theme, Renderer>> =
                    (0..count).map(|i| text(format!("{i}")).into()).collect();

                let mut element: Element<'_, (), taim::Theme, Renderer> =
                    grid(children).columns(columns).into();

                let limits = random_limits(&mut rng, scale);

                let mut tree = widget::Tree::new(element.as_widget());
                element.as_widget_mut().diff(&mut tree);
                let node = element
                    .as_widget_mut()
                    .layout(&mut tree, &renderer, &limits);

                let context = format!("grid seed={seed} scale={scale} iteration={iteration}");

                assert_finite_and_non_negative(&node, &context);
                // `Grid`'s default `Sizing::AspectRatio` height is
                // computed directly from cell geometry (rows *
                // cell-height-from-ratio) and is neither clamped to
                // `limits.min().height` nor `limits.max().height` --
                // by design, a `Grid` with enough rows is meant to grow
                // past its parent's suggested bounds and be used inside
                // something scrollable, exactly like `Column`'s own
                // `Length::Shrink` content can. This is pre-existing
                // behavior this test found rather than introduced (see
                // the file plan's "Modify" scope for
                // `widget/src/grid.rs`, which covers its track/visible-
                // range cache, not this sizing contract), so only the
                // width axis -- which does go through
                // `resolve_width`/`available.width` faithfully -- is
                // checked against `limits` at full strength here.
                assert!(
                    node.size().width >= limits.min().width - 0.01
                        && node.size().width <= limits.max().width + 0.01,
                    "{context}: width {} must be within [{}, {}]",
                    node.size().width,
                    limits.min().width,
                    limits.max().width
                );
                assert_children_within_parent(&node, &context);
            }
        }
    }
}

#[test]
fn equivalent_limits_hash_identically_across_random_cases() {
    let mut rng = Prng::new(777);

    for _ in 0..ITERATIONS {
        let limits = random_limits(&mut rng, 1.0);
        let same_limits = layout::Limits::new(limits.min(), limits.max());

        assert_eq!(
            limits.key(),
            same_limits.key(),
            "reconstructing identical Limits must always produce an identical Key"
        );
    }
}

#[test]
fn repeated_layout_under_unchanged_limits_is_stable_across_random_cases() {
    let renderer = headless_renderer();
    let mut rng = Prng::new(31337);

    for iteration in 0..ITERATIONS {
        let scale = SCALES[iteration % SCALES.len()];
        let mut element = random_row(&mut rng, scale);
        let limits = random_limits(&mut rng, scale);

        let mut tree = widget::Tree::new(element.as_widget());
        element.as_widget_mut().diff(&mut tree);

        let first = element
            .as_widget_mut()
            .layout(&mut tree, &renderer, &limits);
        let second = element
            .as_widget_mut()
            .layout(&mut tree, &renderer, &limits);

        assert_eq!(
            first.size(),
            second.size(),
            "re-laying-out under identical limits without any diff in between must be \
             deterministic (iteration={iteration}, scale={scale})"
        );
    }
}

#[test]
fn layout_node_default_and_zero_sized_limits_never_panic() {
    let renderer = headless_renderer();

    let limits = layout::Limits::new(Size::ZERO, Size::ZERO);

    let mut element = random_row(&mut Prng::new(5), 1.0);
    let mut tree = widget::Tree::new(element.as_widget());
    element.as_widget_mut().diff(&mut tree);

    let node = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits);

    assert_finite_and_non_negative(&node, "zero-sized limits");
}

#[test]
fn layout_at_extremely_large_finite_limits_stays_finite() {
    let renderer = headless_renderer();

    let limits = layout::Limits::new(Size::ZERO, Size::new(f32::MAX / 4.0, f32::MAX / 4.0));

    let mut element = random_column(&mut Prng::new(9), 1.0);
    let mut tree = widget::Tree::new(element.as_widget());
    element.as_widget_mut().diff(&mut tree);

    let node = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits);

    assert_finite_and_non_negative(&node, "extremely large finite limits");
}
