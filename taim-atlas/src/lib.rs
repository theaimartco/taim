//! High-performance generational skyline and free-rectangle atlas allocation for TAIM.
//!
//! Combines a cache-local skyline frontier with dynamic free-rectangle recycling
//! and generational slot validation. Freed rectangular regions are reclaimed and
//! coalesced in-place without requiring full atlas resets. Persistent scratch
//! storage ensures zero heap allocations during steady-state allocation calls.

/// Width and height of an allocated atlas region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extent {
    /// Horizontal extent in pixels.
    pub width: u32,
    /// Vertical extent in pixels.
    pub height: u32,
}

impl Extent {
    const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// A generational ID identifying an allocation in the atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocationId {
    /// Slot index in the allocation table.
    pub index: u32,
    /// Monotonic generation preventing stale access and use-after-free.
    pub generation: u32,
}

/// A free rectangular slot reclaimed from deallocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FreeRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

/// A skyline node tracking the frontier of unallocated space in the atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SkylineNode {
    x: u32,
    y: u32,
    width: u32,
}

/// An entry in the generational slot table.
#[derive(Debug, Clone, Copy)]
struct SlotEntry {
    generation: u32,
    is_active: bool,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    pad_w: u32,
    pad_h: u32,
}

/// The Taim-native texture atlas allocator with free-rect recycling.
pub struct Allocator {
    size: u32,
    padding: u32,
    skyline: Vec<SkylineNode>,
    free_rects: Vec<FreeRect>,
    slots: Vec<SlotEntry>,
    free_slot_indices: Vec<u32>,
    scratch_skyline: Vec<SkylineNode>,
    scratch_merged: Vec<SkylineNode>,
    allocations: usize,
    peak_skyline_nodes: usize,
    peak_free_rects: usize,
}

impl Allocator {
    /// Creates a new texture atlas allocator with dimension `size` x `size`.
    pub fn new(size: u32) -> Allocator {
        Self::with_padding(size, 1)
    }

    /// Creates an allocator without per-region padding.
    ///
    /// This is appropriate for exact texel-load consumers such as the TAIM
    /// glyph engine, where neighboring texels are never filtered together.
    pub fn unpadded(size: u32) -> Allocator {
        Self::with_padding(size, 0)
    }

    fn with_padding(size: u32, padding: u32) -> Allocator {
        Allocator {
            size,
            padding,
            skyline: vec![SkylineNode {
                x: 0,
                y: 0,
                width: size,
            }],
            free_rects: Vec::new(),
            slots: Vec::with_capacity(64),
            free_slot_indices: Vec::new(),
            scratch_skyline: Vec::with_capacity(32),
            scratch_merged: Vec::with_capacity(32),
            allocations: 0,
            peak_skyline_nodes: 1,
            peak_free_rects: 0,
        }
    }

    /// Allocates a region of `width` x `height` within the atlas.
    ///
    /// First searches the free-rectangle recycler to reuse holes left by
    /// previous deallocations. If no recycled hole fits, allocates from the
    /// skyline frontier using preallocated scratch buffers (zero heap allocations).
    pub fn allocate(&mut self, width: u32, height: u32) -> Option<Region> {
        let pad_width = width + self.padding * 2 <= self.size;
        let pad_height = height + self.padding * 2 <= self.size;

        let pad_w = if pad_width { self.padding } else { 0 };
        let pad_h = if pad_height { self.padding } else { 0 };

        let alloc_w = width + pad_w * 2;
        let alloc_h = height + pad_h * 2;

        if alloc_w > self.size || alloc_h > self.size {
            return None;
        }

        // --- Step 1: Check recycled free rectangles (hole reuse) ---
        if let Some((best_idx, rect)) = self.find_best_free_rect(alloc_w, alloc_h) {
            let _ = self.free_rects.swap_remove(best_idx);

            // Guillotine split residual area
            let rem_w = rect.width - alloc_w;
            let rem_h = rect.height - alloc_h;

            if rem_w > 0 && rem_h > 0 {
                // Shorter axis split to minimize fragmentation
                if rem_w <= rem_h {
                    self.push_free_rect(FreeRect {
                        x: rect.x + alloc_w,
                        y: rect.y,
                        width: rem_w,
                        height: alloc_h,
                    });
                    self.push_free_rect(FreeRect {
                        x: rect.x,
                        y: rect.y + alloc_h,
                        width: rect.width,
                        height: rem_h,
                    });
                } else {
                    self.push_free_rect(FreeRect {
                        x: rect.x,
                        y: rect.y + alloc_h,
                        width: alloc_w,
                        height: rem_h,
                    });
                    self.push_free_rect(FreeRect {
                        x: rect.x + alloc_w,
                        y: rect.y,
                        width: rem_w,
                        height: rect.height,
                    });
                }
            } else if rem_w > 0 {
                self.push_free_rect(FreeRect {
                    x: rect.x + alloc_w,
                    y: rect.y,
                    width: rem_w,
                    height: rect.height,
                });
            } else if rem_h > 0 {
                self.push_free_rect(FreeRect {
                    x: rect.x,
                    y: rect.y + alloc_h,
                    width: rect.width,
                    height: rem_h,
                });
            }

            let id = self.register_allocation(rect.x, rect.y, alloc_w, alloc_h, pad_w, pad_h);
            return Some(Region {
                id,
                x: rect.x + pad_w,
                y: rect.y + pad_h,
                width,
                height,
                padding: Extent::new(pad_w, pad_h),
            });
        }

        // --- Step 2: Allocate from Skyline frontier ---
        let mut best_i = None;
        let mut best_y = u32::MAX;
        let mut best_width_waste = u32::MAX;

        for i in 0..self.skyline.len() {
            let node_x = self.skyline[i].x;
            if node_x + alloc_w > self.size {
                continue;
            }

            // Find maximum skyline height across span [node_x, node_x + alloc_w]
            let mut y_max = 0;
            let mut j = i;
            let mut span = 0;

            while j < self.skyline.len() && span < alloc_w {
                y_max = y_max.max(self.skyline[j].y);
                span += self.skyline[j].width;
                j += 1;
            }

            if y_max + alloc_h <= self.size
                && (y_max < best_y || (y_max == best_y && self.skyline[i].width < best_width_waste))
            {
                best_y = y_max;
                best_i = Some(i);
                best_width_waste = self.skyline[i].width;
            }
        }

        let i = best_i?;
        let alloc_x = self.skyline[i].x;
        let alloc_y = best_y;

        let new_node = SkylineNode {
            x: alloc_x,
            y: alloc_y + alloc_h,
            width: alloc_w,
        };

        // Update skyline using preallocated scratch buffers to avoid heap allocations
        self.scratch_skyline.clear();
        let mut inserted = false;

        for node in &self.skyline {
            if node.x + node.width <= alloc_x {
                self.scratch_skyline.push(*node);
            } else if node.x >= alloc_x + alloc_w {
                if !inserted {
                    self.scratch_skyline.push(new_node);
                    inserted = true;
                }
                self.scratch_skyline.push(*node);
            } else {
                if node.x < alloc_x {
                    self.scratch_skyline.push(SkylineNode {
                        x: node.x,
                        y: node.y,
                        width: alloc_x - node.x,
                    });
                }
                if !inserted {
                    self.scratch_skyline.push(new_node);
                    inserted = true;
                }
                let node_right = node.x + node.width;
                let alloc_right = alloc_x + alloc_w;
                if node_right > alloc_right {
                    self.scratch_skyline.push(SkylineNode {
                        x: alloc_right,
                        y: node.y,
                        width: node_right - alloc_right,
                    });
                }
            }
        }
        if !inserted {
            self.scratch_skyline.push(new_node);
        }

        // Merge adjacent nodes with identical y in scratch_merged
        self.scratch_merged.clear();
        for node in &self.scratch_skyline {
            if let Some(last) = self
                .scratch_merged
                .last_mut()
                .filter(|l| l.y == node.y && l.x + l.width == node.x)
            {
                last.width += node.width;
                continue;
            }
            self.scratch_merged.push(*node);
        }

        std::mem::swap(&mut self.skyline, &mut self.scratch_merged);
        self.peak_skyline_nodes = self.peak_skyline_nodes.max(self.skyline.len());

        let id = self.register_allocation(alloc_x, alloc_y, alloc_w, alloc_h, pad_w, pad_h);

        Some(Region {
            id,
            x: alloc_x + pad_w,
            y: alloc_y + pad_h,
            width,
            height,
            padding: Extent::new(pad_w, pad_h),
        })
    }

    /// Deallocates a region from the atlas.
    ///
    /// Validates the generational allocation ID to prevent double-free and
    /// use-after-free. Reclaims the rectangular space into the free list,
    /// immediately coalescing with neighboring free rectangles where possible.
    pub fn deallocate(&mut self, region: &Region) {
        let slot_idx = region.id.index as usize;
        if slot_idx >= self.slots.len() {
            return;
        }

        let slot = &mut self.slots[slot_idx];
        if !slot.is_active || slot.generation != region.id.generation {
            // Stale ID or already deallocated
            return;
        }

        slot.is_active = false;
        slot.generation = slot.generation.wrapping_add(1);
        self.free_slot_indices.push(slot_idx as u32);
        self.allocations = self.allocations.saturating_sub(1);

        if self.allocations == 0 {
            // All allocations released: completely reset skyline and free list
            self.free_rects.clear();
            self.skyline.clear();
            self.skyline.push(SkylineNode {
                x: 0,
                y: 0,
                width: self.size,
            });
            return;
        }

        // Reclaim hole and coalesce
        let reclaimed = FreeRect {
            x: slot.x,
            y: slot.y,
            width: slot.width,
            height: slot.height,
        };
        self.push_free_rect(reclaimed);
    }

    /// Returns `true` if there are no active allocations.
    pub fn is_empty(&self) -> bool {
        self.allocations == 0
    }

    /// Returns the number of active allocations.
    pub fn allocations(&self) -> usize {
        self.allocations
    }

    /// Expands the square allocation domain without moving active regions.
    ///
    /// The skyline already exposes newly available vertical space once the
    /// limit changes. A right-edge node exposes the additional horizontal
    /// strip, preserving every allocation coordinate and generation.
    pub fn grow(&mut self, new_size: u32) {
        assert!(new_size >= self.size, "an atlas allocator cannot shrink");

        if new_size == self.size {
            return;
        }

        let old_size = self.size;
        self.size = new_size;

        if self.allocations == 0 {
            self.free_rects.clear();
            self.skyline.clear();
            self.skyline.push(SkylineNode {
                x: 0,
                y: 0,
                width: new_size,
            });
            return;
        }

        if let Some(last) = self.skyline.last_mut().filter(|last| last.y == 0) {
            last.width += new_size - old_size;
        } else {
            self.skyline.push(SkylineNode {
                x: old_size,
                y: 0,
                width: new_size - old_size,
            });
        }
        self.peak_skyline_nodes = self.peak_skyline_nodes.max(self.skyline.len());
    }

    /// Returns the count of recyclable free holes currently tracked.
    #[allow(dead_code)]
    pub fn free_rect_count(&self) -> usize {
        self.free_rects.len()
    }

    /// Returns the number of skyline nodes currently tracked.
    #[allow(dead_code)]
    pub fn skyline_node_count(&self) -> usize {
        self.skyline.len()
    }

    /// Returns the peak number of skyline nodes observed.
    #[allow(dead_code)]
    pub fn peak_skyline_nodes(&self) -> usize {
        self.peak_skyline_nodes
    }

    /// Returns the peak number of free rectangles observed.
    #[allow(dead_code)]
    pub fn peak_free_rects(&self) -> usize {
        self.peak_free_rects
    }

    /// Returns `true` if the given allocation ID is currently valid and active.
    #[allow(dead_code)]
    pub fn is_allocated(&self, id: AllocationId) -> bool {
        let idx = id.index as usize;
        if idx < self.slots.len() {
            let slot = &self.slots[idx];
            slot.is_active && slot.generation == id.generation
        } else {
            false
        }
    }

    // --- Internal Helpers ---

    fn find_best_free_rect(&self, w: u32, h: u32) -> Option<(usize, FreeRect)> {
        let mut best = None;
        let mut min_waste = u64::MAX;

        for (idx, &rect) in self.free_rects.iter().enumerate() {
            if rect.width >= w && rect.height >= h {
                let waste = (rect.width as u64 * rect.height as u64) - (w as u64 * h as u64);
                if waste < min_waste {
                    min_waste = waste;
                    best = Some((idx, rect));
                    if waste == 0 {
                        break;
                    }
                }
            }
        }
        best
    }

    fn push_free_rect(&mut self, mut rect: FreeRect) {
        // Coalesce with adjacent free rectangles
        let mut i = 0;
        while i < self.free_rects.len() {
            let other = self.free_rects[i];

            // Horizontal coalesce (same y, same height, contiguous x)
            if rect.y == other.y && rect.height == other.height {
                if rect.x + rect.width == other.x {
                    rect.width += other.width;
                    let _ = self.free_rects.swap_remove(i);
                    i = 0;
                    continue;
                } else if other.x + other.width == rect.x {
                    rect.x = other.x;
                    rect.width += other.width;
                    let _ = self.free_rects.swap_remove(i);
                    i = 0;
                    continue;
                }
            }

            // Vertical coalesce (same x, same width, contiguous y)
            if rect.x == other.x && rect.width == other.width {
                if rect.y + rect.height == other.y {
                    rect.height += other.height;
                    let _ = self.free_rects.swap_remove(i);
                    i = 0;
                    continue;
                } else if other.y + other.height == rect.y {
                    rect.y = other.y;
                    rect.height += other.height;
                    let _ = self.free_rects.swap_remove(i);
                    i = 0;
                    continue;
                }
            }

            i += 1;
        }

        self.free_rects.push(rect);
        self.peak_free_rects = self.peak_free_rects.max(self.free_rects.len());
    }

    fn register_allocation(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        pad_w: u32,
        pad_h: u32,
    ) -> AllocationId {
        self.allocations += 1;

        if let Some(idx) = self.free_slot_indices.pop() {
            let slot = &mut self.slots[idx as usize];
            slot.is_active = true;
            slot.x = x;
            slot.y = y;
            slot.width = width;
            slot.height = height;
            slot.pad_w = pad_w;
            slot.pad_h = pad_h;
            AllocationId {
                index: idx,
                generation: slot.generation,
            }
        } else {
            let idx = self.slots.len() as u32;
            self.slots.push(SlotEntry {
                generation: 1,
                is_active: true,
                x,
                y,
                width,
                height,
                pad_w,
                pad_h,
            });
            AllocationId {
                index: idx,
                generation: 1,
            }
        }
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
    padding: Extent,
}

impl Region {
    /// Returns the top-left (x, y) coordinates of the content within the atlas.
    pub fn position(&self) -> (u32, u32) {
        (self.x, self.y)
    }

    /// Returns the logical content size (excluding padding).
    pub fn size(&self) -> Extent {
        Extent::new(self.width, self.height)
    }

    /// Returns the padding applied to isolate the texture from neighboring allocations.
    pub fn padding(&self) -> Extent {
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
            .field("free_rects", &self.free_rects.len())
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
        assert!(allocator.is_allocated(r1.id()));

        let r2 = allocator.allocate(128, 64).expect("allocate r2");
        assert_eq!(allocator.allocations(), 2);
        assert!(allocator.is_allocated(r2.id()));

        // Ensure regions do not overlap
        let (x1, y1) = r1.position();
        let (x2, y2) = r2.position();
        let no_overlap = x1 + r1.size().width <= x2
            || x2 + r2.size().width <= x1
            || y1 + r1.size().height <= y2
            || y2 + r2.size().height <= y1;
        assert!(no_overlap, "regions must not overlap");

        allocator.deallocate(&r1);
        assert_eq!(allocator.allocations(), 1);
        assert!(!allocator.is_allocated(r1.id()));
        assert!(allocator.is_allocated(r2.id()));

        allocator.deallocate(&r2);
        assert_eq!(allocator.allocations(), 0);
        assert!(allocator.is_empty());
    }

    #[test]
    fn test_hole_reclamation_and_reuse() {
        let mut allocator = Allocator::new(512);

        // Allocate 3 blocks
        let b1 = allocator.allocate(64, 64).expect("b1");
        let b2 = allocator.allocate(64, 64).expect("b2");
        let b3 = allocator.allocate(64, 64).expect("b3");
        assert_eq!(allocator.allocations(), 3);

        // Free the middle block b2
        allocator.deallocate(&b2);
        assert_eq!(allocator.allocations(), 2);
        assert_eq!(allocator.free_rect_count(), 1);
        assert!(!allocator.is_allocated(b2.id()));

        // Double-free of b2 must be safely ignored
        allocator.deallocate(&b2);
        assert_eq!(allocator.allocations(), 2);

        // Allocate a new block of identical size: MUST reuse the hole left by b2
        let b4 = allocator.allocate(64, 64).expect("b4");
        assert_eq!(allocator.allocations(), 3);
        assert_eq!(allocator.free_rect_count(), 0); // Hole was reused!
        assert_eq!(b4.position(), b2.position());
        assert_ne!(b4.id(), b2.id()); // Generational ID differs!

        allocator.deallocate(&b1);
        allocator.deallocate(&b3);
        allocator.deallocate(&b4);
        assert!(allocator.is_empty());
    }

    #[test]
    fn test_coalescing_of_freed_holes() {
        let mut allocator = Allocator::new(512);

        // Allocate 2 horizontally adjacent 64x64 blocks
        let b1 = allocator.allocate(64, 64).expect("b1");
        let b2 = allocator.allocate(64, 64).expect("b2");
        let _b3 = allocator.allocate(64, 64).expect("b3");

        // Free b1 and b2
        allocator.deallocate(&b1);
        allocator.deallocate(&b2);

        // Holes b1 and b2 should coalesce into a single 132-wide hole (accounting for padding)
        assert_eq!(allocator.free_rect_count(), 1);

        // A 128x64 allocation should now fit into the coalesced hole!
        let b_large = allocator.allocate(128, 64).expect("b_large");
        assert_eq!(b_large.position(), b1.position());
    }

    #[test]
    fn test_stale_id_detection() {
        let mut allocator = Allocator::new(256);
        let r1 = allocator.allocate(32, 32).expect("r1");
        let stale_id = r1.id();

        allocator.deallocate(&r1);
        assert!(!allocator.is_allocated(stale_id));

        // Subsequent allocation reuses slot, but has bumped generation
        let r2 = allocator.allocate(32, 32).expect("r2");
        assert!(allocator.is_allocated(r2.id()));
        assert!(!allocator.is_allocated(stale_id));
        assert_ne!(r2.id(), stale_id);
    }

    #[test]
    fn test_skyline_overflow() {
        let mut allocator = Allocator::new(128);
        assert!(allocator.allocate(256, 256).is_none());
    }

    #[test]
    fn test_ten_thousand_cycles_glyph_and_image() {
        let mut allocator = Allocator::new(2048);
        let mut active_regions = Vec::new();
        let mut latencies_ns = Vec::with_capacity(10_000);

        // Run 10,000 allocate & deallocate cycles with mixed sizes
        for cycle in 0..10_000 {
            // Alternate between tiny glyphs and medium images
            let (w, h) = match cycle % 5 {
                0 => (12, 16),   // tiny glyph
                1 => (18, 24),   // small glyph
                2 => (32, 32),   // icon
                3 => (64, 64),   // medium avatar
                _ => (128, 128), // image
            };

            let start = std::time::Instant::now();
            let opt_region = allocator.allocate(w, h);
            latencies_ns.push(start.elapsed().as_nanos() as u64);

            if let Some(region) = opt_region {
                active_regions.push(region);
            }

            // Keep bounded working set to stress hole recycling
            if active_regions.len() > 150 || (cycle % 3 == 0 && !active_regions.is_empty()) {
                let idx = cycle % active_regions.len();
                let region = active_regions.swap_remove(idx);
                allocator.deallocate(&region);
            }
        }

        // Clean up remaining allocations
        for r in &active_regions {
            allocator.deallocate(r);
        }

        assert!(allocator.is_empty());
        assert_eq!(allocator.allocations(), 0);
        assert!(allocator.peak_free_rects() > 0);

        latencies_ns.sort_unstable();
        let p50 = latencies_ns[latencies_ns.len() * 50 / 100];
        let p95 = latencies_ns[latencies_ns.len() * 95 / 100];
        let p99 = latencies_ns[latencies_ns.len() * 99 / 100];
        println!(
            "[Atlas Benchmark] 10,000 cycles: p50={}ns, p95={}ns, p99={}ns, peak_skyline={}, peak_free_rects={}",
            p50,
            p95,
            p99,
            allocator.peak_skyline_nodes(),
            allocator.peak_free_rects()
        );
    }

    #[test]
    fn test_fragmentation_and_hole_reuse_pressure() {
        let mut allocator = Allocator::new(1024);
        let mut regions = Vec::new();

        // Fill with 32x32 blocks
        for _ in 0..64 {
            if let Some(r) = allocator.allocate(32, 32) {
                regions.push(r);
            }
        }
        let total_initial = regions.len();

        // Deallocate every other block to create holes
        let mut freed_count = 0;
        let mut i = 0;
        while i < regions.len() {
            let r = regions.swap_remove(i);
            allocator.deallocate(&r);
            freed_count += 1;
            i += 1;
        }

        // The 32 freed blocks coalesce into larger contiguous free spans
        assert!(allocator.free_rect_count() >= 1);

        // Reallocate: all must fit into freed holes without growing the skyline
        let skyline_before = allocator.skyline_node_count();
        for _ in 0..freed_count {
            let r = allocator
                .allocate(32, 32)
                .expect("must fit in recycled hole");
            regions.push(r);
        }

        assert_eq!(regions.len(), total_initial);
        assert_eq!(allocator.skyline_node_count(), skyline_before);

        // Deallocate all
        for r in &regions {
            allocator.deallocate(r);
        }
        assert!(allocator.is_empty());
    }
}
