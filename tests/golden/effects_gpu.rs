//! The GPU effect pipeline, checked against real pixels.
//!
//! `wgpu/src/effects/pipeline.rs` was an empty struct whose `prepare` and
//! `render` were `// Stub`. Nine modules of bounds arithmetic, tier gating
//! and pooling sat on top of it, all tested, and none of it drew anything --
//! which is why `blur_layers` and `effect_ladder` were the *cheapest* scenes
//! in the benchmark suite (`Capstone.md` F-07).
//!
//! These render through `wgpu`, not `tiny-skia`, because the effect pipeline
//! is a `wgpu` pipeline. They are skipped rather than failed when no adapter
//! is available: a machine with no GPU and no software Vulkan cannot answer
//! the question, and a suite that goes red on such a machine teaches its
//! readers to ignore it.

use aura::{Color, Rectangle, Size};
use aura_core::Renderer as _;
use aura_wgpu::Renderer;
use aura_wgpu::graphics::Viewport;

const WIDTH: u32 = 128;
const HEIGHT: u32 = 128;

/// A renderer on the `wgpu` backend, or `None` when no adapter exists.
fn wgpu_renderer() -> Option<Renderer> {
    // The concrete `wgpu` renderer, not `aura_renderer`'s fallback enum:
    // `add_effect` belongs to the backend that has an effect pipeline.
    aura_futures::futures::executor::block_on(<Renderer as aura_core::renderer::Headless>::new(
        aura_core::renderer::Settings::default(),
        None,
    ))
}

macro_rules! needs_gpu {
    ($renderer:ident) => {
        let Some(mut $renderer) = wgpu_renderer() else {
            eprintln!("skipped: no wgpu adapter available");
            return;
        };
    };
}

/// Draws a hard-edged white square, optionally under `effect`.
fn square(renderer: &mut Renderer, effect: Option<aura_wgpu::effects::Effect>) -> Vec<u8> {
    let logical = Size::new(WIDTH as f32, HEIGHT as f32);

    renderer.reset(Rectangle::with_size(logical));

    renderer.fill_quad(
        aura_core::renderer::Quad {
            bounds: Rectangle {
                x: 32.0,
                y: 32.0,
                width: 64.0,
                height: 64.0,
            },
            ..Default::default()
        },
        Color::WHITE,
    );

    if let Some(effect) = effect {
        assert!(
            renderer.add_effect(effect, aura_wgpu::graphics::quality::Tier::default()),
            "the tier rejected the effect before it could be rendered"
        );
    }

    let viewport = Viewport::with_physical_size(Size::new(WIDTH, HEIGHT), 1.0);

    renderer.screenshot(&viewport, Color::BLACK)
}

/// How many pixels sit strictly between black and white.
///
/// The discriminator for a blur: a hard-edged square has almost none -- only
/// what antialiasing produces along its border -- and a blurred one has a
/// wide band of them. Counting intermediate values rather than comparing to a
/// reference image means the test states the *property* it cares about.
fn intermediate(rgba: &[u8]) -> usize {
    rgba.chunks_exact(4)
        .filter(|pixel| (12..=243).contains(&pixel[0]))
        .count()
}

#[test]
fn a_blur_softens_an_edge_that_was_hard() {
    needs_gpu!(renderer);

    let sharp = square(&mut renderer, None);
    let blurred = square(
        &mut renderer,
        Some(aura_wgpu::effects::Effect::Blur(
            aura_wgpu::effects::blur::Blur { radius: 8.0 },
        )),
    );

    let sharp_edge = intermediate(&sharp);
    let blurred_edge = intermediate(&blurred);

    println!("intermediate pixels: sharp={sharp_edge} blurred={blurred_edge}");

    assert_ne!(sharp, blurred, "the blur changed nothing at all");

    assert!(
        blurred_edge > sharp_edge * 3,
        "a radius-8 blur produced {blurred_edge} intermediate pixels against \
         {sharp_edge} for a hard edge; that is not a blur"
    );
}

#[test]
fn a_blur_conserves_the_image_it_blurs() {
    needs_gpu!(renderer);

    let sharp = square(&mut renderer, None);
    let blurred = square(
        &mut renderer,
        Some(aura_wgpu::effects::Effect::Blur(
            aura_wgpu::effects::blur::Blur { radius: 6.0 },
        )),
    );

    // A Gaussian is normalised, so blurring spreads light without creating or
    // destroying it. A kernel whose weights do not sum to one shows up here
    // as an image that got brighter or darker overall -- the classic bug, and
    // invisible in a single screenshot.
    let total =
        |rgba: &[u8]| -> u64 { rgba.chunks_exact(4).map(|pixel| u64::from(pixel[0])).sum() };

    // Non-vacuous: identical images have identical totals, and this test
    // would then pass while proving nothing.
    assert_ne!(sharp, blurred, "the blur changed nothing at all");

    let before = total(&sharp);
    let after = total(&blurred);

    println!("luminance: sharp={before} blurred={after}");

    let drift = (after as f64 - before as f64).abs() / before as f64;

    assert!(
        drift < 0.10,
        "blurring moved total luminance by {:.1}%; the kernel is not normalised",
        drift * 100.0
    );
}

#[test]
fn a_zero_saturation_matrix_removes_colour() {
    needs_gpu!(renderer);

    let logical = Size::new(WIDTH as f32, HEIGHT as f32);

    renderer.reset(Rectangle::with_size(logical));

    renderer.fill_quad(
        aura_core::renderer::Quad {
            bounds: Rectangle {
                x: 16.0,
                y: 16.0,
                width: 96.0,
                height: 96.0,
            },
            ..Default::default()
        },
        Color::from_rgb(1.0, 0.0, 0.0),
    );

    assert!(
        renderer.add_effect(
            aura_wgpu::effects::Effect::Saturation(aura_wgpu::effects::saturation::Saturation(0.0)),
            aura_wgpu::graphics::quality::Tier::default(),
        ),
        "the tier rejected saturation before it could be rendered"
    );

    let viewport = Viewport::with_physical_size(Size::new(WIDTH, HEIGHT), 1.0);
    let rgba = renderer.screenshot(&viewport, Color::BLACK);

    // Grey means the three channels agree. Pure red does not.
    let centre = ((HEIGHT / 2) * WIDTH + WIDTH / 2) as usize * 4;
    let pixel = &rgba[centre..centre + 3];

    println!("centre pixel after desaturation: {pixel:?}");

    assert!(
        pixel[0].abs_diff(pixel[1]) < 12 && pixel[1].abs_diff(pixel[2]) < 12,
        "saturation 0.0 left {pixel:?}, which is not grey"
    );

    assert!(
        pixel[0] > 8,
        "saturation 0.0 produced black; the matrix ate the image"
    );
}

#[test]
fn noise_perturbs_a_flat_fill_without_shifting_it() {
    needs_gpu!(renderer);

    let flat = square(&mut renderer, None);
    let grainy = square(
        &mut renderer,
        Some(aura_wgpu::effects::Effect::Noise(
            aura_wgpu::effects::noise::Noise { seed: 0x5EED },
        )),
    );

    assert_ne!(flat, grainy, "noise changed nothing at all");

    // Grain is signed, so it must not move the image's overall brightness.
    let total =
        |rgba: &[u8]| -> u64 { rgba.chunks_exact(4).map(|pixel| u64::from(pixel[0])).sum() };

    let before = total(&flat) as f64;
    let after = total(&grainy) as f64;
    let drift = (after - before).abs() / before;

    println!("noise luminance drift: {:.3}%", drift * 100.0);

    assert!(
        drift < 0.02,
        "noise moved total luminance by {:.2}%; the grain is not signed",
        drift * 100.0
    );
}

#[test]
fn noise_is_deterministic_for_a_seed() {
    needs_gpu!(renderer);

    // The same seed must give the same grain, or a static screen shimmers.
    let effect = |seed| {
        Some(aura_wgpu::effects::Effect::Noise(
            aura_wgpu::effects::noise::Noise { seed },
        ))
    };

    let first = square(&mut renderer, effect(7));
    let again = square(&mut renderer, effect(7));
    let other = square(&mut renderer, effect(8));

    assert_eq!(first, again, "the same seed produced different grain");
    assert_ne!(first, other, "different seeds produced identical grain");
}

#[test]
fn a_backdrop_blur_blurs_the_page_and_not_the_sheet() {
    needs_gpu!(renderer);

    // A hard-edged background, then an opaque "sheet" drawn over its middle
    // with a backdrop blur. The blur must soften the background around the
    // sheet -- what is *behind* it -- while the sheet itself stays crisp.
    let logical = Size::new(WIDTH as f32, HEIGHT as f32);

    let draw = |renderer: &mut Renderer, backdrop: bool| {
        renderer.reset(Rectangle::with_size(logical));

        // The page: a hard vertical edge down the middle of the surface.
        renderer.fill_quad(
            aura_core::renderer::Quad {
                bounds: Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: 64.0,
                    height: 128.0,
                },
                ..Default::default()
            },
            Color::WHITE,
        );

        // The sheet, in a layer of its own. A backdrop effect reads what is
        // behind *its* layer, so the page has to be in an earlier one -- put
        // both in the same layer and the blur runs against an empty frame,
        // which is exactly what the first version of this test did.
        let sheet = Rectangle {
            x: 24.0,
            y: 32.0,
            width: 80.0,
            height: 64.0,
        };

        renderer.start_layer(sheet);

        if backdrop {
            assert!(
                renderer.add_effect(
                    aura_wgpu::effects::Effect::BackdropBlur(
                        aura_wgpu::effects::backdrop_blur::BackdropBlur {
                            radius: 10.0,
                            area: 1.0,
                        },
                    ),
                    aura_wgpu::graphics::quality::Tier::default(),
                ),
                "the tier rejected the backdrop blur"
            );
        }

        renderer.end_layer();

        let viewport = Viewport::with_physical_size(Size::new(WIDTH, HEIGHT), 1.0);

        renderer.screenshot(&viewport, Color::BLACK)
    };

    let plain = draw(&mut renderer, false);
    let frosted = draw(&mut renderer, true);

    assert_ne!(plain, frosted, "the backdrop blur changed nothing at all");

    println!(
        "backdrop: plain={} frosted={} intermediate pixels",
        intermediate(&plain),
        intermediate(&frosted)
    );

    assert!(
        intermediate(&frosted) > intermediate(&plain) + 100,
        "the backdrop blur did not soften the page behind the sheet"
    );
}

#[test]
fn a_mask_erases_what_it_does_not_cover() {
    needs_gpu!(renderer);

    let size = Renderer::mask_size() as usize;

    // Opaque on the left half, transparent on the right.
    let mut coverage = vec![0u8; size * size];

    for row in 0..size {
        for column in 0..size / 2 {
            coverage[row * size + column] = 255;
        }
    }

    let slot = renderer
        .upload_mask(0, &coverage)
        .expect("a correctly sized mask in slot 0 must be accepted");

    let masked = square(
        &mut renderer,
        Some(aura_wgpu::effects::Effect::Mask(
            aura_wgpu::effects::mask::Mask { atlas_slot: slot },
        )),
    );

    // The square spans x 32..96, so its own halves are 32..64 and 64..96.
    let sample = |x: u32, y: u32| -> u8 { masked[((y * WIDTH + x) * 4) as usize] };

    let kept = sample(40, 64);
    let erased = sample(88, 64);

    println!("mask: kept={kept} erased={erased}");

    assert!(kept > 200, "the covered half was erased too, got {kept}");
    assert!(erased < 40, "the uncovered half survived, got {erased}");
}

#[test]
fn a_wrongly_sized_mask_is_refused() {
    let Some(renderer) = wgpu_renderer() else {
        eprintln!("skipped: no wgpu adapter available");
        return;
    };

    // Silently accepting one would upload garbage into the atlas and clip
    // every later effect to it.
    assert_eq!(renderer.upload_mask(0, &[255; 16]), None);
    assert_eq!(
        renderer.upload_mask(
            Renderer::mask_slots(),
            &vec![255; (Renderer::mask_size() * Renderer::mask_size()) as usize],
        ),
        None,
        "an out-of-range slot must be refused"
    );
}

/// Writes the blurred square out for a human to look at.
///
/// Not an assertion. The numeric tests above state properties -- more
/// intermediate pixels, conserved luminance -- and a property can hold while
/// the picture is still wrong. `AURA_DUMP_EFFECTS=<dir>` renders the pair to
/// binary PPM so someone can check that a blur looks like a blur.
#[test]
fn dump_for_inspection() {
    let Ok(directory) = std::env::var("AURA_DUMP_EFFECTS") else {
        return;
    };

    needs_gpu!(renderer);

    let _ = std::fs::create_dir_all(&directory);

    for (name, effect) in [
        ("sharp", None),
        (
            "noise",
            Some(aura_wgpu::effects::Effect::Noise(
                aura_wgpu::effects::noise::Noise { seed: 0x5EED },
            )),
        ),
        (
            "desaturated",
            Some(aura_wgpu::effects::Effect::Saturation(
                aura_wgpu::effects::saturation::Saturation(0.0),
            )),
        ),
        (
            "blur8",
            Some(aura_wgpu::effects::Effect::Blur(
                aura_wgpu::effects::blur::Blur { radius: 8.0 },
            )),
        ),
        (
            "blur16",
            Some(aura_wgpu::effects::Effect::Blur(
                aura_wgpu::effects::blur::Blur { radius: 16.0 },
            )),
        ),
    ] {
        let rgba = square(&mut renderer, effect);

        let mut bytes = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
        bytes.extend(rgba.chunks_exact(4).flat_map(|pixel| &pixel[..3]));

        let _ = std::fs::write(format!("{directory}/{name}.ppm"), bytes);
    }
}
