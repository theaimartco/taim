#![allow(missing_docs)]

use super::{ComponentSpec, Density, InputMethods, StateSupport};
use crate::tokens::Radius;

use aura_core::Size;
use aura_core::semantics::{Actions, Role};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slider {
    pub value: f32,
    pub range: (f32, f32),
    /// Whether the track runs right-to-left. A slider is the one control
    /// whose *direction* must mirror, not merely its layout: in RTL the
    /// maximum sits at the left.
    pub rtl: bool,
    pub disabled: bool,
}

impl Slider {
    /// Returns the normalized track position of [`Self::value`] within
    /// [`Self::range`], already mirrored for [`Self::rtl`], in
    /// `0.0..=1.0`.
    ///
    /// A degenerate range (min == max) has no meaningful position and
    /// reports the start of the track. Values outside the range clamp
    /// rather than running off the track.
    pub fn normalized(self) -> f32 {
        let min = self.range.0.min(self.range.1);
        let max = self.range.0.max(self.range.1);
        let span = max - min;

        if span <= 0.0 {
            return 0.0;
        }

        let fraction = ((self.value - min) / span).clamp(0.0, 1.0);

        if self.rtl { 1.0 - fraction } else { fraction }
    }
}

impl ComponentSpec for Slider {
    fn role(&self) -> Role {
        Role::Slider
    }

    fn actions(&self) -> Actions {
        if self.disabled {
            Actions::empty()
        } else {
            // A slider is set directly (drag/touch) *and* stepped (arrow
            // keys, gamepad d-pad) -- both must be offered, or the control
            // is unusable without a pointer.
            Actions::FOCUS | Actions::SET_VALUE | Actions::INCREMENT | Actions::DECREMENT
        }
    }

    fn min_size(&self, density: Density) -> Size {
        let height = match density {
            Density::Compact => 48.0,
            Density::Comfortable => 52.0,
            Density::Spacious => 56.0,
        };

        // Wide enough that a drag has usable precision, tall enough to be
        // a legal touch target.
        Size::new(144.0, height)
    }

    fn supported_states(&self) -> StateSupport {
        StateSupport::INTERACTIVE
    }

    fn input_methods(&self) -> InputMethods {
        InputMethods::ALL
    }

    fn radius(&self) -> Radius {
        Radius::Full
    }
}
