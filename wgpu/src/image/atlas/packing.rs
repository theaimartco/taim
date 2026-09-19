use super::allocator::Allocator;
use super::entry::{self, Entry};
use super::{Allocation, Layer};

use crate::core::Size;

/// CPU-side atlas placement, isolated from physical texture capacity.
#[derive(Debug)]
pub(super) struct Packing {
    side: u32,
    maximum_layers: usize,
    layers: Vec<Layer>,
}

impl Packing {
    pub fn new(side: u32, initial_layers: u32, maximum_layers: u32) -> Self {
        Self {
            side,
            maximum_layers: maximum_layers as usize,
            layers: (0..initial_layers).map(|_| Layer::Empty).collect(),
        }
    }

    pub fn allocate(&mut self, width: u32, height: u32) -> Option<Entry> {
        if width == 0 || height == 0 {
            return None;
        }

        if width <= self.side && height <= self.side {
            return self.allocate_tile(width, height).map(Entry::Contiguous);
        }

        self.allocate_fragmented(width, height)
    }

    pub fn remove(&mut self, entry: &Entry) {
        match entry {
            Entry::Contiguous(allocation) => self.release(allocation),
            Entry::Fragmented { fragments, .. } => {
                for fragment in fragments {
                    self.release(&fragment.allocation);
                }
            }
        }
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn busy_layers(&self) -> usize {
        self.layers.iter().filter(|layer| !layer.is_empty()).count()
    }

    pub fn allocations(&self) -> usize {
        self.layers.iter().map(Layer::allocations).sum()
    }

    fn allocate_fragmented(&mut self, width: u32, height: u32) -> Option<Entry> {
        let columns = width.div_ceil(self.side);
        let rows = height.div_ceil(self.side);
        let tile_count = u64::from(columns).checked_mul(u64::from(rows))?;
        let reserve = usize::try_from(tile_count)
            .unwrap_or(self.maximum_layers)
            .min(self.maximum_layers);
        let original_layer_count = self.layers.len();
        let mut fragments = Vec::with_capacity(reserve);

        let mut y = 0;
        while y < height {
            let tile_height = (height - y).min(self.side);
            let mut x = 0;

            while x < width {
                let tile_width = (width - x).min(self.side);
                let Some(allocation) = self.allocate_tile(tile_width, tile_height) else {
                    self.rollback(&fragments, original_layer_count);
                    return None;
                };

                fragments.push(entry::Fragment {
                    position: (x, y),
                    allocation,
                });
                x += tile_width;
            }
            y += tile_height;
        }

        Some(Entry::Fragmented {
            size: Size::new(width, height),
            fragments,
        })
    }

    fn allocate_tile(&mut self, width: u32, height: u32) -> Option<Allocation> {
        if width == self.side && height == self.side {
            if let Some((index, layer)) = self
                .layers
                .iter_mut()
                .enumerate()
                .find(|(_, layer)| layer.is_empty())
            {
                *layer = Layer::Full;
                return Some(Allocation::Full {
                    layer: index,
                    size: self.side,
                });
            }

            if self.layers.len() == self.maximum_layers {
                return None;
            }

            self.layers.push(Layer::Full);
            return Some(Allocation::Full {
                layer: self.layers.len() - 1,
                size: self.side,
            });
        }

        for (index, layer) in self.layers.iter_mut().enumerate() {
            match layer {
                Layer::Empty => {
                    let mut allocator = Allocator::new(self.side);
                    let region = allocator.allocate(width, height)?;
                    *layer = Layer::Busy(allocator);
                    return Some(Allocation::Partial {
                        region,
                        layer: index,
                        atlas_size: self.side,
                    });
                }
                Layer::Busy(allocator) => {
                    if let Some(region) = allocator.allocate(width, height) {
                        return Some(Allocation::Partial {
                            region,
                            layer: index,
                            atlas_size: self.side,
                        });
                    }
                }
                Layer::Full => {}
            }
        }

        if self.layers.len() == self.maximum_layers {
            return None;
        }

        let mut allocator = Allocator::new(self.side);
        let region = allocator.allocate(width, height)?;
        self.layers.push(Layer::Busy(allocator));
        Some(Allocation::Partial {
            region,
            layer: self.layers.len() - 1,
            atlas_size: self.side,
        })
    }

    fn rollback(&mut self, fragments: &[entry::Fragment], original_layer_count: usize) {
        for fragment in fragments.iter().rev() {
            self.release(&fragment.allocation);
        }
        self.layers.truncate(original_layer_count);
    }

    fn release(&mut self, allocation: &Allocation) {
        let Some(layer) = self.layers.get_mut(allocation.layer()) else {
            return;
        };

        match allocation {
            Allocation::Full { .. } => {
                if matches!(layer, Layer::Full) {
                    *layer = Layer::Empty;
                }
            }
            Allocation::Partial { region, .. } => {
                if let Layer::Busy(allocator) = layer {
                    allocator.deallocate(region);
                    if allocator.is_empty() {
                        *layer = Layer::Empty;
                    }
                }
            }
        }
    }
}
