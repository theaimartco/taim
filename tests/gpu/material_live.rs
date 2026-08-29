//! Materials on the live render path.
//!
//! `tests/gpu/material_shading.rs` proves the physics agrees between CPU and
//! GPU. That is necessary and not sufficient: correct arithmetic nothing calls
//! is the failure mode this repository has hit four times
//! (`RetainedLayerCache`, `Tree::layout_with_cache`, `effects::pool::Pool`,
//! `PipelineCache`).
//!
//! These render real quads through the real renderer and read the pixels back,
//! so they fail if the material never reaches the shader — which no unit test
//! of the physics can detect.
//!
//! Skipped rather than failed when no adapter is available.

use aura_core::material::Material;
use aura_core::renderer::Quad;
use aura_core::{Color, Rectangle, Renderer as _, Size};
use aura_wgpu::Renderer;
use aura_wgpu::graphics::Viewport;

const SIZE: u32 = 64;

fn renderer() -> Option<Renderer> {
    aura_futures::futures::executor::block_on(<Renderer as aura_core::renderer::Headless>::new(
        aura_core::renderer::Settings::default(),
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

/// Draws one mid-grey rounded rectangle of `material` and returns the frame.
fn card(renderer: &mut Renderer, material: Material) -> Vec<u8> {
    let logical = Size::new(SIZE as f32, SIZE as f32);
    renderer.reset(Rectangle::with_size(logical));

    renderer.fill_quad(
        Quad {
            bounds: Rectangle {
                x: 8.0,
                y: 8.0,
                width: 48.0,
                height: 48.0,
            },
            border: aura_core::Border {
                radius: 12.0.into(),
                ..Default::default()
            },
            material,
            ..Default::default()
        },
        Color::from_rgb(0.35, 0.35, 0.35),
    );

    let viewport = Viewport::with_physical_size(Size::new(SIZE, SIZE), 1.0);

    renderer.screenshot(&viewport, Color::BLACK)
}

/// The red channel at a pixel.
fn at(pixels: &[u8], x: u32, y: u32) -> u8 {
    pixels[((y * SIZE + x) * 4) as usize]
}

/// The brightest red channel anywhere in the frame.
fn brightest(pixels: &[u8]) -> u8 {
    pixels
        .chunks_exact(4)
        .map(|pixel| pixel[0])
        .max()
        .unwrap_or(0)
}

#[test]
fn an_opaque_card_is_shaded_exactly_as_it_was_before_materials_existed() {
    // The safety property. `Material::OPAQUE` is the default, so every widget
    // in the framework takes this path, and it must be byte-identical to a
    // build with no material support at all.
    needs_gpu!(renderer);

    let default = card(&mut renderer, Material::default());
    let opaque = card(&mut renderer, Material::OPAQUE);

    assert_eq!(
        default, opaque,
        "the default material is not `OPAQUE`, so widgets changed appearance"
    );

    // An opaque surface bends no light, so nothing anywhere may exceed the
    // fill it was drawn with.
    let fill = at(&opaque, 32, 32);

    assert!(
        brightest(&opaque) <= fill.saturating_add(2),
        "an opaque card has a highlight: brightest {} against fill {fill}",
        brightest(&opaque)
    );
}

#[test]
fn a_material_reaches_the_shader_and_lights_the_bevel() {
    // The test that fails if the material is never plumbed through: identical
    // geometry, identical colour, different material, and the pixels must
    // differ. Nothing in the physics unit tests can catch a missing vertex
    // attribute or a shader that ignores it.
    needs_gpu!(renderer);

    let flat = card(&mut renderer, Material::OPAQUE);
    let glass = card(&mut renderer, Material::GLASS);

    assert_ne!(
        flat, glass,
        "a material changed nothing; it is not reaching the shader"
    );

    assert!(
        brightest(&glass) > brightest(&flat),
        "glass ({}) is no brighter anywhere than an opaque card ({})",
        brightest(&glass),
        brightest(&flat)
    );
}

#[test]
fn the_highlight_is_on_the_edge_and_not_the_middle() {
    // A bevel highlight belongs to the edge. If the interior brightened too,
    // the SDF normal is wrong — most likely facing the viewer everywhere,
    // which is what a constant normal produces.
    needs_gpu!(renderer);

    let flat = card(&mut renderer, Material::OPAQUE);
    let glass = card(&mut renderer, Material::GLASS);

    let centre_change = i16::from(at(&glass, 32, 32)) - i16::from(at(&flat, 32, 32));

    assert!(
        centre_change.abs() <= 3,
        "the card's interior changed by {centre_change}; the bevel is not confined to the edge"
    );

    // Somewhere on the card, the edge did change.
    let edge_changed = (0..SIZE)
        .any(|x| (0..SIZE).any(|y| i16::from(at(&glass, x, y)) - i16::from(at(&flat, x, y)) > 3));

    assert!(edge_changed, "no pixel anywhere caught a highlight");
}

#[test]
fn a_rougher_material_spreads_its_highlight_more_softly() {
    // The physical difference between glass and frosted glass: the same
    // energy over a wider lobe, so the peak is lower. If roughness were not
    // reaching the shader these would be identical.
    needs_gpu!(renderer);

    let glass = card(&mut renderer, Material::GLASS);
    let frosted = card(&mut renderer, Material::FROSTED);

    assert_ne!(glass, frosted, "roughness is not reaching the shader");

    assert!(
        brightest(&glass) >= brightest(&frosted),
        "frosted ({}) peaks higher than clear glass ({}), so the GGX lobe is inverted",
        brightest(&frosted),
        brightest(&glass)
    );
}

#[test]
fn no_material_produces_an_out_of_range_pixel() {
    // A NaN or an infinity in the lighting term arrives as an arbitrary byte.
    // What is checkable is that a mid-grey card never clips to black or white
    // anywhere, which an exploded specular term would cause.
    needs_gpu!(renderer);

    for material in [
        Material::OPAQUE,
        Material::GLASS,
        Material::FROSTED,
        Material::POLISHED,
        // Deliberately out of range: authored materials arrive like this.
        Material {
            roughness: -1.0,
            ior: 0.1,
            thickness_mm: f32::NAN,
            ..Material::GLASS
        },
    ] {
        let pixels = card(&mut renderer, material);
        let peak = brightest(&pixels);

        assert!(
            peak < 255,
            "{material:?} blew out to {peak}; the specular term exploded"
        );
        assert!(peak > 0, "{material:?} produced a black frame");
    }
}

#[test]
fn the_scene_light_is_configurable_and_moves_the_highlight() {
    // Proof that the light is plumbed rather than a constant in disguise.
    // It reached the shader through what used to be uniform padding, so this
    // fails if the renderer's light is ignored — which no physics test can
    // detect.
    needs_gpu!(renderer);

    let from_upper_left = card(&mut renderer, Material::GLASS);

    // Mirror the light across the vertical axis: the highlight must move to
    // the other edge.
    let key = aura_core::material::SceneLight::KEY;
    renderer.set_light(aura_core::material::SceneLight {
        direction: [-key.direction[0], key.direction[1], key.direction[2]],
        ..key
    });

    let from_upper_right = card(&mut renderer, Material::GLASS);

    assert_ne!(
        from_upper_left, from_upper_right,
        "moving the light changed nothing; it is not reaching the shader"
    );

    // The brightest column should move across the card. Compare which half
    // holds more light.
    let half_weight = |pixels: &[u8], left: bool| -> u64 {
        (0..SIZE)
            .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
            .filter(|(x, _)| if left { *x < SIZE / 2 } else { *x >= SIZE / 2 })
            .map(|(x, y)| u64::from(at(pixels, x, y)))
            .sum()
    };

    let original_bias =
        half_weight(&from_upper_left, true) as i64 - half_weight(&from_upper_left, false) as i64;
    let mirrored_bias =
        half_weight(&from_upper_right, true) as i64 - half_weight(&from_upper_right, false) as i64;

    assert!(
        original_bias.signum() != mirrored_bias.signum() || original_bias == 0,
        "mirroring the light did not move the highlight across the card \
         (bias {original_bias} then {mirrored_bias})"
    );

    // Leave the renderer as it was found.
    renderer.set_light(key);
}
