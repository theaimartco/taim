//! Separable, downsampled blur cost model.
//!
//! A Gaussian blur of radius `r` is implemented as two separable passes
//! (horizontal then vertical) over a downsampled copy of the source
//! region -- never the full surface, and never a full 2D kernel. This
//! module owns the *cost model* that keeps that bounded (`phase.md`
//! Phase 12: "separable downsampled blur with radius/area limits",
//! "downsample blur passes", "never blur the full surface when a smaller
//! sampling region is correct"). The renderer owns the actual passes.

use crate::core::Rectangle;

/// A foreground blur of a given radius (in logical pixels).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blur {
    /// Blur radius in logical pixels.
    pub radius: f32,
}

impl Blur {
    /// Returns the integer downsample factor for this radius: a larger
    /// radius tolerates a coarser working resolution, which is what keeps
    /// a wide blur affordable. Always at least `1` (no upsampling).
    pub fn downsample(self) -> u8 {
        if self.radius > 16.0 {
            4
        } else if self.radius > 8.0 {
            2
        } else {
            1
        }
    }

    /// Returns the sampling/damage bounds for this blur: the source
    /// region expanded by the (clamped) radius on every side, so pixels
    /// within `radius` of the region contribute correctly.
    ///
    /// `max` is the tier's `max_blur_radius`; the effective radius is
    /// clamped to it, so a blur can never expand bounds without limit.
    pub fn bounds(self, source: Rectangle, max: f32) -> Rectangle {
        let pad = self.effective_radius(max).ceil();
        Rectangle {
            x: source.x - pad,
            y: source.y - pad,
            width: source.width + pad * 2.0,
            height: source.height + pad * 2.0,
        }
    }

    /// Returns the radius actually applied at a tier whose limit is
    /// `max`: the requested radius clamped to `[0, max]`.
    pub fn effective_radius(self, max: f32) -> f32 {
        self.radius.clamp(0.0, max)
    }

    /// Returns the number of texel-sample operations the two separable
    /// passes cost over `source` at this radius and tier limit,
    /// accounting for the downsample factor.
    ///
    /// This is the quantity that must stay bounded: a naive full-kernel
    /// blur costs `area * (2r+1)^2`; the separable+downsampled path here
    /// costs `(area / factor^2) * 2 * (2r+1)`, the ratio that makes wide
    /// blurs affordable. Callers use it to keep an effect within the
    /// tier's `max_effect_area`-derived budget.
    pub fn separable_sample_cost(self, source: Rectangle, max: f32) -> u64 {
        let factor = f32::from(self.downsample());
        let radius = self.effective_radius(max);

        let width = (source.width.max(0.0) / factor).ceil().max(1.0);
        let height = (source.height.max(0.0) / factor).ceil().max(1.0);
        let downsampled_area = width * height;

        // Two separable passes, each a 1D kernel of (2r + 1) taps.
        let taps_per_pass = 2.0 * radius + 1.0;
        (downsampled_area * 2.0 * taps_per_pass) as u64
    }

    /// Returns whether this blur's expanded bounds are strictly smaller
    /// than `surface` -- i.e. a bounded sampling region genuinely smaller
    /// than the whole surface is being used, honoring "never blur the
    /// full surface when a smaller sampling region is correct."
    ///
    /// Returns `false` when the source (plus its radius padding) already
    /// covers the whole surface, which is the legitimate case where a
    /// full-surface region *is* the correct one.
    pub fn is_bounded_below_surface(self, source: Rectangle, surface: Rectangle, max: f32) -> bool {
        let bounds = self.bounds(source, max);
        let clipped_area = bounds
            .intersection(&surface)
            .map(|region| region.width * region.height)
            .unwrap_or(0.0);
        let surface_area = surface.width * surface.height;

        clipped_area < surface_area
    }
}
