//! Bounded, reusable intermediate-texture accounting for effect passes.
//!
//! The renderer owns the actual GPU textures; this pool owns only the
//! byte accounting and reuse bookkeeping that keeps effect intermediates
//! within a hard budget (`phase.md` Phase 12: "reusable intermediate
//! texture pool with byte budget", "no effect can trigger unbounded
//! texture allocation").
//!
//! # Accounting model
//!
//! - `used` is the total bytes of textures currently *live* (acquired
//!   and not yet released).
//! - `reserved` is the total bytes of *idle* textures kept around in the
//!   free list for reuse.
//! - `used + reserved` never exceeds `budget`: an acquire that would
//!   push the combined footprint past the budget first evicts idle
//!   textures, and fails (allocating nothing) if it still would not fit.
//!
//! Releasing a texture moves its bytes from `used` to `reserved` (it
//! becomes reusable, not freed); acquiring a matching descriptor moves
//! them back with no new allocation. This is the fix for the original
//! accounting, where `release` never decremented `used`, so the pool's
//! footprint grew monotonically and the budget exhausted even under
//! perfect reuse.

use rustc_hash::FxHashMap;

/// Returns the accounted byte size of a texture.
///
/// # Why this is not `width * height * 4`
///
/// It was. `Descriptor` previously carried a `bytes_per_pixel: u8` that was
/// only ever set in tests, and only ever set to `4`. That is wrong for every
/// format Aura uses outside 8-bit colour, and wrong by exactly 2x for the
/// half-float intermediates HDR compositing depends on -- so a budget sized
/// against it would permit twice the memory it intended the moment HDR was
/// switched on.
///
/// # This is an estimate, and says so
///
/// `wgpu` cannot report committed device memory: drivers pad, tile, align and
/// sometimes alias allocations, none of which is observable through the API.
/// What this computes is the *accounted* size -- texel data summed across
/// every mip level, layer and sample. That is the right basis for a budget
/// because it is deterministic, monotonic in the things a caller controls,
/// and identical on every driver. Do not present it as VRAM usage.
#[must_use]
pub fn estimated_texture_bytes(descriptor: &wgpu::TextureDescriptor<'_>) -> u64 {
    let format = descriptor.format;
    let (block_width, block_height) = format.block_dimensions();

    // Depth and stencil formats report no size for the texture as a whole,
    // only per aspect, so sum the aspects that exist: a combined
    // depth-plus-stencil attachment costs both.
    let bytes_per_block = match format.block_copy_size(None) {
        Some(size) => u64::from(size),
        None => {
            let depth = format
                .block_copy_size(Some(wgpu::TextureAspect::DepthOnly))
                .unwrap_or(0);
            let stencil = format
                .block_copy_size(Some(wgpu::TextureAspect::StencilOnly))
                .unwrap_or(0);

            match u64::from(depth) + u64::from(stencil) {
                // An unrecognised format is better accounted pessimistically
                // than as free: a budget that under-counts bounds nothing.
                0 => 4,
                combined => combined,
            }
        }
    };

    let mut total = 0u64;

    for level in 0..descriptor.mip_level_count.max(1) {
        let size = descriptor.size.mip_level_size(level, descriptor.dimension);

        total += u64::from(size.width.div_ceil(block_width))
            * u64::from(size.height.div_ceil(block_height))
            * u64::from(size.depth_or_array_layers)
            * bytes_per_block;
    }

    total * u64::from(descriptor.sample_count.max(1))
}

/// A texture descriptor used for reusable effect intermediates.
///
/// Carries enough of the real texture's shape to price it: two textures that
/// differ only in format are different entries, because they cost different
/// amounts and cannot substitute for one another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Descriptor {
    /// Texture width in physical pixels.
    pub width: u32,
    /// Texture height in physical pixels.
    pub height: u32,
    /// The texture's format, which decides its cost per texel.
    pub format: wgpu::TextureFormat,
    /// Samples per texel. `1` for everything that is not an MSAA attachment.
    pub sample_count: u32,
    /// Mip levels. `1` for a render target.
    pub mip_level_count: u32,
    /// Array layers or depth slices.
    pub layers: u32,
}

impl Descriptor {
    /// A single-sample, single-mip, single-layer 2D texture.
    ///
    /// The shape every effect intermediate actually has.
    #[must_use]
    pub const fn new(width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        Self {
            width,
            height,
            format,
            sample_count: 1,
            mip_level_count: 1,
            layers: 1,
        }
    }

    /// Describes the texture `descriptor` would allocate.
    #[must_use]
    pub fn of(descriptor: &wgpu::TextureDescriptor<'_>) -> Self {
        Self {
            width: descriptor.size.width,
            height: descriptor.size.height,
            format: descriptor.format,
            sample_count: descriptor.sample_count,
            mip_level_count: descriptor.mip_level_count,
            layers: descriptor.size.depth_or_array_layers,
        }
    }

    /// Returns the accounted byte footprint of one such texture.
    #[must_use]
    pub fn bytes(self) -> u64 {
        estimated_texture_bytes(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: self.layers,
            },
            mip_level_count: self.mip_level_count,
            sample_count: self.sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    }
}

/// A byte-bounded intermediate-texture pool. Actual texture creation
/// remains owned by the renderer; this tracks only reuse and budget.
#[derive(Debug)]
pub struct Pool {
    budget: u64,
    used: u64,
    reserved: u64,
    free: FxHashMap<Descriptor, usize>,
}

impl Pool {
    /// Creates a pool with a hard byte budget.
    pub fn new(budget: u64) -> Self {
        Self {
            budget,
            used: 0,
            reserved: 0,
            free: FxHashMap::default(),
        }
    }

    /// Acquires a texture matching `descriptor`, reusing an idle one when
    /// possible. Returns `false` (allocating nothing) when honoring the
    /// request would exceed the budget even after evicting every idle
    /// texture -- the concrete "no unbounded allocation" guarantee.
    pub fn acquire(&mut self, descriptor: Descriptor) -> bool {
        let bytes = descriptor.bytes();

        // Reuse an idle texture of exactly this descriptor: move its
        // bytes from `reserved` back to `used`, no new allocation.
        if let Some(count) = self.free.get_mut(&descriptor)
            && *count > 0
        {
            *count -= 1;
            if *count == 0 {
                let _ = self.free.remove(&descriptor);
            }
            self.reserved = self.reserved.saturating_sub(bytes);
            self.used += bytes;
            return true;
        }

        // A single texture larger than the whole budget can never fit.
        if bytes > self.budget {
            return false;
        }

        // Make room by evicting idle (reserved) textures if the live +
        // idle footprint would otherwise exceed the budget.
        if self.used + self.reserved + bytes > self.budget {
            let ceiling = self.budget.saturating_sub(self.used + bytes);
            self.evict_until(ceiling);
        }

        if self.used + self.reserved + bytes > self.budget {
            return false;
        }

        self.used += bytes;
        true
    }

    /// Releases a texture back to the pool, keeping it idle and reusable
    /// (its bytes move from `used` to `reserved`) rather than freeing it.
    pub fn release(&mut self, descriptor: Descriptor) {
        let bytes = descriptor.bytes();
        // Guard against releasing more than was acquired.
        self.used = self.used.saturating_sub(bytes);
        self.reserved += bytes;
        *self.free.entry(descriptor).or_default() += 1;
    }

    /// Evicts idle textures (largest first) until reserved bytes fit
    /// within `target_reserved_ceiling`, or nothing idle remains.
    fn evict_until(&mut self, target_reserved_ceiling: u64) {
        while self.reserved > target_reserved_ceiling {
            // Evict the largest idle descriptor first for the biggest reclaim
            // per eviction. Chosen by byte cost explicitly: the free list is
            // keyed by shape, and the largest *texture* is not the largest
            // key in any ordering the key type could derive.
            let Some(descriptor) = self.free.keys().copied().max_by_key(|entry| entry.bytes())
            else {
                break;
            };

            let count = self.free.get_mut(&descriptor).expect("just found it");
            *count -= 1;
            if *count == 0 {
                let _ = self.free.remove(&descriptor);
            }
            self.reserved = self.reserved.saturating_sub(descriptor.bytes());
        }
    }

    /// Returns bytes currently live (acquired and not released).
    pub fn used_bytes(&self) -> u64 {
        self.used
    }

    /// Returns bytes currently idle in the free list (reusable).
    pub fn reserved_bytes(&self) -> u64 {
        self.reserved
    }

    /// Returns the total committed footprint (live + idle).
    pub fn footprint_bytes(&self) -> u64 {
        self.used + self.reserved
    }

    /// Returns the hard byte budget.
    pub fn budget_bytes(&self) -> u64 {
        self.budget
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(width: u32, height: u32) -> Descriptor {
        Descriptor::new(width, height, wgpu::TextureFormat::Rgba8Unorm)
    }

    /// The same shape in half float, which costs exactly twice as much.
    pub(super) fn half_float(width: u32, height: u32) -> Descriptor {
        Descriptor::new(width, height, wgpu::TextureFormat::Rgba16Float)
    }

    #[test]
    fn release_reclaims_used_bytes_into_reserved() {
        let one = descriptor(32, 32);
        let mut pool = Pool::new(one.bytes() * 4);

        assert!(pool.acquire(one));
        assert_eq!(pool.used_bytes(), one.bytes());
        assert_eq!(pool.reserved_bytes(), 0);

        pool.release(one);
        assert_eq!(pool.used_bytes(), 0);
        assert_eq!(pool.reserved_bytes(), one.bytes());
    }

    #[test]
    fn reuse_does_not_grow_the_footprint() {
        let one = descriptor(32, 32);
        let mut pool = Pool::new(one.bytes());

        for _ in 0..1_000 {
            assert!(pool.acquire(one));
            pool.release(one);
        }
        assert!(pool.footprint_bytes() <= pool.budget_bytes());
    }

    #[test]
    fn footprint_never_exceeds_budget_across_mixed_sizes() {
        let mut pool = Pool::new(descriptor(64, 64).bytes() * 2);
        let a = descriptor(64, 64);
        let b = descriptor(32, 32);

        assert!(pool.acquire(a));
        assert!(pool.acquire(b));
        assert!(pool.footprint_bytes() <= pool.budget_bytes());

        // A third live texture that overflows is refused, allocating
        // nothing.
        let before = pool.footprint_bytes();
        assert!(!pool.acquire(a));
        assert_eq!(pool.footprint_bytes(), before);
    }

    #[test]
    fn a_texture_over_the_whole_budget_is_refused() {
        let mut pool = Pool::new(1_000);
        assert!(!pool.acquire(descriptor(64, 64)));
        assert_eq!(pool.footprint_bytes(), 0);
    }

    #[test]
    fn eviction_makes_room_for_a_new_acquire_from_idle_textures() {
        // Budget for one 64x64. Acquire+release leaves it idle; acquiring
        // a same-footprint texture reuses it rather than failing.
        let d = descriptor(64, 64);
        let mut pool = Pool::new(d.bytes());
        assert!(pool.acquire(d));
        pool.release(d);
        assert!(pool.acquire(d));
        assert!(pool.footprint_bytes() <= pool.budget_bytes());
    }
}

#[cfg(test)]
mod format_tests {
    use super::tests::*;
    use super::*;

    #[test]
    fn a_half_float_intermediate_costs_twice_an_eight_bit_one() {
        // The accounting defect this module's `estimated_texture_bytes`
        // documentation describes: cost used to be `width * height * 4`
        // regardless of format, so a budget sized for 8-bit intermediates
        // admitted twice the memory it intended once HDR compositing moved
        // them to half float.
        let eight_bit = Descriptor::new(256, 256, wgpu::TextureFormat::Rgba8Unorm);
        let half = half_float(256, 256);

        assert_eq!(eight_bit.bytes(), 256 * 256 * 4);
        assert_eq!(half.bytes(), eight_bit.bytes() * 2);
    }

    #[test]
    fn the_budget_admits_half_as_many_half_float_intermediates() {
        let half = half_float(128, 128);
        let mut pool = Pool::new(half.bytes());

        assert!(pool.acquire(half));
        assert!(
            !pool.acquire(half),
            "a budget sized for one half-float intermediate admitted two"
        );
    }
}
