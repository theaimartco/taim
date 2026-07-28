//! What bounds the layout produced.
//!
//! # What `phase.md` asks for, and what this delivers
//!
//! The plan says "constraints, bounds, baselines and cache hits". This
//! delivers **bounds**, and the honesty is in the other three:
//!
//! - **Constraints** are the `Limits` a parent handed a child. They exist
//!   only inside `Widget::layout`, on the stack, and are gone by the time
//!   anything can be asked. Showing them needs the framework to record
//!   them — a `Visit` that carried the `Limits` each node was measured
//!   against, or a profiler event per layout call. Neither exists.
//! - **Baselines** are not modelled anywhere in Aura. There is no text
//!   baseline in `layout::Node`, so there is nothing to show.
//! - **Cache hits** need the layout cache to record them. Nothing does.
//!
//! A panel that drew a "constraint" it had inferred from the resulting
//! bounds would be showing a developer their own output relabelled as
//! their input — which is worse than a blank space, because it looks like
//! information.
//!
//! # What bounds alone are good for
//!
//! More than it sounds. Most layout bugs are visible in the geometry: a
//! widget with zero width, one that escaped its parent, one that is
//! wider than the surface. Those are checkable from bounds alone, and
//! they are what this panel checks.

use aura::widget::{column, container, row, scrollable, text};
use aura::{Element, Length, Rectangle, Size};

use crate::target::Node;

/// What the plan asks for and this cannot show, and why.
pub const MISSING: &str = "Constraints, baselines and cache hits are not shown. Constraints live \
                           on the stack inside `Widget::layout` and are gone before anything can \
                           ask; baselines are not modelled in `layout::Node` at all; nothing \
                           records layout cache hits. Inferring a constraint from the bounds it \
                           produced would show you your output relabelled as your input.";

/// Something wrong with a node's geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finding {
    /// Zero width or height: invisible and unhittable.
    ZeroSized,
    /// Outside its parent's bounds — usually a layout bug, sometimes a
    /// deliberate overflow.
    Escaped,
    /// Larger than the surface it is laid out against.
    Oversized,
    /// Not finite. A NaN here means a `Length` computed from bad input
    /// reached the layout pass.
    NotFinite,
}

impl Finding {
    /// What to call it.
    pub fn as_str(self) -> &'static str {
        match self {
            Finding::ZeroSized => "zero-sized",
            Finding::Escaped => "escapes parent",
            Finding::Oversized => "exceeds surface",
            Finding::NotFinite => "NOT FINITE",
        }
    }
}

/// Everything wrong with `node`'s geometry.
///
/// `parent` is `None` for a root.
pub fn findings(node: Rectangle, parent: Option<Rectangle>, surface: Size) -> Vec<Finding> {
    let mut findings = Vec::new();

    // Checked first: every comparison below would silently return `false`
    // against a NaN, so an unchecked NaN reports a perfectly healthy node.
    if !node.x.is_finite()
        || !node.y.is_finite()
        || !node.width.is_finite()
        || !node.height.is_finite()
    {
        findings.push(Finding::NotFinite);

        return findings;
    }

    if node.width == 0.0 || node.height == 0.0 {
        findings.push(Finding::ZeroSized);
    }

    if let Some(parent) = parent
        && !node.is_within(&parent)
    {
        findings.push(Finding::Escaped);
    }

    if node.width > surface.width || node.height > surface.height {
        findings.push(Finding::Oversized);
    }

    findings
}

/// Renders the layout overlay.
pub fn view<'a, Message: 'a>(tree: &[Node], surface: Size) -> Element<'a, Message> {
    if tree.is_empty() {
        return container(text("No layout. Nothing is attached.").size(16))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
    }

    let mut rows = column![].spacing(1);
    let mut flagged = 0;

    // A parent stack, so `Escaped` can be checked against the real parent
    // rather than the previous row -- the tree is depth-first, so the
    // previous row is a sibling as often as a parent.
    let mut ancestors: Vec<(usize, Rectangle)> = Vec::new();

    for node in tree {
        ancestors.retain(|(depth, _)| *depth < node.depth);

        let parent = ancestors.last().map(|(_, bounds)| *bounds);
        let found = findings(node.bounds, parent, surface);

        if !found.is_empty() {
            flagged += 1;
        }

        rows = rows.push(node_row(node, &found));
        ancestors.push((node.depth, node.bounds));
    }

    let summary = if flagged > 0 {
        format!(
            "{} nodes \u{00b7} surface {:.0}\u{00d7}{:.0} \u{00b7} {flagged} FLAGGED",
            tree.len(),
            surface.width,
            surface.height
        )
    } else {
        format!(
            "{} nodes \u{00b7} surface {:.0}\u{00d7}{:.0} \u{00b7} geometry clean",
            tree.len(),
            surface.width,
            surface.height
        )
    };

    column![
        text(summary).size(13),
        text(MISSING).size(11),
        scrollable(rows).height(Length::Fill),
    ]
    .spacing(8)
    .into()
}

fn node_row<'a, Message: 'a>(node: &Node, findings: &[Finding]) -> Element<'a, Message> {
    let indent = "  ".repeat(node.depth);

    let flags = findings
        .iter()
        .map(|finding| finding.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    row![
        text(format!(
            "{indent}{:>7.1},{:<7.1}",
            node.bounds.x, node.bounds.y
        ))
        .size(11),
        text(format!(
            "{:>7.1}\u{00d7}{:<7.1}",
            node.bounds.width, node.bounds.height
        ))
        .size(11),
        text(flags).size(11),
    ]
    .spacing(10)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use aura::Point;

    const SURFACE: Size = Size {
        width: 400.0,
        height: 600.0,
    };

    fn at(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
        Rectangle::new(Point::new(x, y), Size::new(width, height))
    }

    #[test]
    fn ordinary_geometry_is_not_flagged() {
        // A warning that is always on is a warning nobody reads.
        let parent = at(0.0, 0.0, 200.0, 200.0);

        assert!(findings(at(10.0, 10.0, 50.0, 20.0), Some(parent), SURFACE).is_empty());
    }

    #[test]
    fn a_zero_sized_node_is_flagged() {
        assert_eq!(
            findings(at(0.0, 0.0, 0.0, 20.0), None, SURFACE),
            vec![Finding::ZeroSized]
        );
    }

    #[test]
    fn a_node_outside_its_parent_is_flagged() {
        let parent = at(0.0, 0.0, 100.0, 100.0);

        assert!(
            findings(at(50.0, 50.0, 100.0, 100.0), Some(parent), SURFACE)
                .contains(&Finding::Escaped)
        );
    }

    #[test]
    fn a_root_has_no_parent_to_escape() {
        // A root is not "escaping" by virtue of having nothing to be
        // inside. Flagging every root would make the flag meaningless.
        assert!(!findings(at(0.0, 0.0, 400.0, 600.0), None, SURFACE).contains(&Finding::Escaped));
    }

    #[test]
    fn a_node_larger_than_the_surface_is_flagged() {
        assert!(findings(at(0.0, 0.0, 500.0, 100.0), None, SURFACE).contains(&Finding::Oversized));
    }

    #[test]
    fn a_nan_is_checked_before_anything_else() {
        // The trap: every comparison below returns `false` against a NaN,
        // so an unchecked NaN reports a perfectly healthy node. It must be
        // the first thing tested and the only thing reported.
        let found = findings(at(f32::NAN, 0.0, 50.0, 20.0), None, SURFACE);

        assert_eq!(
            found,
            vec![Finding::NotFinite],
            "a NaN must not be reported as clean geometry"
        );
    }

    #[test]
    fn an_infinite_size_is_not_finite() {
        assert_eq!(
            findings(at(0.0, 0.0, f32::INFINITY, 20.0), None, SURFACE),
            vec![Finding::NotFinite]
        );
    }

    #[test]
    fn several_findings_are_all_reported() {
        // A node can be wrong in more than one way, and reporting only the
        // first would send a developer round the loop twice.
        let parent = at(0.0, 0.0, 10.0, 10.0);
        let found = findings(at(0.0, 0.0, 0.0, 900.0), Some(parent), SURFACE);

        assert!(found.contains(&Finding::ZeroSized));
        assert!(found.contains(&Finding::Oversized));
    }

    #[test]
    fn the_panel_says_what_it_cannot_show() {
        // Inferring a constraint from the bounds it produced would show a
        // developer their own output relabelled as their input.
        assert!(MISSING.contains("Constraints"), "{MISSING}");
        assert!(MISSING.contains("baselines"), "{MISSING}");
    }

    #[test]
    fn escaping_is_checked_against_the_parent_not_the_previous_row() {
        // The tree is depth-first, so the previous row is a sibling as
        // often as a parent. Checking against it would flag every second
        // child of a row.
        let tree = vec![
            Node {
                depth: 0,
                bounds: at(0.0, 0.0, 200.0, 100.0),
                semantics: None,
            },
            Node {
                depth: 1,
                bounds: at(0.0, 0.0, 50.0, 50.0),
                semantics: None,
            },
            // A sibling to the right of the first child: outside *it*, but
            // well inside the parent.
            Node {
                depth: 1,
                bounds: at(100.0, 0.0, 50.0, 50.0),
                semantics: None,
            },
        ];

        let element: Element<'_, ()> = view(&tree, SURFACE);
        let _ = std::hint::black_box(element);

        // The direct assertion, since the built element cannot be read:
        // the second child is inside the parent and must not be flagged.
        assert!(
            findings(
                at(100.0, 0.0, 50.0, 50.0),
                Some(at(0.0, 0.0, 200.0, 100.0)),
                SURFACE
            )
            .is_empty()
        );
    }
}
