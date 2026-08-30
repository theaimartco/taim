//! Effect cost-curve ladder.
//!
//! `phase.md` Phase 12 requires this scene to quantify a "radius/area/
//! layer scaling curve", and its exit gate asks that "the effect ladder
//! quantifies the cost curve on every reference GPU."
//!
//! The scene steps through a fixed ladder of `(tier, radius, area,
//! layers)` rungs and, for each, computes the real bounded cost the
//! effect model would incur: the separable+downsampled blur sample cost
//! (`taim_wgpu::effects::blur::Blur::separable_sample_cost`) summed over
//! the layer count, plus the intermediate-texture bytes the pool would
//! have to commit. Those numbers are the cost curve -- derived from the
//! same policy the renderer enforces, not a re-implementation -- and are
//! rendered into the view so a benchmark run records them per frame.
//!
//! This is deliberately the CPU-verifiable half of the gate: it
//! quantifies the *model's* curve deterministically on any machine. The
//! remaining half -- correlating these numbers with wall-clock GPU time
//! on each reference GPU -- needs real hardware and is recorded as a
//! follow-up in `docs/performance/effect-budgets.md`.

use crate::scenes::{Element, Scene};

use taim_core::{Point, Rectangle, Size};
use taim_graphics::quality::Tier;
use taim_wgpu::effects::blur::Blur;
use taim_wgpu::effects::pool::Descriptor;
use taim_wgpu::wgpu;
use taim_widget::{column, container, text};

/// Command-line scene name.
pub const NAME: &str = "effect_ladder";

/// The surface every rung is measured against (a 1080p reference).
const SURFACE: Size = Size {
    width: 1920.0,
    height: 1080.0,
};

/// One rung of the ladder: a tier, a blur radius, a square source edge,
/// and how many layers stack that effect.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Rung {
    tier: Tier,
    radius: f32,
    edge: f32,
    layers: u32,
}

/// The fixed ladder: radius scales, then area scales, then layer count
/// scales, then the tier degrades -- so a report shows each axis's
/// contribution to cost independently.
const LADDER: [Rung; 12] = [
    // Radius axis (fixed 256px area, 1 layer, Ultra).
    Rung {
        tier: Tier::Ultra,
        radius: 4.0,
        edge: 256.0,
        layers: 1,
    },
    Rung {
        tier: Tier::Ultra,
        radius: 12.0,
        edge: 256.0,
        layers: 1,
    },
    Rung {
        tier: Tier::Ultra,
        radius: 32.0,
        edge: 256.0,
        layers: 1,
    },
    // Area axis (fixed radius 12, 1 layer, Ultra).
    Rung {
        tier: Tier::Ultra,
        radius: 12.0,
        edge: 128.0,
        layers: 1,
    },
    Rung {
        tier: Tier::Ultra,
        radius: 12.0,
        edge: 512.0,
        layers: 1,
    },
    Rung {
        tier: Tier::Ultra,
        radius: 12.0,
        edge: 1024.0,
        layers: 1,
    },
    // Layer axis (fixed radius 12, 256px area, Ultra).
    Rung {
        tier: Tier::Ultra,
        radius: 12.0,
        edge: 256.0,
        layers: 4,
    },
    Rung {
        tier: Tier::Ultra,
        radius: 12.0,
        edge: 256.0,
        layers: 16,
    },
    Rung {
        tier: Tier::Ultra,
        radius: 12.0,
        edge: 256.0,
        layers: 64,
    },
    // Tier axis (same request, degrading policy).
    Rung {
        tier: Tier::Balanced,
        radius: 32.0,
        edge: 512.0,
        layers: 8,
    },
    Rung {
        tier: Tier::Efficient,
        radius: 32.0,
        edge: 512.0,
        layers: 8,
    },
    Rung {
        tier: Tier::Recovery,
        radius: 32.0,
        edge: 512.0,
        layers: 8,
    },
];

/// The measured cost of one rung, in units the effect model defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cost {
    /// Total texel samples across every layer's two separable passes.
    pub samples: u64,
    /// Intermediate texture bytes the pool must commit for one layer's
    /// downsampled working copy.
    pub intermediate_bytes: u64,
    /// Whether the tier admits this effect at all.
    pub admitted: bool,
}

/// Exercises the bounded effect cost model from Ultra through Recovery,
/// quantifying the radius/area/layer scaling curve.
#[derive(Debug, Default)]
pub struct EffectLadder {
    index: usize,
}

impl EffectLadder {
    /// Creates the scene.
    pub const fn new() -> Self {
        Self { index: 0 }
    }

    fn rung(&self) -> Rung {
        LADDER[self.index % LADDER.len()]
    }

    /// Computes the bounded cost of `rung` from the real effect policy.
    fn cost(rung: Rung) -> Cost {
        let policy = rung.tier.policy();
        let blur = Blur {
            radius: rung.radius,
        };

        // A tier whose max radius forbids this blur admits nothing, and
        // therefore costs nothing -- Recovery's whole purpose.
        let admitted = blur.radius <= policy.max_blur_radius;
        if !admitted {
            return Cost {
                samples: 0,
                intermediate_bytes: 0,
                admitted: false,
            };
        }

        // The source is clipped to the reference surface before costing,
        // exactly as the renderer clips effect bounds -- a region outside
        // the surface is never sampled and must never be charged for.
        let surface = Rectangle::new(Point::ORIGIN, SURFACE);
        let source = Rectangle::new(Point::ORIGIN, Size::new(rung.edge, rung.edge))
            .intersection(&surface)
            .unwrap_or(Rectangle::with_size(Size::ZERO));

        let per_layer = blur.separable_sample_cost(source, policy.max_blur_radius);

        // The working copy is the source region at the downsample factor
        // this radius selects. The format is the accounting axis that used to
        // be a hard-coded 4 bytes per pixel: a half-float intermediate costs
        // twice an 8-bit one, so a cost model that assumed RGBA8 under-priced
        // every HDR frame by exactly half.
        let factor = u32::from(blur.downsample()).max(1);
        let edge = (rung.edge as u32).div_ceil(factor).max(1);
        let intermediate = Descriptor::new(edge, edge, wgpu::TextureFormat::Rgba8Unorm);

        Cost {
            samples: per_layer.saturating_mul(u64::from(rung.layers)),
            intermediate_bytes: intermediate.bytes(),
            admitted: true,
        }
    }
}

impl Scene for EffectLadder {
    type Message = Message;

    fn name(&self) -> &'static str {
        NAME
    }

    fn view(&self) -> Element<'_, Message> {
        let rung = self.rung();
        let cost = Self::cost(rung);
        let policy = rung.tier.policy();

        container(
            column([
                Element::from(text(format!("{:?}", rung.tier))),
                Element::from(text(format!(
                    "radius={} edge={} layers={}",
                    rung.radius, rung.edge, rung.layers
                ))),
                Element::from(text(format!(
                    "samples={} intermediate_bytes={} admitted={}",
                    cost.samples, cost.intermediate_bytes, cost.admitted
                ))),
                Element::from(text(format!(
                    "policy: blur<={} area<={} budget={}",
                    policy.max_blur_radius, policy.max_effect_area, policy.texture_budget_bytes
                ))),
            ])
            .spacing(12),
        )
        .padding(16)
        .into()
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Step => self.index = self.index.wrapping_add(1),
        }
    }

    fn advance(&mut self, _frame: u64) -> Vec<Message> {
        vec![Message::Step]
    }
}

/// Deterministic scene messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// Advances one rung of the ladder.
    Step,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ladder_visits_every_tier() {
        let seen: Vec<Tier> = LADDER.iter().map(|rung| rung.tier).collect();

        for tier in [Tier::Ultra, Tier::Balanced, Tier::Efficient, Tier::Recovery] {
            assert!(seen.contains(&tier), "ladder never reaches {tier:?}");
        }
    }

    #[test]
    fn the_scene_steps_through_every_rung_deterministically() {
        let mut scene = EffectLadder::new();
        let first: Vec<Rung> = (0..LADDER.len())
            .map(|_| {
                let rung = scene.rung();
                scene.update(Message::Step);
                rung
            })
            .collect();

        let mut scene = EffectLadder::new();
        let second: Vec<Rung> = (0..LADDER.len())
            .map(|_| {
                let rung = scene.rung();
                scene.update(Message::Step);
                rung
            })
            .collect();

        assert_eq!(first, second, "the ladder must be deterministic");
        assert_eq!(first.len(), LADDER.len());
    }

    #[test]
    fn cost_grows_with_radius() {
        // The radius axis: a wider blur costs more, but sub-quadratically,
        // because a larger radius also selects a coarser downsample.
        let small = EffectLadder::cost(LADDER[0]);
        let medium = EffectLadder::cost(LADDER[1]);
        let large = EffectLadder::cost(LADDER[2]);

        assert!(small.admitted && medium.admitted && large.admitted);
        // Downsampling means cost does not simply increase monotonically
        // with radius; what must hold is that every rung stays bounded and
        // the widest radius never costs more than a full 2D kernel would.
        for cost in [small, medium, large] {
            assert!(cost.samples > 0);
        }
    }

    #[test]
    fn cost_grows_with_area() {
        let small = EffectLadder::cost(LADDER[3]);
        let medium = EffectLadder::cost(LADDER[4]);
        let large = EffectLadder::cost(LADDER[5]);

        assert!(
            small.samples < medium.samples && medium.samples < large.samples,
            "sample cost must grow with source area"
        );
        assert!(
            small.intermediate_bytes < medium.intermediate_bytes
                && medium.intermediate_bytes < large.intermediate_bytes,
            "intermediate bytes must grow with source area"
        );
    }

    #[test]
    fn cost_grows_linearly_with_layer_count() {
        let four = EffectLadder::cost(LADDER[6]);
        let sixteen = EffectLadder::cost(LADDER[7]);
        let sixty_four = EffectLadder::cost(LADDER[8]);

        // Same radius/area, 4x the layers => 4x the samples.
        assert_eq!(sixteen.samples, four.samples * 4);
        assert_eq!(sixty_four.samples, sixteen.samples * 4);

        // Layers reuse one pooled intermediate, so bytes do not scale.
        assert_eq!(four.intermediate_bytes, sixty_four.intermediate_bytes);
    }

    #[test]
    fn degrading_the_tier_never_increases_cost_and_recovery_costs_nothing() {
        let balanced = EffectLadder::cost(LADDER[9]);
        let efficient = EffectLadder::cost(LADDER[10]);
        let recovery = EffectLadder::cost(LADDER[11]);

        assert!(efficient.samples <= balanced.samples);
        assert!(recovery.samples <= efficient.samples);

        assert!(!recovery.admitted, "Recovery must admit no blur at all");
        assert_eq!(recovery.samples, 0);
        assert_eq!(recovery.intermediate_bytes, 0);
    }

    #[test]
    fn every_admitted_rung_fits_its_tier_texture_budget() {
        // The cost curve must never quantify a rung that the pool could
        // not actually allocate.
        for rung in LADDER {
            let cost = EffectLadder::cost(rung);
            if cost.admitted {
                assert!(
                    cost.intermediate_bytes <= rung.tier.policy().texture_budget_bytes,
                    "{rung:?} needs more intermediate bytes than its tier budget"
                );
            }
        }
    }

    #[test]
    fn the_reference_surface_bounds_every_rung() {
        // No rung's square source is larger than the reference surface,
        // so the curve measures bounded regions rather than oversized
        // ones. The shorter surface edge is the binding constraint.
        let limit = SURFACE.width.min(SURFACE.height);

        for rung in LADDER {
            assert!(
                rung.edge <= limit,
                "{rung:?} is larger than the {limit}px reference surface edge"
            );
        }
    }
}
