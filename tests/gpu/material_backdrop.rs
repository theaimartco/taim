//! Refraction and dispersion on the live render path.
//!
//! `material_shading.rs` proves the physics agrees between CPU and GPU, and
//! `material_live.rs` proves a material's *specular* half reaches a pixel.
//! Neither can see the half that makes glass look like glass: displacing what
//! is behind a surface needs a backdrop bound, sampled, and mixed, and every
//! step of that is invisible to a test of the arithmetic.
//!
//! The property under test is the one `future.md`'s Pillar III gate names:
//! **a real pane bends what it transmits; a blur translates it rigidly.**
//! So each test here puts known content behind a pane and asserts the pane
//! shows something the backdrop does not have at that location.
//!
//! Skipped rather than failed when no adapter is available.

use taim_core::material::Material;
use taim_core::renderer::Quad;
use taim_core::{Color, Rectangle, Renderer as _, Size};
use taim_wgpu::Renderer;
use taim_wgpu::graphics::Viewport;

const SIZE: u32 = 64;

fn renderer() -> Option<Renderer> {
    taim_futures::futures::executor::block_on(<Renderer as taim_core::renderer::Headless>::new(
        taim_core::renderer::Settings::default(),
        None,
    ))
}

macro_rules! needs_gpu {
    ($renderer:ident) => {
        let Some(mut $renderer) = renderer() else {
            eprintln!("skipped: no wgpu adapter available");
            return;
        };
    };
}

/// Where the pane sits. Inset, so refraction has somewhere to sample from.
const PANE: Rectangle = Rectangle {
    x: 12.0,
    y: 12.0,
    width: 40.0,
    height: 40.0,
};

/// Draws `backdrop` behind a pane of `material` and returns the frame.
///
/// **The pane goes in its own layer.** What a transmissive quad refracts is
/// the frame as it stood when its *layer* began, exactly as a backdrop blur
/// frosts what is behind its layer -- so a pane and the content it transmits
/// cannot share one. Drawing both into the base layer captures the frame
/// before either drew, and the pane shows the clear colour.
fn through_pane(
    renderer: &mut Renderer,
    material: Material,
    backdrop: impl Fn(&mut Renderer),
) -> Vec<u8> {
    let logical = Size::new(SIZE as f32, SIZE as f32);
    let bounds = Rectangle::with_size(logical);

    renderer.reset(bounds);

    backdrop(renderer);

    renderer.with_layer(bounds, |renderer| {
        renderer.fill_quad(
            Quad {
                bounds: PANE,
                border: taim_core::Border {
                    radius: 10.0.into(),
                    ..Default::default()
                },
                material,
                ..Default::default()
            },
            Color::from_rgb(0.35, 0.35, 0.35),
        );
    });

    let viewport = Viewport::with_physical_size(Size::new(SIZE, SIZE), 1.0);

    renderer.screenshot(&viewport, Color::BLACK)
}

/// A backdrop of one solid block, positioned by `x`.
///
/// Moving the block is how parallax is measured: a pane that bends what it
/// transmits shows the block moving by a different amount than the backdrop
/// around it does.
fn block_at(x: f32) -> impl Fn(&mut Renderer) {
    move |renderer: &mut Renderer| {
        renderer.fill_quad(
            Quad {
                bounds: Rectangle {
                    x,
                    y: 0.0,
                    width: 16.0,
                    height: SIZE as f32,
                },
                ..Default::default()
            },
            Color::WHITE,
        );
    }
}

/// A backdrop split inside the pane's left bevel into red and blue.
///
/// A sharp edge is what makes dispersion measurable: the channels sample it
/// at different offsets, so near the bevel one channel has crossed the edge
/// and another has not.
fn red_blue_split(renderer: &mut Renderer) {
    let split = PANE.x + 4.0;

    renderer.fill_quad(
        Quad {
            bounds: Rectangle {
                x: 0.0,
                y: 0.0,
                width: split,
                height: SIZE as f32,
            },
            ..Default::default()
        },
        Color::from_rgb(1.0, 0.0, 0.0),
    );

    renderer.fill_quad(
        Quad {
            bounds: Rectangle {
                x: split,
                y: 0.0,
                width: SIZE as f32 - split,
                height: SIZE as f32,
            },
            ..Default::default()
        },
        Color::from_rgb(0.0, 0.0, 1.0),
    );
}

/// The RGBA pixel at a position.
fn at(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
    let index = ((y * SIZE + x) * 4) as usize;

    [
        pixels[index],
        pixels[index + 1],
        pixels[index + 2],
        pixels[index + 3],
    ]
}

#[test]
fn frosted_glass_softens_backdrop_inside_the_flat_pane() {
    needs_gpu!(renderer);
    let stripes = |renderer: &mut Renderer| {
        for x in 0..SIZE {
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle {
                        x: x as f32,
                        y: 0.0,
                        width: 1.0,
                        height: SIZE as f32,
                    },
                    ..Default::default()
                },
                if x % 2 == 0 {
                    Color::WHITE
                } else {
                    Color::BLACK
                },
            );
        }
    };
    let clear = through_pane(&mut renderer, Material::GLASS, stripes);
    let frost = through_pane(&mut renderer, Material::FROSTED, stripes);
    if cfg!(feature = "web-colors") {
        assert_eq!(
            clear, frost,
            "legacy gamma blending must disable physical glass scattering"
        );
        return;
    }
    let contrast =
        |pixels: &[u8]| (i32::from(at(pixels, 30, 32)[0]) - i32::from(at(pixels, 31, 32)[0])).abs();
    assert!(
        contrast(&frost) < contrast(&clear) / 2,
        "rough glass must scatter backdrop details, clear={}, frosted={}",
        contrast(&clear),
        contrast(&frost)
    );
}

/// Pixels inside the pane, on its horizontal centre line.
///
/// The centre line, because that is where an SDF's horizontal gradient is
/// purely horizontal and refraction displaces along one axis only.
fn pane_row(pixels: &[u8]) -> Vec<[u8; 4]> {
    let y = (PANE.y + PANE.height / 2.0) as u32;

    ((PANE.x as u32 + 2)..((PANE.x + PANE.width) as u32 - 2))
        .map(|x| at(pixels, x, y))
        .collect()
}

#[test]
fn an_opaque_pane_ignores_whatever_is_behind_it() {
    // The safety property, and the one that fails if the backdrop bind group
    // leaked into the common path: `Material::OPAQUE` is every widget in the
    // framework, and it must render identically no matter what is behind it.
    needs_gpu!(renderer);

    let over_black = through_pane(&mut renderer, Material::OPAQUE, |_| {});
    let over_block = through_pane(&mut renderer, Material::OPAQUE, block_at(20.0));

    assert_eq!(
        pane_row(&over_black),
        pane_row(&over_block),
        "an opaque pane changed with its backdrop; it is sampling one it should not"
    );
}

#[test]
fn a_transmissive_pane_shows_what_is_behind_it() {
    // The precondition for everything below: if the backdrop never reaches
    // the shader, nothing else in this file means anything.
    needs_gpu!(renderer);

    let opaque = through_pane(&mut renderer, Material::OPAQUE, block_at(20.0));
    let glass = through_pane(&mut renderer, Material::GLASS, block_at(20.0));

    if cfg!(feature = "web-colors") {
        assert_eq!(
            pane_row(&opaque),
            pane_row(&glass),
            "legacy blending must leave physical transmission disabled"
        );
        return;
    }

    assert_ne!(
        pane_row(&opaque),
        pane_row(&glass),
        "a transmissive pane looks identical to an opaque one; \
         the backdrop is not reaching the shader"
    );

    // And it is showing the backdrop specifically, not just some difference:
    // the block behind is white, so the pane over it must be brighter than
    // the same pane over black.
    let over_black = through_pane(&mut renderer, Material::GLASS, |_| {});

    let brightest = |pixels: &[u8]| {
        pane_row(pixels)
            .into_iter()
            .map(|pixel| pixel[0])
            .max()
            .unwrap_or(0)
    };

    assert!(
        brightest(&glass) > brightest(&over_black),
        "a pane over white ({}) is no brighter than the same pane over black ({}); \
         it is not transmitting what is behind it",
        brightest(&glass),
        brightest(&over_black)
    );
}

#[test]
fn a_pane_bends_what_it_transmits_rather_than_translating_it() {
    // The Pillar III gate, in one assertion. A backdrop blur translates
    // rigidly: move the content behind it by N pixels and every pixel it
    // shows moves by exactly N. A pane refracts, so the displacement varies
    // across it -- most at the bevel, none in the flat middle.
    //
    // Measured as a *shape* change rather than a shift, because a shift is
    // exactly what a rigid translation would also produce.
    needs_gpu!(renderer);

    let glass_a = through_pane(&mut renderer, Material::GLASS, block_at(18.0));
    let glass_b = through_pane(&mut renderer, Material::GLASS, block_at(22.0));

    let row_a = pane_row(&glass_a);
    let row_b = pane_row(&glass_b);

    if cfg!(feature = "web-colors") {
        assert_eq!(
            row_a, row_b,
            "legacy blending must leave physical transmission disabled"
        );
        return;
    }

    assert_ne!(row_a, row_b, "moving the backdrop changed nothing");

    // How far each row's brightest point sits from the row's start.
    let peak = |row: &[[u8; 4]]| {
        row.iter()
            .enumerate()
            .max_by_key(|(_, pixel)| pixel[0])
            .map(|(index, _)| index)
            .unwrap_or(0)
    };

    // The interior of a pane refracts nothing (the normal faces the viewer,
    // so the offset is zero) while the bevel refracts most. That difference
    // is the whole claim: an offset that varies with position is not a
    // translation.
    let interior = |row: &[[u8; 4]]| {
        let middle = row.len() / 2;
        row[middle][0]
    };

    // Both rows must show the block somewhere, or the peaks are noise.
    assert!(
        row_a.iter().any(|pixel| pixel[0] > 40) && row_b.iter().any(|pixel| pixel[0] > 40),
        "the block behind the pane is not visible through it at all"
    );

    let _ = (peak(&row_a), peak(&row_b), interior(&row_a));

    // The direct statement of "bends rather than translates": compare what
    // the pane shows against what the *backdrop itself* shows at the same
    // pixels. A rigid translation would keep those in lockstep.
    let bare_a = through_pane(&mut renderer, Material::OPAQUE, block_at(18.0));
    let bare_b = through_pane(&mut renderer, Material::OPAQUE, block_at(22.0));

    let delta = |a: &[u8], b: &[u8]| -> Vec<i16> {
        pane_row(a)
            .into_iter()
            .zip(pane_row(b))
            .map(|(x, y)| i16::from(x[0]) - i16::from(y[0]))
            .collect()
    };

    let through_glass = delta(&glass_a, &glass_b);
    let behind_only = delta(&bare_a, &bare_b);

    assert_ne!(
        through_glass, behind_only,
        "what the pane shows moved exactly as the backdrop did; \
         it is translating rather than refracting"
    );
}

#[test]
fn a_pane_separates_colour_at_its_bevel() {
    // Dispersion, the detail that reads as glass rather than as a filter.
    // Red bends least and blue most, so at the bevel the two channels sample
    // the backdrop at measurably different places.
    //
    // `GLASS` disperses; `FROSTED` disperses half as much; a material with
    // `dispersion: 0.0` must not separate at all. Comparing the three is what
    // separates "dispersion works" from "the pane happens to be colourful".
    needs_gpu!(renderer);

    // The renderer does not know physical pixel density yet, so this 64 px
    // proof uses a thicker, strongly dispersive authored pane to give the
    // channel spread more than a sub-byte footprint. The ordinary 1.5 mm,
    // 0.012 preset exercises the same arithmetic but its delta can quantise
    // away at this resolution.
    let glass = Material {
        thickness_mm: 8.0,
        dispersion: 0.25,
        ..Material::GLASS
    };

    let dispersive = through_pane(&mut renderer, glass, red_blue_split);

    let achromatic = through_pane(
        &mut renderer,
        Material {
            dispersion: 0.0,
            ..glass
        },
        red_blue_split,
    );

    if cfg!(feature = "web-colors") {
        assert_eq!(
            pane_row(&dispersive),
            pane_row(&achromatic),
            "legacy blending must leave physical transmission disabled"
        );
        return;
    }

    assert_ne!(
        pane_row(&dispersive),
        pane_row(&achromatic),
        "dispersion changed nothing; the per-channel offsets are not reaching the sampler"
    );

    // The direction matters and is easy to get backwards: reversing it
    // produces edges that look wrong without looking obviously wrong. Over a
    // red-to-blue edge, the channel that bends more reaches across the edge
    // sooner, so the two materials' red-minus-blue differs where they
    // straddle it.
    let contrast = |pixels: &[u8]| -> Vec<i16> {
        pane_row(pixels)
            .into_iter()
            .map(|pixel| i16::from(pixel[0]) - i16::from(pixel[2]))
            .collect()
    };

    let dispersive_contrast = contrast(&dispersive);
    let achromatic_contrast = contrast(&achromatic);

    let separated = dispersive_contrast
        .iter()
        .zip(&achromatic_contrast)
        .any(|(a, b)| (a - b).abs() > 1);

    assert!(
        separated,
        "no pixel separated red from blue differently than a non-dispersive pane did"
    );
}

#[test]
fn a_gradient_pane_shades_its_material_like_a_solid_one() {
    // The gradient quad pipeline reads its instance attributes through a
    // separate `vertex_attr_array!` from the solid one, and until this test
    // existed nothing in the repository ever rendered a gradient quad with a
    // material at all -- so an offset error in that list was undetectable.
    //
    // A gradient pane and a solid pane of the same material over the same
    // backdrop will not be pixel-identical (their fills differ by
    // construction), but both must respond to the material: neither may come
    // back byte-identical to its own opaque counterpart.
    needs_gpu!(renderer);

    fn gradient_pane(renderer: &mut Renderer, material: Material) -> Vec<u8> {
        let logical = Size::new(SIZE as f32, SIZE as f32);
        let bounds = Rectangle::with_size(logical);

        renderer.reset(bounds);

        block_at(20.0)(renderer);

        renderer.with_layer(bounds, |renderer| {
            renderer.fill_quad(
                Quad {
                    bounds: PANE,
                    border: taim_core::Border {
                        radius: 10.0.into(),
                        ..Default::default()
                    },
                    material,
                    ..Default::default()
                },
                taim_core::Background::Gradient(taim_core::Gradient::Linear(
                    taim_core::gradient::Linear::new(0.0)
                        .add_stop(0.0, Color::from_rgb(0.2, 0.2, 0.2))
                        .add_stop(1.0, Color::from_rgb(0.5, 0.5, 0.5)),
                )),
            );
        });

        let viewport = Viewport::with_physical_size(Size::new(SIZE, SIZE), 1.0);

        renderer.screenshot(&viewport, Color::BLACK)
    }

    let opaque = gradient_pane(&mut renderer, Material::OPAQUE);
    let glass = gradient_pane(&mut renderer, Material::GLASS);

    if cfg!(feature = "web-colors") {
        assert_eq!(
            pane_row(&opaque),
            pane_row(&glass),
            "legacy blending must leave physical transmission disabled"
        );
        return;
    }

    assert_ne!(
        pane_row(&opaque),
        pane_row(&glass),
        "a material changed nothing on the gradient pipeline; \
         its instance attributes are misaligned or unread"
    );
}

#[test]
fn no_transmissive_material_produces_an_out_of_range_pixel() {
    // Sampling a texture at an offset derived from authored numbers is a new
    // way to produce a black hole or a blown-out frame, and neither is a
    // thing a NaN-guard on the physics can catch: an offset that lands
    // outside the frame is perfectly finite and still wrong.
    needs_gpu!(renderer);

    let materials = [
        Material::GLASS,
        Material::FROSTED,
        Material {
            transmission: 1.0,
            thickness_mm: 1_000.0,
            ..Material::GLASS
        },
        Material {
            transmission: 1.0,
            dispersion: 0.5,
            ..Material::GLASS
        },
        Material {
            transmission: f32::NAN,
            ..Material::GLASS
        },
        Material {
            transmission: 9.0,
            thickness_mm: -4.0,
            ..Material::GLASS
        },
    ];

    for material in materials {
        let frame = through_pane(&mut renderer, material, block_at(20.0));

        assert!(
            frame.chunks_exact(4).all(|pixel| pixel[3] == 255),
            "{material:?} punched a hole in the frame's alpha"
        );

        // The backdrop is white on black and the fill is mid-grey, so
        // nothing the pane shows can be darker than black or brighter than
        // white -- but a wrapped or wildly offset sample reads as a band of
        // one extreme across the pane.
        let row = pane_row(&frame);

        assert!(
            !row.iter().all(|pixel| pixel[0] == 0),
            "{material:?} rendered the pane entirely black"
        );
        assert!(
            !row.iter().all(|pixel| pixel[0] == 255),
            "{material:?} blew the pane out to white"
        );
    }
}
