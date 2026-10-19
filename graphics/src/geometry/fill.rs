//! Fill [`Geometry`] with a certain style.
//!
//! [`Geometry`]: super::Renderer::Geometry
pub use crate::geometry::Style;

use crate::core::Color;
use crate::gradient::{self, Gradient};

/// The style used to fill geometry.
#[derive(Debug, Clone, Copy)]
pub struct Fill {
    /// The color or gradient of the fill.
    ///
    /// By default, it is set to [`Style::Solid`] with [`Color::BLACK`].
    pub style: Style,

    /// The fill rule defines how to determine what is inside and what is
    /// outside of a shape.
    ///
    /// See the [SVG specification][1] for more details.
    ///
    /// By default, it is set to `NonZero`.
    ///
    /// [1]: https://www.w3.org/TR/SVG/painting.html#FillRuleProperty
    pub rule: Rule,
}

impl Fill {
    /// Tell the geometry recorder whether this operation can be omitted.
    /// Winding rules do not affect alpha contribution.
    #[inline]
    pub fn is_fully_transparent(self) -> bool {
        self.style.is_fully_transparent()
    }

    /// Preserve the pigment while choosing even-odd/nonzero winding.
    pub fn with_rule(mut self, rule: Rule) -> Self {
        self.rule = rule;
        self
    }
}

impl Default for Fill {
    fn default() -> Self {
        Self {
            style: Style::Solid(Color::BLACK),
            rule: Rule::NonZero,
        }
    }
}

impl From<Color> for Fill {
    fn from(color: Color) -> Fill {
        Fill {
            style: Style::Solid(color),
            ..Fill::default()
        }
    }
}

impl From<Gradient> for Fill {
    fn from(gradient: Gradient) -> Self {
        Fill {
            style: Style::Gradient(gradient),
            ..Default::default()
        }
    }
}

impl From<gradient::Linear> for Fill {
    fn from(gradient: gradient::Linear) -> Self {
        Fill {
            style: Style::Gradient(Gradient::Linear(gradient)),
            ..Default::default()
        }
    }
}

/// The fill rule defines how to determine what is inside and what is outside of
/// a shape.
///
/// See the [SVG specification][1].
///
/// [1]: https://www.w3.org/TR/SVG/painting.html#FillRuleProperty
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Rule {
    NonZero,
    EvenOdd,
}

#[cfg(test)]
mod taim_fill_tests {
    use super::*;

    #[test]
    fn winding_rule_does_not_change_culling_decision() {
        let empty = Fill::from(Color::TRANSPARENT);
        assert!(empty.is_fully_transparent());
        assert!(empty.with_rule(Rule::EvenOdd).is_fully_transparent());
        assert!(!Fill::default().is_fully_transparent());
        assert_eq!(empty.with_rule(Rule::EvenOdd).rule, Rule::EvenOdd);
    }
}
