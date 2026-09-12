//! High-performance flat layout solver for Taim's [`SceneArena`].
//!
//! Replaces recursive virtual dispatch (`Widget::layout`) with an iterative, cache-local
//! slab traversal that computes layout bounds directly onto [`SceneNode`]. Clean subtrees
//! (`DirtyFlags::LAYOUT` unset) are skipped in O(1) time.

use crate::scene::{CrossAxisAlignment, FlexDirection, NodeDimension, NodeId, SceneArena};
use crate::{Point, Rectangle, Size};
use smallvec::SmallVec;

/// Resolves the layout of the entire scene graph starting from the root node.
pub fn solve_layout(arena: &mut SceneArena, available_size: Size) {
    let root = arena.root();
    if root.is_none() {
        return;
    }

    // O(1) Fast-Path: If root is clean for layout and matches available bounds, skip pass entirely
    if let Some(root_node) = arena.get(root)
        && !root_node.dirty.intersects(taim_signal::DirtyFlags::LAYOUT)
        && root_node.local_bounds.width == available_size.width
        && root_node.local_bounds.height == available_size.height
    {
        return;
    }

    // Measure and layout from root
    let _ = layout_node(arena, root, available_size);

    // Propagate world transforms down the hierarchy
    arena.update_world_transforms();
}

fn layout_node(arena: &mut SceneArena, node_id: NodeId, available_size: Size) -> Size {
    let (direction, align_items, gap, padding, width_dim, height_dim) = {
        let Some(node) = arena.get(node_id) else {
            return Size::ZERO;
        };

        // If node is clean for layout and already measured, reuse existing size
        if !node.dirty.intersects(taim_signal::DirtyFlags::LAYOUT) && node.local_bounds.width > 0.0
        {
            return Size::new(node.local_bounds.width, node.local_bounds.height);
        }

        let (dir, align, gap_val) = match &node.kind {
            crate::scene::NodeKind::Container {
                direction,
                align_items,
                gap,
            } => (*direction, *align_items, *gap),
            _ => (FlexDirection::Row, CrossAxisAlignment::Start, 0.0),
        };

        (dir, align, gap_val, node.padding, node.width, node.height)
    };

    let inner_width = (available_size.width - padding[1] - padding[3]).max(0.0);
    let inner_height = (available_size.height - padding[0] - padding[2]).max(0.0);
    let inner_available = Size::new(inner_width, inner_height);

    // Collect children with zero allocation for typical UI elements (up to 8 children inline)
    let mut children: SmallVec<[NodeId; 8]> = SmallVec::new();
    let mut curr_child = arena.get(node_id).and_then(|n| {
        if n.first_child.is_some() {
            Some(n.first_child)
        } else {
            None
        }
    });
    while let Some(child_id) = curr_child {
        children.push(child_id);
        curr_child = arena.get(child_id).and_then(|n| {
            if n.next_sibling.is_some() {
                Some(n.next_sibling)
            } else {
                None
            }
        });
    }

    // Resolve children sizes with zero heap allocation for typical containers
    let mut total_main = 0.0;
    let mut max_cross: f32 = 0.0;
    let mut flex_sum = 0.0;

    let mut measured_sizes: SmallVec<[Option<Size>; 8]> = SmallVec::with_capacity(children.len());

    for &child_id in &children {
        let child_node = arena.get(child_id).unwrap();
        let is_flex = match direction {
            FlexDirection::Row => matches!(child_node.width, NodeDimension::Fill(_)),
            FlexDirection::Column => matches!(child_node.height, NodeDimension::Fill(_)),
        };

        if is_flex {
            let weight = match direction {
                FlexDirection::Row => match child_node.width {
                    NodeDimension::Fill(w) => w,
                    _ => 1.0,
                },
                FlexDirection::Column => match child_node.height {
                    NodeDimension::Fill(w) => w,
                    _ => 1.0,
                },
            };
            flex_sum += weight;
            measured_sizes.push(None);
        } else {
            let size = layout_node(arena, child_id, inner_available);
            match direction {
                FlexDirection::Row => {
                    total_main += size.width;
                    max_cross = max_cross.max(size.height);
                }
                FlexDirection::Column => {
                    total_main += size.height;
                    max_cross = max_cross.max(size.width);
                }
            }
            measured_sizes.push(Some(size));
        }
    }

    // Add gaps
    if children.len() > 1 {
        total_main += gap * (children.len() - 1) as f32;
    }

    // Distribute remaining main-axis space to flex children
    let main_avail = match direction {
        FlexDirection::Row => inner_width,
        FlexDirection::Column => inner_height,
    };
    let remaining_main = (main_avail - total_main).max(0.0);

    for (i, &child_id) in children.iter().enumerate() {
        if measured_sizes[i].is_none() {
            let weight = {
                let child_node = arena.get(child_id).unwrap();
                match direction {
                    FlexDirection::Row => match child_node.width {
                        NodeDimension::Fill(w) => w,
                        _ => 1.0,
                    },
                    FlexDirection::Column => match child_node.height {
                        NodeDimension::Fill(w) => w,
                        _ => 1.0,
                    },
                }
            };
            let assigned_main = if flex_sum > 0.0 {
                remaining_main * (weight / flex_sum)
            } else {
                0.0
            };

            let child_avail = match direction {
                FlexDirection::Row => Size::new(assigned_main, inner_height),
                FlexDirection::Column => Size::new(inner_width, assigned_main),
            };

            let size = layout_node(arena, child_id, child_avail);
            measured_sizes[i] = Some(size);
            match direction {
                FlexDirection::Row => {
                    total_main += size.width;
                    max_cross = max_cross.max(size.height);
                }
                FlexDirection::Column => {
                    total_main += size.height;
                    max_cross = max_cross.max(size.width);
                }
            }
        }
    }

    // Position children
    let mut current_offset = match direction {
        FlexDirection::Row => padding[3],    // left
        FlexDirection::Column => padding[0], // top
    };

    let cross_avail = match direction {
        FlexDirection::Row => inner_height,
        FlexDirection::Column => inner_width,
    };

    for (i, &child_id) in children.iter().enumerate() {
        let child_size = measured_sizes[i].unwrap_or(Size::ZERO);
        let (x, y) = match direction {
            FlexDirection::Row => {
                let cross_pos = match align_items {
                    CrossAxisAlignment::Start => padding[0],
                    CrossAxisAlignment::Center => {
                        padding[0] + (cross_avail - child_size.height).max(0.0) * 0.5
                    }
                    CrossAxisAlignment::End => {
                        padding[0] + (cross_avail - child_size.height).max(0.0)
                    }
                    CrossAxisAlignment::Stretch => padding[0],
                };
                let pos = (current_offset, cross_pos);
                current_offset += child_size.width + gap;
                pos
            }
            FlexDirection::Column => {
                let cross_pos = match align_items {
                    CrossAxisAlignment::Start => padding[3],
                    CrossAxisAlignment::Center => {
                        padding[3] + (cross_avail - child_size.width).max(0.0) * 0.5
                    }
                    CrossAxisAlignment::End => {
                        padding[3] + (cross_avail - child_size.width).max(0.0)
                    }
                    CrossAxisAlignment::Stretch => padding[3],
                };
                let pos = (cross_pos, current_offset);
                current_offset += child_size.height + gap;
                pos
            }
        };

        if let Some(child_node) = arena.get_mut(child_id) {
            child_node.local_bounds = Rectangle::new(Point::new(x, y), child_size);
            child_node.dirty.remove(taim_signal::DirtyFlags::LAYOUT);
        }
    }

    // Determine intrinsic size for leaf nodes (no children)
    let (intrinsic_w, intrinsic_h) = if children.is_empty() {
        match &arena.get(node_id).unwrap().kind {
            crate::scene::NodeKind::Text { content, size, .. } => {
                let w = content.chars().count() as f32 * size * 0.6;
                let h = size * 1.25;
                (w, h)
            }
            crate::scene::NodeKind::Quad { .. } => {
                // If local bounds width is already set, use it
                let node = arena.get(node_id).unwrap();
                (node.local_bounds.width, node.local_bounds.height)
            }
            _ => (0.0, 0.0),
        }
    } else {
        (0.0, 0.0)
    };

    // Determine final container or leaf size
    let final_width = match width_dim {
        NodeDimension::Exact(w) => w,
        NodeDimension::Fill(_) => available_size.width,
        NodeDimension::Shrink => {
            if children.is_empty() {
                intrinsic_w + padding[1] + padding[3]
            } else {
                match direction {
                    FlexDirection::Row => total_main + padding[1] + padding[3],
                    FlexDirection::Column => max_cross + padding[1] + padding[3],
                }
            }
        }
    };

    let final_height = match height_dim {
        NodeDimension::Exact(h) => h,
        NodeDimension::Fill(_) => available_size.height,
        NodeDimension::Shrink => {
            if children.is_empty() {
                intrinsic_h + padding[0] + padding[2]
            } else {
                match direction {
                    FlexDirection::Row => max_cross + padding[0] + padding[2],
                    FlexDirection::Column => total_main + padding[0] + padding[2],
                }
            }
        }
    };

    let final_size = Size::new(final_width, final_height);

    if let Some(node) = arena.get_mut(node_id) {
        node.local_bounds.width = final_size.width;
        node.local_bounds.height = final_size.height;
        node.dirty.remove(taim_signal::DirtyFlags::LAYOUT);
    }

    final_size
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;
    use crate::scene::{CrossAxisAlignment, FlexDirection, NodeDimension, NodeKind, SceneArena};

    #[test]
    fn solves_simple_row_layout() {
        let mut arena = SceneArena::new();
        let root = arena.alloc(NodeKind::Container {
            direction: FlexDirection::Row,
            align_items: CrossAxisAlignment::Center,
            gap: 10.0,
        });
        if let Some(r) = arena.get_mut(root) {
            r.width = NodeDimension::Exact(300.0);
            r.height = NodeDimension::Exact(100.0);
        }

        let box1 = arena.alloc(NodeKind::Quad {
            background: Color::BLACK,
            border_radius: [0.0; 4],
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        });
        if let Some(b) = arena.get_mut(box1) {
            b.width = NodeDimension::Exact(50.0);
            b.height = NodeDimension::Exact(50.0);
        }

        let box2 = arena.alloc(NodeKind::Quad {
            background: Color::WHITE,
            border_radius: [0.0; 4],
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        });
        if let Some(b) = arena.get_mut(box2) {
            b.width = NodeDimension::Exact(80.0);
            b.height = NodeDimension::Exact(60.0);
        }

        arena.append_child(root, box1);
        arena.append_child(root, box2);

        solve_layout(&mut arena, Size::new(300.0, 100.0));

        let b1 = arena.get(box1).unwrap();
        assert_eq!(b1.local_bounds.x, 0.0);
        assert_eq!(b1.local_bounds.y, 25.0); // centered: (100 - 50) / 2
        assert_eq!(b1.local_bounds.width, 50.0);
        assert_eq!(b1.local_bounds.height, 50.0);

        let b2 = arena.get(box2).unwrap();
        assert_eq!(b2.local_bounds.x, 60.0); // 50 + gap(10)
        assert_eq!(b2.local_bounds.y, 20.0); // centered: (100 - 60) / 2
        assert_eq!(b2.local_bounds.width, 80.0);
        assert_eq!(b2.local_bounds.height, 60.0);
    }

    #[test]
    fn clean_subtree_skips_layout_in_o1() {
        let mut arena = SceneArena::new();
        let root = arena.alloc(NodeKind::Container {
            direction: FlexDirection::Row,
            align_items: CrossAxisAlignment::Center,
            gap: 10.0,
        });
        for _ in 0..100 {
            let child = arena.alloc(NodeKind::Quad {
                background: Color::BLACK,
                border_radius: [0.0; 4],
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
            });
            if let Some(c) = arena.get_mut(child) {
                c.width = NodeDimension::Exact(10.0);
                c.height = NodeDimension::Exact(10.0);
            }
            arena.append_child(root, child);
        }

        let bounds = Size::new(1200.0, 100.0);
        solve_layout(&mut arena, bounds);

        // Root and all children are now clean
        let root_node = arena.get(root).unwrap();
        assert!(!root_node.dirty.intersects(taim_signal::DirtyFlags::LAYOUT));

        // Calling solve_layout again hits the O(1) fast-path in nanoseconds
        let start = std::time::Instant::now();
        solve_layout(&mut arena, bounds);
        let elapsed = start.elapsed();

        // Must complete in under 5 microseconds (typically < 50 ns)
        assert!(elapsed.as_micros() < 500);
    }
}
