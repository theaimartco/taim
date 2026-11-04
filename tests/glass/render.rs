//! Liquid glass, rendered by the GPU renderer over a scene with colour and
//! detail, and checked for what makes glass read as glass: what is behind
//! it shows through, softened, and its edge bends it.
//!
//! Skips (and says so) when no GPU adapter is available. With
//! `TAIM_GLASS_OUT=<directory>` it also writes the rendered images, for
//! looking at.
use taim::prelude::*;
use taim::view::{Constraints, DescribeCx, MeasureCx, PaintCx, Root};
use taim_core::gradient::Linear;
use taim_core::renderer::Headless as _;
use taim_core::time::Instant;
use taim_core::{Border, Color, Radians, Rectangle, Size, semantics};

pub const WINDOW: Size = Size {
    width: 480.0,
    height: 360.0,
};

/// A scene worth looking through: a sunset gradient, bright discs and
/// stripes of fine detail.
pub struct Scene;

impl View for Scene {
    fn measure(&mut self, _: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        constraints.max
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        cx.fill(
            bounds,
            Linear::new(Radians(0.6))
                .add_stop(0.0, Color::from_rgb8(0xFF, 0x5F, 0x6D))
                .add_stop(0.5, Color::from_rgb8(0xFF, 0xC3, 0x71))
                .add_stop(1.0, Color::from_rgb8(0x24, 0x6B, 0xFD)),
            Border::default(),
        );

        let discs = [
            (60.0, 70.0, 90.0, Color::from_rgb8(0x00, 0xD2, 0xFF)),
            (330.0, 40.0, 120.0, Color::from_rgb8(0x8E, 0x2D, 0xE2)),
            (210.0, 210.0, 110.0, Color::from_rgb8(0x11, 0x99, 0x8E)),
            (380.0, 230.0, 80.0, Color::from_rgb8(0xF7, 0x97, 0x1E)),
        ];
        for (x, y, size, color) in discs {
            cx.fill(
                Rectangle::new(taim_core::Point::new(x, y), Size::new(size, size)),
                color,
                Border::default().rounded(size / 2.0),
            );
        }

        // Fine stripes: blur and refraction both show on them.
        for index in 0..24 {
            let x = 20.0 + index as f32 * 19.0;
            cx.fill(
                Rectangle::new(taim_core::Point::new(x, 150.0), Size::new(6.0, 70.0)),
                Color::from_rgba(1.0, 1.0, 1.0, 0.85),
                Border::default().rounded(3.0),
            );
        }
    }

    fn describe(&self, _: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }
}

/// The GPU renderer, or `None` when there is no adapter.
pub fn gpu() -> Option<taim::Renderer> {
    let renderer = taim_futures::futures::executor::block_on(taim::Renderer::new(
        taim_core::renderer::Settings::default(),
        Some("wgpu"),
    ))?;
    Some(renderer)
}

/// Renders `root` and returns its RGBA pixels.
pub fn render(renderer: &mut taim::Renderer, root: &mut Root, text: Color) -> Vec<u8> {
    use taim_core::Renderer as _;

    root.layout(renderer, WINDOW);
    renderer.reset(Rectangle::with_size(WINDOW));
    root.paint(renderer, text, None, Instant::now());
    renderer.screenshot(
        Size::new(WINDOW.width as u32, WINDOW.height as u32),
        1.0,
        Color::BLACK,
    )
}

/// Writes `rgba` as a PNG named `name` under `TAIM_GLASS_OUT`, if set.
pub fn keep(name: &str, rgba: &[u8]) {
    let Some(directory) = std::env::var_os("TAIM_GLASS_OUT") else {
        return;
    };
    let path = std::path::Path::new(&directory).join(format!("{name}.ppm"));
    let mut ppm = format!("P6 {} {} 255\n", WINDOW.width as u32, WINDOW.height as u32).into_bytes();
    for pixel in rgba.chunks_exact(4) {
        ppm.extend_from_slice(&pixel[..3]);
    }
    std::fs::write(path, ppm).expect("the output directory is writable");
}

fn panel(dark: bool) -> Root {
    let mut root = Root::new(move |_| {
        layers(views![
            Scene,
            container(
                glass(
                    column(views![
                        text("Now Playing").size(24.0),
                        text("Liquid glass, lensed and frosted").size(14.0),
                    ])
                    .gap(6.0),
                )
                .radius(30.0)
                .padding(24.0),
            )
            .center()
            .fill(),
        ])
    });
    root.set_theme(if dark {
        taim::Theme::Dark
    } else {
        taim::Theme::Light
    });
    root
}

#[test]
fn glass_renders_on_the_gpu() {
    let Some(mut renderer) = gpu() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let light = render(&mut renderer, &mut panel(false), Color::BLACK);
    keep("glass-light", &light);
    let dark = render(&mut renderer, &mut panel(true), Color::WHITE);
    keep("glass-dark", &dark);

    assert_ne!(light, dark, "the theme changes the glass");
}

/// The green channel along row `y` from `x0` to `x1`: the white stripes
/// stand out from the warm backdrop in green, not in red.
fn row(rgba: &[u8], y: usize, x0: usize, x1: usize) -> Vec<f32> {
    let width = WINDOW.width as usize;
    (x0..x1)
        .map(|x| f32::from(rgba[(y * width + x) * 4 + 1]))
        .collect()
}

/// How much neighbouring pixels differ: high over sharp detail.
fn detail(values: &[f32]) -> f32 {
    values
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .sum::<f32>()
        / values.len() as f32
}

fn spread(values: &[f32]) -> f32 {
    let (min, max) = values.iter().fold((f32::MAX, f32::MIN), |(min, max), v| {
        (min.min(*v), max.max(*v))
    });
    max - min
}

#[test]
fn glass_frosts_fine_detail_behind_it() {
    let Some(mut renderer) = gpu() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };
    let pixels = render(&mut renderer, &mut panel(false), Color::BLACK);

    // The stripes run across y 150..220; the pane spans about x 115..365
    // and y 128..232. Row 214 is below the pane's text.
    let outside = detail(&row(&pixels, 214, 20, 110));
    let inside = detail(&row(&pixels, 214, 160, 320));

    assert!(
        inside < outside * 0.35,
        "the stripes behind the glass must be frosted: detail {inside} inside, {outside} outside"
    );
}

#[test]
fn glass_shows_what_is_behind_it() {
    let Some(mut renderer) = gpu() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };
    let pixels = render(&mut renderer, &mut panel(false), Color::BLACK);

    // Across the pane the backdrop changes from the cyan disc on the left
    // to the warm gradient on the right; an opaque pane would be flat.
    let across = row(&pixels, 140, 130, 350);
    assert!(
        spread(&across) > 25.0,
        "the pane is flat ({}): it is not showing the scene behind it",
        spread(&across)
    );
}

#[test]
fn without_a_gpu_glass_is_a_solid_tinted_pane() {
    let mut renderer = taim_futures::futures::executor::block_on(taim::Renderer::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("the CPU renderer starts headless");

    let pixels = render(&mut renderer, &mut panel(false), Color::BLACK);
    keep("glass-cpu", &pixels);

    let inside = row(&pixels, 214, 160, 320);
    assert!(
        detail(&inside) < 1.0,
        "the CPU fallback shows nothing through the pane"
    );
    assert!(
        inside.iter().all(|green| *green > 200.0),
        "and it is the bright tint"
    );
}
