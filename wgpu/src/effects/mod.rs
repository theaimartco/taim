//! Bounded GPU effect graph contracts.
pub mod backdrop_blur;
pub mod blur;
pub mod color_matrix;
pub mod mask;
pub mod noise;
pub mod pipeline;
pub mod pool;
pub mod saturation;
pub mod shadow;

use crate::core::Rectangle;
use crate::graphics::quality::Tier;

/// An effect node with explicit sampling bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Effect {
    /// Shadow sampling outside source bounds.
    Shadow(shadow::Shadow),
    /// Foreground blur.
    Blur(blur::Blur),
    /// Backdrop capture blur.
    BackdropBlur(backdrop_blur::BackdropBlur),
    /// Color transform.
    ColorMatrix(color_matrix::ColorMatrix),
    /// Saturation transform.
    Saturation(saturation::Saturation),
    /// Optional material noise.
    Noise(noise::Noise),
    /// Clip/mask operation.
    Mask(mask::Mask),
}

impl Effect {
    /// Returns the damage/sampling bounds for this effect, clipped to `surface`.
    pub fn bounds(self, source: Rectangle, surface: Rectangle, tier: Tier) -> Option<Rectangle> {
        let policy = tier.policy();
        let bounds = match self {
            Self::Shadow(effect) => effect.bounds(source),
            Self::Blur(effect) => effect.bounds(source, policy.max_blur_radius),
            Self::BackdropBlur(effect) => effect.bounds(source, policy),
            Self::ColorMatrix(_) | Self::Saturation(_) | Self::Noise(_) | Self::Mask(_) => source,
        };
        bounds.intersection(&surface)
    }

    /// Returns whether this effect is allowed at `tier`.
    ///
    /// Expensive, optional effects (backdrop blur, noise) are gated by
    /// the tier's capability flags; blur is gated by radius against the
    /// tier's `max_blur_radius` (so Recovery, whose limit is `0.0`,
    /// forbids any positive-radius blur outright rather than silently
    /// clamping it to nothing). Cheap, content-defining effects
    /// (color matrix, saturation, mask) are always allowed -- they never
    /// carry the runaway cost this gate exists to bound.
    pub fn allowed(self, tier: Tier) -> bool {
        let policy = tier.policy();
        match self {
            Self::Blur(effect) => effect.radius <= policy.max_blur_radius,
            Self::BackdropBlur(effect) => {
                policy.backdrop_blur && effect.area <= policy.max_effect_area
            }
            Self::Noise(_) => policy.noise,
            _ => true,
        }
    }

    /// Returns the clipped sampling area (in logical pixels squared) this
    /// effect would touch at `tier`, or `0.0` if it is disallowed or its
    /// bounds fall entirely outside `surface`.
    ///
    /// This is what the graph sums to keep total effect work within the
    /// tier's `max_effect_area` budget, and is the quantity a "no
    /// unbounded allocation" check is written against.
    pub fn sampling_area(self, source: Rectangle, surface: Rectangle, tier: Tier) -> f32 {
        if !self.allowed(tier) {
            return 0.0;
        }
        self.bounds(source, surface, tier)
            .map(|region| region.width.max(0.0) * region.height.max(0.0))
            .unwrap_or(0.0)
    }
}

/// A validated, budget-bounded effect sequence for a layer.
#[derive(Debug, Clone, Default)]
pub struct Graph {
    effects: Vec<Effect>,
}

impl Graph {
    /// Creates an empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an effect if it is allowed at the active `tier`. Returns
    /// whether it was accepted.
    pub fn push(&mut self, effect: Effect, tier: Tier) -> bool {
        if effect.allowed(tier) {
            self.effects.push(effect);
            true
        } else {
            false
        }
    }

    /// Adds an effect only if it is allowed *and* keeps the graph's total
    /// sampling area within the tier's `max_effect_area` budget over
    /// `source`/`surface`. Returns whether it was accepted -- the
    /// concrete "no effect can trigger unbounded texture allocation"
    /// gate at the graph level.
    pub fn push_within_budget(
        &mut self,
        effect: Effect,
        source: Rectangle,
        surface: Rectangle,
        tier: Tier,
    ) -> bool {
        if !effect.allowed(tier) {
            return false;
        }

        let projected = self.total_sampling_area(source, surface, tier)
            + effect.sampling_area(source, surface, tier);

        if projected > tier.policy().max_effect_area {
            return false;
        }

        self.effects.push(effect);
        true
    }

    /// Returns the total clipped sampling area of every effect in the
    /// graph at `tier`.
    pub fn total_sampling_area(
        self: &Graph,
        source: Rectangle,
        surface: Rectangle,
        tier: Tier,
    ) -> f32 {
        self.effects
            .iter()
            .map(|effect| effect.sampling_area(source, surface, tier))
            .sum()
    }

    /// Returns graph effects in execution order.
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }

    /// Returns whether the graph holds no effects.
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }
}
