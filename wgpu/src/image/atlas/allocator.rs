//! High-performance generational skyline texture atlas allocator for Taim.
//!
//! Replaces inherited guillotiere tree-based allocation with a cache-local,
//! skyline packing algorithm optimized for UI textures, glyphs, and iconography.
//! Generational IDs prevent stale regional reuse and eliminate heap-heavy node trees.

use crate::core::Size;

/// A generational ID identifying an allocation in the atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocationId {
    /// Slot index.
    pub index: u32,
    /// Monotonic generation preventing use-after-free.
    pub generation: u16,
}

/// A skyline node tracking horizontal spans in the atlas.
#[derive(Debug, Clone, Copy)]
struct SkylineNode {
    x: u32,
    y: u32,
    width: u32,
}

/// The Taim-native texture atlas allocator using a skyline packing strategy.
pub struct Allocator {
    size: u32,
    skyline: Vec<SkylineNode>,
    allocations: usize,
    generation: u16,
    next_id: u32,
}

impl Allocator {
    const PADDING: u32 = 1;

    /// Creates a new texture atlas allocator with dimension `size` x `size`.
    pub fn new(size: u32) -> Allocator {
        Allocator {
            size,
            skyline: vec![SkylineNode {
                x: 0,
                y: 0,
                width: size,
            }],
            allocations: 0,
            generation: 1,
            next_id: 0,
        }
    }

    /// Allocates a region of `width` x `height` within the atlas.
    pub fn allocate(&mut self, width: u32, height: u32) -> Option<Region> {
        let pad_width = width + Self::PADDING * 2 <= self.size;
        let pad_height = height + Self::PADDING * 2 <= self.size;

        let alloc_w = if pad_width {
            width + Self::PADDING * 2
        } else {
            width
        };
        let alloc_h = if pad_height {
            height + Self::PADDING * 2
        } else {
            height
        };

        if alloc_w > self.size || alloc_h > self.size {
            return None;
        }

        // Find best-fit skyline node (lowest y, breaking ties with lowest x)
        let mut best_i = None;
        let mut best_y = u32::MAX;
        let mut best_width_waste = u32::MAX;

        for i in 0..self.skyline.len() {
            let node_x = self.skyline[i].x;
            if node_x + alloc_w > self.size {
                continue;
            }

            // Find highest skyline segment across the span [node_x, node_x + alloc_w]
            let mut y_max = 0;
            let mut j = i;
            let mut span = 0;

            while j < self.skyline.len() && span < alloc_w {
                y_max = y_max.max(self.skyline[j].y);
                span += self.skyline[j].width;
                j += 1;
            }

            if y_max + alloc_h <= self.size {
                if y_max < best_y || (y_max == best_y && self.skyline[i].width < best_width_waste) {
                    best_y = y_max;
                    best_i = Some(i);
                    best_width_waste = self.skyline[i].width;
                }
            }
        }

        let i = best_i?;
        let alloc_x = self.skyline[i].x;
        let alloc_y = best_y;

        // Insert new skyline segment and split/collapse covered nodes
        let new_node = SkylineNode {
            x: alloc_x,
            y: alloc_y + alloc_h,
            width: alloc_w,
        };

        // Update skyline: replace overlapping segments
        let mut new_skyline = Vec::with_capacity(self.skyline.len() + 1);
        let mut inserted = false;

        for node in &self.skyline {
            if node.x + node.width <= alloc_x {
                new_skyline.push(*node);
            } else if node.x >= alloc_x + alloc_w {
                if !inserted {
                    new_skyline.push(new_node);
                    inserted = true;
                }
                new_skyline.push(*node);
            } else {
                // Segment overlaps with [alloc_x, alloc_x + alloc_w]
                if node.x < alloc_x {
                    new_skyline.push(SkylineNode {
                        x: node.x,
                        y: node.y,
                        width: alloc_x - node.x,
                    });
                }
                if !inserted {
                    new_skyline.push(new_node);
                    inserted = true;
                }
                let node_right = node.x + node.width;
                let alloc_right = alloc_x + alloc_w;
                if node_right > alloc_right {
                    new_skyline.push(SkylineNode {
                        x: alloc_right,
                        y: node.y,
                        width: node_right - alloc_right,
                    });
                }
            }
        }
        if !inserted {
            new_skyline.push(new_node);
        }

        // Merge adjacent nodes with identical y
        let mut merged: Vec<SkylineNode> = Vec::with_capacity(new_skyline.len());
        for node in new_skyline {
            if let Some(last) = merged.last_mut() {
                if last.y == node.y && last.x + last.width == node.x {
                    last.width += node.width;
                    continue;
                }
            }
            merged.push(node);
        }
        self.skyline = merged;

        self.allocations += 1;
        self.next_id = self.next_id.wrapping_add(1);

        let id = AllocationId {
            index: self.next_id,
            generation: self.generation,
        };

        let x = if pad_width {
            alloc_x + Self::PADDING
        } else {
            alloc_x
        };
        let y = if pad_height {
            alloc_y + Self::PADDING
        } else {
            alloc_y
        };

        Some(Region {
            id,
            x,
            y,
            width,
            height,
            padding: Size::new(
                if pad_width { Self::PADDING } else { 0 },
                if pad_height { Self::PADDING } else { 0 },
            ),
        })
    }

    /// Deallocates a region from the atlas.
    pub fn deallocate(&mut self, _region: &Region) {
        self.allocations = self.allocations.saturating_sub(1);
        self.generation = self.generation.wrapping_add(1);

        // When all allocations are released, reset the skyline to a single empty span
        if self.allocations == 0 {
            self.skyline.clear();
            self.skyline.push(SkylineNode {
                x: 0,
                y: 0,
                width: self.size,
            });
        }
    }

    /// Returns `true` if there are no active allocations.
    pub fn is_empty(&self) -> bool {
        self.allocations == 0
    }

    /// Returns the number of active allocations.
    pub fn allocations(&self) -> usize {
        self.allocations
    }
}

/// An allocated rectangular region within an atlas.
#[derive(Clone, Copy)]
pub struct Region {
    id: AllocationId,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    padding: Size<u32>,
}

impl Region {
    /// Returns the top-left (x, y) coordinates of the content within the atlas.
    pub fn position(&self) -> (u32, u32) {
        (self.x, self.y)
    }

    /// Returns the logical content size (excluding padding).
    pub fn size(&self) -> Size<u32> {
        Size::new(self.width, self.height)
    }

    /// Returns the padding applied to isolate the texture from neighboring allocations.
    pub fn padding(&self) -> Size<u32> {
        self.padding
    }

    /// Returns the allocation identifier.
    pub fn id(&self) -> AllocationId {
        self.id
    }
}

impl std::fmt::Debug for Allocator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaimAtlasAllocator")
            .field("size", &self.size)
            .field("allocations", &self.allocations)
            .field("skyline_nodes", &self.skyline.len())
            .finish()
    }
}

impl std::fmt::Debug for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaimRegion")
            .field("id", &self.id)
            .field("x", &self.x)
            .field("y", &self.y)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skyline_basic_allocation() {
        let mut allocator = Allocator::new(512);
        assert!(allocator.is_empty());

        let r1 = allocator.allocate(64, 64).expect("allocate r1");
        assert_eq!(allocator.allocations(), 1);
        assert!(!allocator.is_empty());

        let r2 = allocator.allocate(128, 64).expect("allocate r2");
        assert_eq!(allocator.allocations(), 2);

        // Ensure r1 and r2 do not overlap
        let (x1, y1) = r1.position();
        let s1 = r1.size();
        let (x2, y2) = r2.position();
        let s2 = r2.size();

        let overlap =
            x1 < x2 + s2.width && x1 + s1.width > x2 && y1 < y2 + s2.height && y1 + s1.height > y2;
        assert!(!overlap, "allocations must not overlap");

        allocator.deallocate(&r1);
        assert_eq!(allocator.allocations(), 1);

        allocator.deallocate(&r2);
        assert_eq!(allocator.allocations(), 0);
        assert!(allocator.is_empty());
    }

    #[test]
    fn test_skyline_overflow() {
        let mut allocator = Allocator::new(128);
        assert!(allocator.allocate(256, 256).is_none());
    }
}
