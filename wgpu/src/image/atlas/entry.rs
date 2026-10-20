//! TAIM GPU image residency contracts.
//!
//! Each image entry can occupy one contiguous allocation or a sequence
//! of independently packed tiles. Before staging data into a texture,
//! validate that fragmented entries cover every source pixel exactly once.
//! This avoids out-of-bounds source reads and incomplete uploads without
//! allocating a per-pixel coverage bitmap.

use crate::core::Size;
use crate::image::atlas;

/// Device allocation(s) backing a logical source image.
#[derive(Debug)]
pub enum Entry {
    /// Source fits in a single atlas region.
    Contiguous(atlas::Allocation),
    /// An oversized source is cut into raster-ordered atlas tiles.
    Fragmented {
        /// Full logical source dimensions, independent of atlas layer size.
        size: Size<u32>,
        /// Tiles ordered left-to-right and top-to-bottom.
        fragments: Vec<Fragment>,
    },
}

impl Entry {
    /// Logical dimensions of this image, including any fragment boundaries.
    #[cfg(feature = "image")]
    pub fn size(&self) -> Size<u32> {
        match self {
            Self::Contiguous(allocation) => allocation.size(),
            Self::Fragmented { size, .. } => *size,
        }
    }

    /// Validate source coverage before staging GPU copies.
    ///
    /// Full source coverage is checked using a row-major tiling invariant.
    /// It is O(number of fragments) with constant extra memory. Every tile
    /// must have positive area, fit within the declared source bounds,
    /// continue exactly where its predecessor ended, and maintain constant
    /// row height. A completed row advances Y; finishing all rows must land
    /// precisely at the source image height.
    ///
    /// The packing allocator creates entries with this ordering, so the
    /// check is both cheap and stricter than a total-area-only check (which
    /// would miss overlapping fragments plus uncovered holes).
    #[must_use]
    pub fn valid_source_coverage(&self) -> bool {
        match self {
            Self::Contiguous(allocation) => {
                let size = allocation.size();
                size.width > 0 && size.height > 0
            }
            Self::Fragmented { size, fragments } => {
                if size.width == 0 || size.height == 0 || fragments.is_empty() {
                    return false;
                }

                let mut x = 0_u32;
                let mut y = 0_u32;
                let mut row_height = 0_u32;

                for fragment in fragments {
                    let tile = fragment.allocation.size();
                    if (x, y) != fragment.position
                        || tile.width == 0
                        || tile.height == 0
                        || y >= size.height
                    {
                        return false;
                    }

                    if x == 0 {
                        row_height = tile.height;
                    } else if tile.height != row_height {
                        return false;
                    }

                    let Some(next_x) = x.checked_add(tile.width) else {
                        return false;
                    };
                    let Some(next_y) = y.checked_add(tile.height) else {
                        return false;
                    };
                    if next_x > size.width || next_y > size.height {
                        return false;
                    }

                    if next_x == size.width {
                        x = 0;
                        y += row_height;
                    } else {
                        x = next_x;
                    }
                }

                x == 0 && y == size.height
            }
        }
    }
}

/// Logical source-tile origin plus its independent GPU allocation.
#[derive(Debug)]
pub struct Fragment {
    /// X/Y offset inside the original source pixel buffer.
    pub position: (u32, u32),
    /// GPU texture layer region containing the tile.
    pub allocation: atlas::Allocation,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(x: u32, y: u32, side: u32, layer: usize) -> Fragment {
        Fragment {
            position: (x, y),
            allocation: atlas::Allocation::Full { layer, size: side },
        }
    }

    #[test]
    fn valid_square_tiling_is_accepted() {
        let entry = Entry::Fragmented {
            size: Size::new(4, 4),
            fragments: vec![
                tile(0, 0, 2, 0),
                tile(2, 0, 2, 1),
                tile(0, 2, 2, 2),
                tile(2, 2, 2, 3),
            ],
        };
        assert!(entry.valid_source_coverage());
    }

    #[test]
    fn uncovered_rows_or_out_of_order_tiles_are_rejected() {
        let missing = Entry::Fragmented {
            size: Size::new(4, 4),
            fragments: vec![tile(0, 0, 2, 0), tile(2, 0, 2, 1)],
        };
        assert!(!missing.valid_source_coverage());
        let wrong_order = Entry::Fragmented {
            size: Size::new(4, 2),
            fragments: vec![tile(2, 0, 2, 0), tile(0, 0, 2, 1)],
        };
        assert!(!wrong_order.valid_source_coverage());
    }

    #[test]
    fn overlaps_out_of_bounds_and_nonrectangular_rows_are_rejected() {
        let overlap = Entry::Fragmented {
            size: Size::new(4, 2),
            fragments: vec![tile(0, 0, 2, 0), tile(0, 0, 2, 1)],
        };
        assert!(!overlap.valid_source_coverage());
        let exceeded = Entry::Fragmented {
            size: Size::new(3, 2),
            fragments: vec![tile(0, 0, 2, 0), tile(2, 0, 2, 1)],
        };
        assert!(!exceeded.valid_source_coverage());
        let nonuniform = Entry::Fragmented {
            size: Size::new(4, 3),
            fragments: vec![tile(0, 0, 2, 0), tile(2, 0, 3, 1)],
        };
        assert!(!nonuniform.valid_source_coverage());
    }

    #[test]
    fn trivial_and_empty_sources() {
        assert!(Entry::Contiguous(atlas::Allocation::Full { layer: 0, size: 8 })
            .valid_source_coverage());
        let empty = Entry::Fragmented {
            size: Size::new(0, 0),
            fragments: vec![],
        };
        assert!(!empty.valid_source_coverage());
    }
}
