//! TAIM pane composition contract for IDE, desktop and split-window scenes.
//!
//! The unchecked legacy construction path remains compatible, but native
//! consumers can validate resource budgets and all split ratios before a
//! pane tree is mounted or its state IDs are allocated.

use crate::pane_grid::Axis;

/// A rooted binary region partition in a TAIM desktop workspace.
#[derive(Debug, Clone)]
pub enum Configuration<T> {
    /// Split the current region along an axis.
    Split {
        /// Split orientation.
        axis: Axis,
        /// Fraction assigned to the first region, from 0 through 1.
        ratio: f32,
        /// Content or further subdivisions occupying the first region.
        a: Box<Configuration<T>>,
        /// Content or further subdivisions occupying the second region.
        b: Box<Configuration<T>>,
    },
    /// One terminal pane and its application state.
    Pane(T),
}

/// Structural characteristics of a checked TAIM pane tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutStats {
    /// Total number of splits and leaves.
    pub nodes: usize,
    /// Number of leaf panes consuming widget state.
    pub panes: usize,
    /// Largest number of splits between root and a pane.
    pub max_depth: usize,
}

/// Why a TAIM pane tree cannot safely be admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutError {
    /// A split ratio is NaN, infinite or outside the closed unit interval.
    InvalidRatio,
    /// Too many nodes for the requested memory budget.
    NodeBudget,
    /// Too many nested splits for the selected depth budget.
    DepthBudget,
}

impl<T> Configuration<T> {
    /// Construct an individual pane, without allocating a split.
    pub fn pane(content: T) -> Self {
        Self::Pane(content)
    }

    /// Construct a split whose children are already valid configurations.
    ///
    /// Validation remains explicit because existing callers are allowed to
    /// construct split variants directly for backward compatibility.
    pub fn split(
        axis: Axis,
        ratio: f32,
        first: Self,
        second: Self,
    ) -> Self {
        Self::Split {
            axis,
            ratio,
            a: Box::new(first),
            b: Box::new(second),
        }
    }

    /// Check geometry and resource limits in a single nonrecursive traversal.
    ///
    /// A caller chooses its own maximum number of nodes and split depth.
    /// Native shells should validate untrusted or dynamically generated
    /// layouts before converting them into live pane, compositor and input
    /// state. No recursive call stack or heap proportional to the depth
    /// other than the explicit bounded traversal stack is required.
    pub fn validate(
        &self,
        max_nodes: usize,
        max_depth: usize,
    ) -> Result<LayoutStats, LayoutError> {
        let mut stack = vec![(self, 0_usize)];
        let mut stats = LayoutStats {
            nodes: 0,
            panes: 0,
            max_depth: 0,
        };

        while let Some((node, depth)) = stack.pop() {
            stats.nodes = stats.nodes.checked_add(1).ok_or(LayoutError::NodeBudget)?;
            if stats.nodes > max_nodes {
                return Err(LayoutError::NodeBudget);
            }
            if depth > max_depth {
                return Err(LayoutError::DepthBudget);
            }
            stats.max_depth = stats.max_depth.max(depth);

            match node {
                Self::Pane(_) => {
                    stats.panes = stats.panes.checked_add(1).ok_or(LayoutError::NodeBudget)?;
                }
                Self::Split { ratio, a, b, .. } => {
                    if !ratio.is_finite() || !(0.0..=1.0).contains(ratio) {
                        return Err(LayoutError::InvalidRatio);
                    }
                    let next = depth.checked_add(1).ok_or(LayoutError::DepthBudget)?;
                    stack.push((b, next));
                    stack.push((a, next));
                }
            }
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_nested_mixed_axis_workspace() {
        let scene = Configuration::split(
            Axis::Vertical,
            0.5,
            Configuration::pane("editor"),
            Configuration::split(
                Axis::Horizontal,
                0.25,
                Configuration::pane("terminal"),
                Configuration::pane("preview"),
            ),
        );
        assert_eq!(
            scene.validate(5, 2),
            Ok(LayoutStats {
                nodes: 5,
                panes: 3,
                max_depth: 2,
            }),
        );
        assert_eq!(scene.validate(4, 2), Err(LayoutError::NodeBudget));
        assert_eq!(scene.validate(5, 1), Err(LayoutError::DepthBudget));
    }

    #[test]
    fn rejects_nonfinite_and_out_of_range_ratios() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 1.1] {
            let config = Configuration::split(
                Axis::Horizontal,
                bad,
                Configuration::pane(1),
                Configuration::pane(2),
            );
            assert_eq!(config.validate(3, 1), Err(LayoutError::InvalidRatio));
        }
    }

    #[test]
    fn plain_pane_requires_one_node_budget() {
        let single = Configuration::pane(42);
        assert_eq!(single.validate(1, 0).unwrap().panes, 1);
        assert_eq!(single.validate(0, 0), Err(LayoutError::NodeBudget));
    }
}
