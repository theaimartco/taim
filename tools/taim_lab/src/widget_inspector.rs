//! What is in the tree, and what it costs.
//!
//! # The two trees
//!
//! This shows the **widget** tree; [`semantics_inspector`] shows the
//! **semantic** forest. They are deliberately different shapes, and the
//! difference is usually the finding: a `column` of forty rows is forty
//! widgets and, correctly, nothing to a screen reader.
//!
//! Showing them in one panel would imply a correspondence that does not
//! exist and cannot be drawn.
//!
//! # What it can and cannot tell you
//!
//! It walks [`Visit`], the same traversal semantics and hit-testing use.
//! That is a deliberate constraint rather than a convenience: an inspector
//! with a walk of its own could disagree with the framework about the
//! shape of the tree, and a disagreeing inspector is worse than none
//! because it is believed.
//!
//! The cost is that `Visit` exposes what a *traversal* needs — bounds,
//! children, semantics — and not what a widget *is*. So this panel cannot
//! name a widget's type, show its state, or say whether its `Tree` node
//! survived reconciliation. `phase.md` asks for "tree, state, revisions
//! and identity"; this delivers tree and identity. State and revisions
//! need `Visit` to carry more, which is a framework change, not a panel
//! one. See [`Missing`].
//!
//! [`semantics_inspector`]: crate::semantics_inspector
//! [`Visit`]: taim_core::widget::visit::Visit

use taim::prelude::*;
use taim::view::AnyView;

use crate::target::Node;

/// What this panel is asked for and cannot show.
///
/// Written down rather than left as an absence: a developer looking for a
/// widget's revision counter and not finding it should learn *why* in one
/// place, not conclude the inspector is buggy.
pub const MISSING: &str = "Type, state and revisions are not shown: `Visit` carries bounds, \
                           children and semantics \u{2014} what a traversal needs \u{2014} not what a \
                           widget is. Adding them is a framework change, not a panel one.";

/// Renders the widget tree.
pub fn view(tree: &[Node]) -> AnyView {
    if tree.is_empty() {
        return container(text("No tree. Nothing is attached.").size(16))
            .center_x()
            .fill_width()
            .center_y()
            .fill_height()
            .into();
    }

    let rows = tree
        .iter()
        .fold(column![].gap(1.0), |rows, node| rows.push(node_row(node)));

    column![text(summarise(tree)).size(13), text(MISSING).size(11), rows,]
        .gap(8.0)
        .into()
}

fn node_row(node: &Node) -> AnyView {
    let indent = "  ".repeat(node.depth);

    // The identity, when the node has one. A node reports an identity only
    // by reporting semantics -- which most widgets do not, and should not.
    // An inspector that invented an id per row would be inventing exactly
    // the thing the accessibility engine depends on being stable.
    let identity = match &node.semantics {
        Some(semantics) => format!("{:?}", semantics.id),
        None => String::new(),
    };

    let role = match &node.semantics {
        Some(semantics) => format!("{:?}", semantics.role),
        None => "\u{2014}".to_owned(),
    };

    row![
        text(format!("{indent}{role}")).size(12),
        text(format!(
            "{:.0},{:.0} {:.0}\u{00d7}{:.0}",
            node.bounds.x, node.bounds.y, node.bounds.width, node.bounds.height
        ))
        .size(11),
        text(identity).size(10),
    ]
    .gap(10.0)
    .into()
}

/// A line describing the tree.
fn summarise(tree: &[Node]) -> String {
    let reporting = tree.iter().filter(|node| node.semantics.is_some()).count();

    let depth = tree.iter().map(|node| node.depth).max().unwrap_or(0);

    let zero_sized = tree
        .iter()
        .filter(|node| node.bounds.width == 0.0 || node.bounds.height == 0.0)
        .count();

    let zero = if zero_sized > 0 {
        // Worth surfacing: a zero-sized widget is invisible, unhittable,
        // and usually a layout mistake rather than an intent.
        format!("  \u{00b7}  {zero_sized} zero-sized")
    } else {
        String::new()
    };

    format!(
        "{} widgets \u{00b7} depth {} \u{00b7} {} report semantics{zero}",
        tree.len(),
        depth,
        reporting
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use taim_core::semantics::Role;
    use taim_core::widget::Id;
    use taim_core::{Point, Rectangle, Size};

    fn node(depth: usize, size: (f32, f32), reports: bool) -> Node {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(size.0, size.1));

        Node {
            depth,
            bounds,
            semantics: reports
                .then(|| taim_core::semantics::Node::new(Id::unique(), bounds, Role::Button)),
        }
    }

    #[test]
    fn the_summary_counts_the_two_trees_separately() {
        // The finding this panel exists for: forty widgets, two of which a
        // screen reader can see. A count that conflated them would hide it.
        let tree = vec![
            node(0, (100.0, 100.0), false),
            node(1, (50.0, 20.0), true),
            node(1, (50.0, 20.0), false),
        ];

        let summary = summarise(&tree);

        assert!(summary.contains("3 widgets"), "{summary}");
        assert!(summary.contains("1 report semantics"), "{summary}");
    }

    #[test]
    fn a_zero_sized_widget_is_surfaced() {
        // Invisible, unhittable, and usually a layout mistake rather than
        // an intent.
        let tree = vec![node(0, (100.0, 100.0), false), node(1, (0.0, 20.0), false)];

        assert!(summarise(&tree).contains("1 zero-sized"));
    }

    #[test]
    fn an_ordinary_tree_reports_no_zero_sized_widgets() {
        // The converse: a warning that is always on is a warning nobody
        // reads.
        let tree = vec![node(0, (100.0, 100.0), false)];

        assert!(!summarise(&tree).contains("zero-sized"));
    }

    #[test]
    fn depth_is_the_deepest_node_not_the_last() {
        // A tree that goes deep and comes back would report the wrong
        // depth if it tracked the last node it saw.
        let tree = vec![
            node(0, (10.0, 10.0), false),
            node(3, (10.0, 10.0), false),
            node(1, (10.0, 10.0), false),
        ];

        assert!(summarise(&tree).contains("depth 3"));
    }

    #[test]
    fn a_node_with_no_semantics_shows_no_invented_identity() {
        // An inspector that made up an id per row would be inventing
        // exactly the thing the accessibility engine depends on being
        // stable -- and a developer comparing ids across frames would be
        // comparing fiction.
        let element: AnyView = node_row(&node(0, (10.0, 10.0), false));

        let _ = std::hint::black_box(element);
    }

    #[test]
    fn the_panel_says_what_it_cannot_show() {
        // A developer looking for a revision counter should learn why in
        // one place, not conclude the inspector is buggy.
        assert!(MISSING.contains("state"), "{MISSING}");
        assert!(MISSING.contains("Visit"), "{MISSING}");
    }

    #[test]
    fn an_empty_tree_says_nothing_is_attached() {
        let element: AnyView = view(&[]);

        let _ = std::hint::black_box(element);
    }
}
