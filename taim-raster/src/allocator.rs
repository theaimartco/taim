//! Sub-allocation free-list manager for persistent GPU storage buffers (Pillar IX).
//!
//! Provides deterministic, zero-allocation sub-block management over a fixed-capacity
//! GPU buffer. Allows dynamically resized paths, segments, and draw objects to patch
//! into persistent GPU buffers via [`crate::SceneEdit`] without re-allocating or re-uploading
//! the entire scene.

#![forbid(unsafe_code)]

/// An allocated slice within a managed GPU buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allocation {
    /// Byte offset within the GPU buffer.
    pub offset: u64,
    /// Size of the allocated block in bytes.
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FreeBlock {
    offset: u64,
    size: u64,
}

#[inline]
fn align_up(val: u64, align: u64) -> u64 {
    if align <= 1 {
        val
    } else if align.is_power_of_two() {
        (val + align - 1) & !(align - 1)
    } else {
        val.div_ceil(align) * align
    }
}

/// A free-list allocator managing sub-allocations in persistent GPU storage buffers.
#[derive(Debug, Clone)]
pub struct FreeListAllocator {
    capacity: u64,
    alignment: u64,
    allocated_bytes: u64,
    free_blocks: Vec<FreeBlock>,
}

impl FreeListAllocator {
    /// Default GPU storage buffer offset alignment (e.g. 16 or 256 bytes for WGPU storage buffers).
    pub const DEFAULT_ALIGNMENT: u64 = 16;

    /// Creates a new allocator managing `capacity` bytes with the specified alignment.
    pub fn new(capacity: u64, alignment: u64) -> Self {
        let align = alignment.max(1);
        Self {
            capacity,
            alignment: align,
            allocated_bytes: 0,
            free_blocks: vec![FreeBlock {
                offset: 0,
                size: capacity,
            }],
        }
    }

    /// Allocates `size` bytes aligned to the allocator's alignment boundary.
    ///
    /// Returns `Some(Allocation)` on success, or `None` if no contiguous free block fits.
    pub fn allocate(&mut self, size: u64) -> Option<Allocation> {
        if size == 0 || size > self.capacity {
            return None;
        }

        // Align size to alignment boundary
        let aligned_size = align_up(size, self.alignment);

        // Find best-fit free block
        let mut best_idx = None;
        let mut best_excess = u64::MAX;

        for (i, block) in self.free_blocks.iter().enumerate() {
            // Compute aligned start inside this block
            let aligned_offset = align_up(block.offset, self.alignment);
            let waste = aligned_offset - block.offset;
            if block.size >= aligned_size + waste {
                let excess = block.size - (aligned_size + waste);
                if excess < best_excess {
                    best_excess = excess;
                    best_idx = Some(i);
                }
            }
        }

        let idx = best_idx?;
        let block = self.free_blocks.remove(idx);
        let aligned_offset = align_up(block.offset, self.alignment);
        let prefix_waste = aligned_offset - block.offset;

        // If there was prefix waste before aligned offset, put it back as a free block
        if prefix_waste > 0 {
            self.free_blocks.push(FreeBlock {
                offset: block.offset,
                size: prefix_waste,
            });
        }

        // If there is suffix space after the allocated chunk, put it back as a free block
        let allocated_end = aligned_offset + aligned_size;
        let block_end = block.offset + block.size;
        if block_end > allocated_end {
            self.free_blocks.push(FreeBlock {
                offset: allocated_end,
                size: block_end - allocated_end,
            });
        }

        // Keep free blocks sorted by offset and coalesce
        self.coalesce();

        self.allocated_bytes += aligned_size;

        Some(Allocation {
            offset: aligned_offset,
            size: aligned_size,
        })
    }

    /// Returns a previously allocated block to the free list and coalesces adjacent blocks.
    pub fn deallocate(&mut self, allocation: Allocation) {
        if allocation.size == 0 {
            return;
        }

        self.allocated_bytes = self.allocated_bytes.saturating_sub(allocation.size);

        self.free_blocks.push(FreeBlock {
            offset: allocation.offset,
            size: allocation.size,
        });

        self.coalesce();
    }

    /// Merges adjacent free blocks into contiguous chunks.
    fn coalesce(&mut self) {
        if self.free_blocks.len() <= 1 {
            return;
        }

        self.free_blocks.sort_by_key(|b| b.offset);

        let mut merged = Vec::with_capacity(self.free_blocks.len());
        let mut current = self.free_blocks[0];

        for block in &self.free_blocks[1..] {
            if current.offset + current.size == block.offset {
                // Adjacent: merge
                current.size += block.size;
            } else {
                merged.push(current);
                current = *block;
            }
        }
        merged.push(current);

        self.free_blocks = merged;
    }

    /// Total capacity managed by the allocator.
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Currently active allocated bytes.
    pub fn allocated_bytes(&self) -> u64 {
        self.allocated_bytes
    }

    /// Currently unallocated bytes.
    pub fn free_bytes(&self) -> u64 {
        self.capacity.saturating_sub(self.allocated_bytes)
    }

    /// Fragmentation metric between 0.0 (perfectly consolidated or empty) and 1.0 (heavily fragmented).
    pub fn fragmentation(&self) -> f32 {
        let free = self.free_bytes();
        if free == 0 {
            return 0.0;
        }
        let largest_free_block = self.free_blocks.iter().map(|b| b.size).max().unwrap_or(0);
        1.0 - (largest_free_block as f32 / free as f32)
    }

    /// Resets the allocator, reclaiming all space.
    pub fn reset(&mut self) {
        self.allocated_bytes = 0;
        self.free_blocks.clear();
        self.free_blocks.push(FreeBlock {
            offset: 0,
            size: self.capacity,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_allocator_has_full_capacity_available() {
        let alloc = FreeListAllocator::new(1024, 16);
        assert_eq!(alloc.capacity(), 1024);
        assert_eq!(alloc.allocated_bytes(), 0);
        assert_eq!(alloc.free_bytes(), 1024);
        assert_eq!(alloc.fragmentation(), 0.0);
    }

    #[test]
    fn allocates_aligned_slices() {
        let mut alloc = FreeListAllocator::new(1024, 16);
        let a1 = alloc.allocate(20).expect("should fit");
        assert_eq!(a1.offset, 0);
        assert_eq!(a1.size, 32); // aligned to 16
        assert_eq!(alloc.allocated_bytes(), 32);

        let a2 = alloc.allocate(16).expect("should fit");
        assert_eq!(a2.offset, 32);
        assert_eq!(a2.size, 16);
    }

    #[test]
    fn deallocate_coalesces_adjacent_blocks() {
        let mut alloc = FreeListAllocator::new(1024, 16);
        let a1 = alloc.allocate(64).unwrap();
        let a2 = alloc.allocate(64).unwrap();
        let a3 = alloc.allocate(64).unwrap();

        assert_eq!(alloc.allocated_bytes(), 192);

        // Free middle block
        alloc.deallocate(a2);
        assert_eq!(alloc.allocated_bytes(), 128);

        // Free first block -> should coalesce with middle
        alloc.deallocate(a1);
        assert_eq!(alloc.allocated_bytes(), 64);

        // Free third block -> should coalesce everything back to a single block
        alloc.deallocate(a3);
        assert_eq!(alloc.allocated_bytes(), 0);
        assert_eq!(alloc.free_bytes(), 1024);
        assert_eq!(alloc.fragmentation(), 0.0);
    }

    #[test]
    fn rejects_allocations_exceeding_capacity() {
        let mut alloc = FreeListAllocator::new(512, 16);
        assert!(alloc.allocate(1024).is_none());
        assert!(alloc.allocate(0).is_none());
    }

    #[test]
    fn best_fit_selection_minimizes_fragmentation() {
        let mut alloc = FreeListAllocator::new(1024, 16);
        let a1 = alloc.allocate(128).unwrap(); // 0..128
        let a2 = alloc.allocate(256).unwrap(); // 128..384
        let a3 = alloc.allocate(128).unwrap(); // 384..512
        let _a4 = alloc.allocate(512).unwrap(); // 512..1024

        // Free 128-byte hole at 0 and 128-byte hole at 384
        alloc.deallocate(a1);
        alloc.deallocate(a3);

        // A 128-byte alloc should fit into one of the 128-byte holes perfectly
        let a5 = alloc.allocate(128).unwrap();
        assert!(a5.offset == 0 || a5.offset == 384);

        alloc.deallocate(a2);
    }
}
