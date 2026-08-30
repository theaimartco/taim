//! Controlled effect radii/areas/layer counts.
//!
//! Real GPU blur/shadow effects do not exist yet -- they are Phase 12's
//! `wgpu/src/effects/blur.rs` and `shadow.rs`. This scene instead stands
//! in with a controlled number of overlapping, styled container "layers"
//! (each with a border and background), which is the closest thing to
//! "layer count" this codebase can stress today. Its `layer_count` and
//! `area` parameters are named to match what Phase 12's real blur scene
//! will need to sweep, so that scene can replace this one's body without
//! changing the scene's public shape or report fields.

use crate::scenes::{Element, Scene};

use taim_core::{Color, Length};
use taim_widget::{container, space, stack};

/// Command-line/report identifier for this scene.
pub const NAME: &str = "blur_layers";

/// Number of overlapping styled layers stacked in the scene.
const LAYER_COUNT: usize = 12;

/// `blur_layers` never changes between frames; it exists to measure
/// per-layer paint/composite cost at a fixed, controlled layer count, not
/// to exercise a churn pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {}

/// A fixed stack of overlapping styled containers.
#[derive(Debug, Default)]
pub struct BlurLayers;

impl Scene for BlurLayers {
    type Message = Message;

    fn name(&self) -> &'static str {
        NAME
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let mut layers = stack![];

        for index in 0..LAYER_COUNT {
            let inset = (index * 12) as f32;
            let hue = index as f32 / LAYER_COUNT as f32;

            let layer = container(space())
                .width(Length::Fixed(400.0 - inset))
                .height(Length::Fixed(300.0 - inset))
                .style(move |_theme| container::Style {
                    background: Some(Color::from_rgba(hue, 1.0 - hue, 0.5, 0.6).into()),
                    border: taim_core::border::rounded(8),
                    ..container::Style::default()
                });

            layers = layers.push(layer);
        }

        container(layers).padding(16).into()
    }

    fn update(&mut self, message: Self::Message) {
        match message {}
    }

    fn advance(&mut self, _frame_index: u64) -> Vec<Self::Message> {
        Vec::new()
    }
}
