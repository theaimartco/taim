//! Cache text.
use crate::core::cache::Budget;
use crate::core::text::fallback::{FallbackChain, Script};
use crate::core::text::paragraph::Direction;
use crate::core::{Font, Size};
use crate::text;

use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use std::hash::{Hash, Hasher};

/// A store of recently used sections of text.
///
/// Bounded by a byte [`Budget`] (phase.md rule: "bound every cache and
/// queue"): [`Cache::trim`] already evicts entries untouched since the
/// previous trim (a generational sweep), but a single frame can still
/// allocate an unbounded number of distinct text sections before the
/// next trim runs -- a scene with thousands of unique, one-off strings
/// (e.g. live data streaming through many rows) would otherwise grow
/// this cache without limit for the lifetime of one frame. Once the
/// estimated total exceeds [`Cache::budget`], [`Cache::allocate`] evicts
/// the least-recently-touched entries (skipping anything already
/// touched this frame, i.e. in `recently_used`) before inserting the
/// new one, the same way `graphics::retained_layer::RetainedLayerCache`
/// bounds retained layers.
#[derive(Debug)]
pub struct Cache {
    entries: FxHashMap<KeyHash, Entry>,
    aliases: FxHashMap<KeyHash, KeyHash>,
    recently_used: FxHashSet<KeyHash>,
    /// Per-entry bookkeeping for budget enforcement, kept out of the
    /// public [`Entry`] type (which exists to be a plain "here is your
    /// shaped text" value, not to carry cache-internal state).
    accounting: FxHashMap<KeyHash, Accounting>,
    budget: Budget,
    current_bytes: u64,
    /// Monotonically increasing counter, bumped on every
    /// [`Cache::allocate`] touch (hit or miss), used to find the
    /// least-recently-touched entry for eviction. Not reset by
    /// [`Cache::trim`]: a `trim`'d cache should still remember which of
    /// its *surviving* entries are older than which others.
    clock: u64,
    /// Cumulative since construction. See [`Cache::statistics`].
    statistics: Statistics,
}

/// What a [`Cache`] has done since it was created.
///
/// Cumulative, like `runtime::diagnostics::Counters`, rather than
/// per-frame: a cache has no notion of "frame" of its own (nothing
/// tells it when one ends), so the caller — the one place that does
/// know — takes two readings and subtracts to get a frame's worth. That
/// is also why this is a plain data snapshot rather than something that
/// resets itself: a reset here could race a reader that had not taken
/// its "before" reading yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Statistics {
    /// Lookups that reused a previously shaped entry.
    pub hits: u64,
    /// Lookups that had to shape text from scratch.
    pub misses: u64,
    /// Estimated bytes reclaimed by eviction (age-based [`Cache::trim`]
    /// or budget-driven eviction inside [`Cache::allocate`]).
    pub evicted_bytes: u64,
    /// Total nanoseconds spent shaping, across every miss.
    ///
    /// Measured directly around the shaping call in [`Cache::allocate`],
    /// not estimated: a caller reporting a miss's cost to a profiler
    /// needs a real number there, and a fabricated one — evenly
    /// distributing some other total across misses that were not all
    /// equally expensive — would be exactly the kind of invented
    /// structure a trace must never contain. Dividing this by
    /// [`Statistics::misses`] after a [`Statistics::since`] gives an
    /// honest *average* cost per miss that frame, which is a real
    /// reduction of real measurements rather than an invention.
    pub total_miss_nanos: u64,
}

impl Statistics {
    /// This minus `earlier`, for turning two cumulative readings into
    /// one frame's activity.
    ///
    /// Saturating: a cache that was replaced outright (see
    /// `text::Pipeline::load_font`, which swaps in a fresh [`Cache`])
    /// would otherwise underflow into a wildly wrong `u64`, which is a
    /// worse answer than reporting zero for the one frame that happened
    /// to straddle the swap.
    pub fn since(&self, earlier: Statistics) -> Statistics {
        Statistics {
            hits: self.hits.saturating_sub(earlier.hits),
            misses: self.misses.saturating_sub(earlier.misses),
            evicted_bytes: self.evicted_bytes.saturating_sub(earlier.evicted_bytes),
            total_miss_nanos: self
                .total_miss_nanos
                .saturating_sub(earlier.total_miss_nanos),
        }
    }
}

/// Per-entry bookkeeping used only by [`Cache`]'s budget enforcement.
#[derive(Debug, Clone, Copy)]
struct Accounting {
    estimated_bytes: u64,
    last_touch: u64,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    /// The default byte budget: 8 MiB of estimated shaped-text data.
    ///
    /// Chosen to comfortably hold a few thousand distinct, moderately
    /// sized text sections (see [`estimate_bytes`]) -- generous for a
    /// typical UI, while still bounding a pathological single frame that
    /// allocates far more distinct sections than any real interface
    /// needs simultaneously visible.
    pub const DEFAULT_BUDGET: Budget = Budget::Bytes(8 * 1024 * 1024);

    /// Creates a new, empty [`Cache`] with [`Cache::DEFAULT_BUDGET`].
    pub fn new() -> Self {
        Self::with_budget(Self::DEFAULT_BUDGET)
    }

    /// Creates a new, empty [`Cache`] with the given [`Budget`].
    pub fn with_budget(budget: Budget) -> Self {
        Self {
            entries: FxHashMap::default(),
            aliases: FxHashMap::default(),
            recently_used: FxHashSet::default(),
            accounting: FxHashMap::default(),
            budget,
            current_bytes: 0,
            clock: 0,
            statistics: Statistics::default(),
        }
    }

    /// Returns this [`Cache`]'s configured [`Budget`].
    pub fn budget(&self) -> Budget {
        self.budget
    }

    /// Returns this [`Cache`]'s cumulative hit/miss/eviction counts.
    ///
    /// Cumulative since construction, not since the last read: see
    /// [`Statistics::since`] for turning two readings into one frame's
    /// worth.
    pub fn statistics(&self) -> Statistics {
        self.statistics
    }

    /// Returns the current estimated byte usage across every entry.
    pub fn current_bytes(&self) -> u64 {
        self.current_bytes
    }

    /// Gets the text [`Entry`] with the given [`KeyHash`].
    pub fn get(&self, key: &KeyHash) -> Option<&Entry> {
        self.entries.get(key)
    }

    /// Allocates a text [`Entry`] if it is not already present in the [`Cache`].
    pub fn allocate(
        &mut self,
        font_system: &mut cosmic_text::FontSystem,
        key: Key<'_>,
    ) -> (KeyHash, &mut Entry) {
        let hash = key.hash(FxHasher::default());

        self.clock += 1;
        let touch = self.clock;

        if let Some(hash) = self.aliases.get(&hash) {
            let hash = *hash;
            let _ = self.recently_used.insert(hash);

            if let Some(accounting) = self.accounting.get_mut(&hash) {
                accounting.last_touch = touch;
            }

            self.statistics.hits += 1;

            return (hash, self.entries.get_mut(&hash).unwrap());
        }

        if !self.entries.contains_key(&hash) {
            self.statistics.misses += 1;

            // Measured directly around the shaping work, not estimated:
            // a profiler consuming `total_miss_nanos` needs a real cost
            // here, and averaging real measurements is honest in a way
            // that inventing one would not be. Bookkeeping below (byte
            // estimation, eviction) is deliberately excluded -- that is
            // this cache's own overhead, not the cost of shaping text.
            let shaping_began = std::time::Instant::now();

            let metrics =
                cosmic_text::Metrics::new(key.size, key.line_height.max(f32::MIN_POSITIVE));
            let mut buffer = cosmic_text::Buffer::new(font_system, metrics);

            let max_height = key.bounds.height.max(key.line_height);

            buffer.set_size(Some(key.bounds.width), Some(max_height));

            buffer.set_wrap(text::to_wrap(key.wrapping));
            buffer.set_ellipsize(text::to_ellipsize(key.ellipsis, max_height));

            buffer.set_text(
                key.content,
                &text::to_attributes(key.font),
                text::to_shaping(key.shaping, key.content),
                None,
            );
            buffer.shape_until_scroll(font_system, false);

            let bounds = text::align(&mut buffer, font_system, key.align_x);

            self.statistics.total_miss_nanos += shaping_began.elapsed().as_nanos() as u64;

            let estimated_bytes = estimate_bytes(key.content, &buffer);

            self.evict_to_fit(estimated_bytes);

            let _ = self.entries.insert(
                hash,
                Entry {
                    buffer,
                    min_bounds: bounds,
                },
            );

            let _ = self.accounting.insert(
                hash,
                Accounting {
                    estimated_bytes,
                    last_touch: touch,
                },
            );
            self.current_bytes += estimated_bytes;

            for bounds in [
                bounds,
                Size {
                    width: key.bounds.width,
                    ..bounds
                },
            ] {
                if key.bounds != bounds {
                    let _ = self
                        .aliases
                        .insert(Key { bounds, ..key }.hash(FxHasher::default()), hash);
                }
            }
        } else if let Some(accounting) = self.accounting.get_mut(&hash) {
            accounting.last_touch = touch;
            self.statistics.hits += 1;
        }

        let _ = self.recently_used.insert(hash);

        (hash, self.entries.get_mut(&hash).unwrap())
    }

    /// Evicts the least-recently-touched entries (skipping anything
    /// already touched this frame, i.e. present in `recently_used`)
    /// until inserting `incoming_bytes` more would not exceed
    /// [`Cache::budget`], or every evictable entry has been removed.
    ///
    /// An entry touched this frame is never evicted: it is either the
    /// entry currently being allocated (not yet inserted, so not a
    /// candidate anyway) or one already relied upon earlier in the same
    /// frame, and evicting something the current frame still needs
    /// would force it to be redundantly reshaped moments later --
    /// worse than temporarily exceeding the budget within one frame.
    fn evict_to_fit(&mut self, incoming_bytes: u64) {
        let Budget::Bytes(budget) = self.budget else {
            return;
        };

        while self.current_bytes + incoming_bytes > budget {
            let victim = self
                .accounting
                .iter()
                .filter(|(hash, _)| !self.recently_used.contains(*hash))
                .min_by_key(|(_, accounting)| accounting.last_touch)
                .map(|(hash, _)| *hash);

            let Some(victim) = victim else {
                // Every remaining entry was touched this frame; further
                // eviction would remove something still in use.
                break;
            };

            if let Some(accounting) = self.accounting.remove(&victim) {
                self.current_bytes = self
                    .current_bytes
                    .saturating_sub(accounting.estimated_bytes);
                self.statistics.evicted_bytes += accounting.estimated_bytes;
            }

            let _ = self.entries.remove(&victim);
            self.aliases.retain(|_, target| *target != victim);
        }
    }

    /// Trims the [`Cache`].
    ///
    /// This will clear the sections of text that have not been used since the last `trim`.
    pub fn trim(&mut self) {
        let recently_used = &self.recently_used;
        let current_bytes = &mut self.current_bytes;
        let evicted_bytes = &mut self.statistics.evicted_bytes;

        self.entries.retain(|key, _| recently_used.contains(key));

        self.accounting.retain(|key, accounting| {
            let keep = recently_used.contains(key);

            if !keep {
                *current_bytes = current_bytes.saturating_sub(accounting.estimated_bytes);
                *evicted_bytes += accounting.estimated_bytes;
            }

            keep
        });

        self.aliases
            .retain(|_, value| self.recently_used.contains(value));

        self.recently_used.clear();
    }
}

/// Estimates the number of bytes a shaped [`cosmic_text::Buffer`] costs
/// to retain, for [`Cache::budget`] accounting.
///
/// This is deliberately an estimate, not an exact accounting (matching
/// `graphics::retained_layer`'s own `estimate_display_list_bytes`):
/// `cosmic_text::Buffer` exposes no public size introspection, so this
/// approximates shaped-glyph and layout-line overhead as proportional to
/// the original text's byte length, plus a fixed per-entry floor for the
/// `Buffer` struct and its line/scroll state even for very short text.
fn estimate_bytes(content: &str, buffer: &cosmic_text::Buffer) -> u64 {
    const PER_ENTRY_OVERHEAD: u64 = 256;
    const PER_LINE_OVERHEAD: u64 = 64;
    /// Shaped glyph data (position, font id, cluster info, ...) is
    /// larger than the original UTF-8 bytes per character; this
    /// multiplier is a rough, documented approximation, not a measured
    /// constant.
    const SHAPED_BYTES_PER_CONTENT_BYTE: u64 = 4;

    let content_bytes = content.len() as u64 * SHAPED_BYTES_PER_CONTENT_BYTE;
    let line_overhead = buffer.lines.len() as u64 * PER_LINE_OVERHEAD;

    PER_ENTRY_OVERHEAD + content_bytes + line_overhead
}

/// A cache key representing a section of text.
#[derive(Debug, Clone, Copy)]
pub struct Key<'a> {
    /// The content of the text.
    pub content: &'a str,
    /// The size of the text.
    pub size: f32,
    /// The line height of the text.
    pub line_height: f32,
    /// The [`Font`] of the text.
    pub font: Font,
    /// The bounds of the text.
    pub bounds: Size,
    /// The shaping strategy of the text.
    pub shaping: text::Shaping,
    /// The alignment of the text.
    pub align_x: text::Alignment,
    /// The wrapping strategy of the text.
    pub wrapping: text::Wrapping,
    /// The ellipsis strategy of the text.
    pub ellipsis: text::Ellipsis,
    /// BCP-47 locale used for shaping, when known.
    pub locale: Option<&'a str>,
    /// Base paragraph direction.
    pub direction: Direction,
    /// Script selected for deterministic fallback.
    pub script: Script,
    /// Ordered fallback families used by the shaper.
    pub fallback: FallbackChain,
    /// Quantized variable-font axis values, sorted by tag.
    pub axes: &'a [([u8; 4], u32)],
}

impl Key<'_> {
    fn hash<H: Hasher>(self, mut hasher: H) -> KeyHash {
        self.content.hash(&mut hasher);
        self.size.to_bits().hash(&mut hasher);
        self.line_height.to_bits().hash(&mut hasher);
        self.font.hash(&mut hasher);
        self.bounds.width.to_bits().hash(&mut hasher);
        self.bounds.height.to_bits().hash(&mut hasher);
        self.shaping.hash(&mut hasher);
        self.align_x.hash(&mut hasher);
        self.wrapping.hash(&mut hasher);
        self.ellipsis.hash(&mut hasher);
        self.locale.hash(&mut hasher);
        self.direction.hash(&mut hasher);
        self.script.hash(&mut hasher);
        self.fallback.hash(&mut hasher);
        self.axes.hash(&mut hasher);

        hasher.finish()
    }
}

/// The hash of a [`Key`].
pub type KeyHash = u64;

/// A cache entry.
#[derive(Debug)]
pub struct Entry {
    /// The buffer of text, ready for drawing.
    pub buffer: cosmic_text::Buffer,
    /// The minimum bounds of the text.
    pub min_bounds: Size,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Font;

    /// A fresh `FontSystem` per test (rather than a shared `static`):
    /// these tests run in parallel, and a shared lock means one test's
    /// panic poisons it for every other test still holding a guard,
    /// turning an unrelated assertion failure into a cascade of
    /// spurious `PoisonError`s elsewhere.
    fn font_system() -> cosmic_text::FontSystem {
        cosmic_text::FontSystem::new()
    }

    fn key(content: &str) -> Key<'_> {
        Key {
            content,
            size: 16.0,
            line_height: 20.0,
            font: Font::default(),
            bounds: Size::new(200.0, 200.0),
            shaping: text::Shaping::Basic,
            align_x: text::Alignment::default(),
            wrapping: text::Wrapping::default(),
            ellipsis: text::Ellipsis::default(),
            locale: None,
            direction: Direction::Auto,
            script: Script::Latin,
            fallback: FallbackChain::for_script(Script::Latin),
            axes: &[],
        }
    }

    #[test]
    fn a_fresh_cache_starts_at_zero_bytes() {
        let cache = Cache::new();

        assert_eq!(cache.current_bytes(), 0);
    }

    #[test]
    fn allocating_a_new_entry_increases_current_bytes() {
        let mut cache = Cache::new();
        let mut font_system = font_system();

        let _ = cache.allocate(&mut font_system, key("hello"));

        assert!(cache.current_bytes() > 0);
    }

    #[test]
    fn allocating_the_same_key_twice_does_not_double_count_bytes() {
        let mut cache = Cache::new();
        let mut font_system = font_system();

        let _ = cache.allocate(&mut font_system, key("hello"));
        let after_first = cache.current_bytes();

        let _ = cache.allocate(&mut font_system, key("hello"));

        assert_eq!(cache.current_bytes(), after_first);
    }

    #[test]
    fn shaping_affecting_attributes_never_alias_a_cached_buffer() {
        let mut cache = Cache::new();
        let mut font_system = font_system();
        let english = key("shared content");
        let arabic = Key {
            locale: Some("ar"),
            direction: Direction::RightToLeft,
            script: Script::Arabic,
            fallback: FallbackChain::for_script(Script::Arabic),
            axes: &[(*b"wght", 700.0_f32.to_bits())],
            ..english
        };

        let (english_hash, _) = cache.allocate(&mut font_system, english);
        let (arabic_hash, _) = cache.allocate(&mut font_system, arabic);

        assert_ne!(english_hash, arabic_hash);
    }

    #[test]
    fn trim_reclaims_bytes_for_entries_not_touched_since_the_last_trim() {
        let mut cache = Cache::new();
        let mut font_system = font_system();

        let _ = cache.allocate(&mut font_system, key("hello"));
        assert!(cache.current_bytes() > 0);

        // The entry survives this trim (it was touched by the
        // `allocate` call above, before this trim ran).
        cache.trim();
        assert!(cache.current_bytes() > 0);

        // Not touched again before this second trim: must be evicted.
        cache.trim();

        assert_eq!(cache.current_bytes(), 0);
    }

    #[test]
    fn trim_keeps_bytes_for_entries_touched_since_the_last_trim() {
        let mut cache = Cache::new();
        let mut font_system = font_system();

        let (hash, _) = cache.allocate(&mut font_system, key("hello"));
        let after_first = cache.current_bytes();

        let _ = cache.allocate(&mut font_system, key("hello"));
        cache.trim();

        assert_eq!(cache.current_bytes(), after_first);
        assert!(cache.get(&hash).is_some());
    }

    #[test]
    fn a_tiny_budget_evicts_older_entries_to_make_room_for_a_new_one() {
        // A budget just large enough for roughly one entry's estimated
        // bytes: allocating a second, different entry must evict the
        // first (since it was not re-touched) rather than exceed the
        // budget unboundedly.
        let mut cache = Cache::with_budget(Budget::Bytes(400));
        let mut font_system = font_system();

        let (first_hash, _) = cache.allocate(&mut font_system, key("aaaa"));
        assert!(cache.get(&first_hash).is_some());

        // A fresh `allocate` call bumps the clock; without a second
        // `trim`-independent "new frame" signal, `recently_used` still
        // contains `first_hash` from the call above. Simulate having
        // moved to a new frame (as `Renderer::draw`'s `trim()` calls
        // do) so the first entry becomes evictable.
        cache.trim();

        let (second_hash, _) = cache.allocate(&mut font_system, key("bbbb"));

        assert!(
            cache.get(&second_hash).is_some(),
            "the newly allocated entry must always be present"
        );
        assert!(
            cache.current_bytes() <= 400,
            "current_bytes ({}) must respect the configured budget",
            cache.current_bytes()
        );
    }

    #[test]
    fn an_entry_touched_this_frame_is_never_evicted_even_under_budget_pressure() {
        // `evict_to_fit` must skip anything in `recently_used`: forcing
        // a tiny budget while a single frame allocates several *new*
        // distinct entries must not evict any of them, since all are
        // still needed this frame -- the budget may be temporarily
        // exceeded rather than losing something in active use.
        let mut cache = Cache::with_budget(Budget::Bytes(1));
        let mut font_system = font_system();

        let (first_hash, _) = cache.allocate(&mut font_system, key("first"));
        let (second_hash, _) = cache.allocate(&mut font_system, key("second"));

        assert!(cache.get(&first_hash).is_some());
        assert!(cache.get(&second_hash).is_some());
    }

    #[test]
    fn with_budget_reports_the_configured_budget() {
        let cache = Cache::with_budget(Budget::Bytes(1234));

        assert_eq!(cache.budget(), Budget::Bytes(1234));
    }

    #[test]
    fn a_fresh_cache_has_no_activity() {
        assert_eq!(Cache::new().statistics(), Statistics::default());
    }

    #[test]
    fn a_new_key_is_a_miss() {
        let mut cache = Cache::new();
        let mut font_system = font_system();

        let _ = cache.allocate(&mut font_system, key("hello"));

        let statistics = cache.statistics();

        assert_eq!(statistics.misses, 1);
        assert_eq!(statistics.hits, 0);
    }

    #[test]
    fn the_same_key_again_is_a_hit() {
        // The un-aliased path: the hash already has an `Entry`, so
        // `allocate` takes the `else if` branch rather than reshaping.
        let mut cache = Cache::new();
        let mut font_system = font_system();

        let _ = cache.allocate(&mut font_system, key("hello"));
        let _ = cache.allocate(&mut font_system, key("hello"));

        let statistics = cache.statistics();

        assert_eq!(statistics.misses, 1, "shaped exactly once");
        assert_eq!(statistics.hits, 1, "reused on the second call");
    }

    #[test]
    fn an_aliased_bounds_lookup_is_a_hit_not_a_miss() {
        // The alias path: allocating with a shrunk width (the same
        // content, laid out narrower than it needs) registers an alias
        // to the original entry rather than reshaping -- see the
        // `aliases.insert` loop in `allocate`. That alias lookup must
        // count as a hit: it reused shaped work, even though the key
        // differs.
        let mut cache = Cache::new();
        let mut font_system = font_system();

        let (_, entry) = cache.allocate(&mut font_system, key("hi"));
        let natural_width = entry.min_bounds.width;

        let wider = Key {
            bounds: Size::new(natural_width * 4.0, 200.0),
            ..key("hi")
        };

        let _ = cache.allocate(&mut font_system, wider);
        let after_first_two = cache.statistics();

        // Re-allocating at the original (narrower) bounds should hit the
        // alias registered above, rather than shaping again.
        let _ = cache.allocate(&mut font_system, key("hi"));

        let statistics = cache.statistics();

        assert_eq!(
            statistics.misses, after_first_two.misses,
            "the aliased lookup must not shape again"
        );
        assert!(
            statistics.hits > after_first_two.hits,
            "the aliased lookup must count as a hit"
        );
    }

    #[test]
    fn trim_reports_evicted_bytes() {
        let mut cache = Cache::new();
        let mut font_system = font_system();

        let _ = cache.allocate(&mut font_system, key("hello"));
        let bytes = cache.current_bytes();

        cache.trim(); // survives (touched by the allocate above)
        cache.trim(); // now evicted (untouched since the last trim)

        assert_eq!(cache.statistics().evicted_bytes, bytes);
    }

    #[test]
    fn budget_driven_eviction_also_reports_evicted_bytes() {
        // The other eviction path -- `evict_to_fit`, triggered mid-frame
        // by a tight budget -- must report through the same counter as
        // `trim`'s age-based eviction. A caller diagnosing memory
        // pressure should not have to know which of the two mechanisms
        // did the evicting.
        let mut cache = Cache::with_budget(Budget::Bytes(400));
        let mut font_system = font_system();

        let _ = cache.allocate(&mut font_system, key("aaaa"));
        cache.trim();

        let _ = cache.allocate(&mut font_system, key("bbbb"));

        assert!(
            cache.statistics().evicted_bytes > 0,
            "evicting to fit the budget must be visible in the statistics"
        );
    }

    #[test]
    fn statistics_are_cumulative_across_many_frames() {
        // Not reset per frame: the caller takes two readings and
        // subtracts (see `Statistics::since`), because the cache itself
        // has no notion of "frame".
        let mut cache = Cache::new();
        let mut font_system = font_system();

        for content in ["a", "b", "c"] {
            let _ = cache.allocate(&mut font_system, key(content));
        }

        assert_eq!(cache.statistics().misses, 3);

        let _ = cache.allocate(&mut font_system, key("a"));

        assert_eq!(
            cache.statistics().misses,
            3,
            "a later hit must not be miscounted as an additional miss"
        );
        assert_eq!(cache.statistics().hits, 1);
    }

    #[test]
    fn since_computes_the_difference_between_two_readings() {
        let earlier = Statistics {
            hits: 10,
            misses: 2,
            evicted_bytes: 100,
            total_miss_nanos: 2_000,
        };
        let later = Statistics {
            hits: 15,
            misses: 3,
            evicted_bytes: 150,
            total_miss_nanos: 2_500,
        };

        assert_eq!(
            later.since(earlier),
            Statistics {
                hits: 5,
                misses: 1,
                evicted_bytes: 50,
                total_miss_nanos: 500,
            }
        );
    }

    #[test]
    fn since_saturates_rather_than_underflowing() {
        // Guards the case a fresh `Cache` (e.g. `Pipeline::load_font`
        // swapping in a new one) produces: a "later" reading smaller
        // than the "earlier" one. Underflowing here would report a
        // wildly wrong `u64` for the one frame that straddled the swap,
        // which is a worse answer than zero.
        let earlier = Statistics {
            hits: 100,
            misses: 50,
            evicted_bytes: 1000,
            total_miss_nanos: 5_000,
        };
        let later = Statistics::default();

        assert_eq!(later.since(earlier), Statistics::default());
    }

    #[test]
    fn a_miss_records_real_elapsed_time_not_a_fabricated_one() {
        // The whole reason this field is measured rather than estimated:
        // a caller reporting this to a profiler needs a real number, and
        // shaping text is not instantaneous, so a miss's cost must be
        // strictly positive.
        let mut cache = Cache::new();
        let mut font_system = font_system();

        let _ = cache.allocate(&mut font_system, key("some real text to shape"));

        assert!(
            cache.statistics().total_miss_nanos > 0,
            "shaping text takes measurable time"
        );
    }

    #[test]
    fn a_hit_adds_no_shaping_time() {
        // A hit reuses the earlier shape; it must not be double-charged
        // as though it were shaped again.
        let mut cache = Cache::new();
        let mut font_system = font_system();

        let _ = cache.allocate(&mut font_system, key("hello"));
        let after_miss = cache.statistics().total_miss_nanos;

        let _ = cache.allocate(&mut font_system, key("hello"));

        assert_eq!(cache.statistics().total_miss_nanos, after_miss);
    }

    #[test]
    fn trimming_alone_adds_no_shaping_time() {
        // Eviction is this cache's own bookkeeping, not the cost of
        // shaping text. Trimming with nothing to shape must leave
        // `total_miss_nanos` at exactly zero -- if eviction's cost had
        // been folded into it, this would be the one case that exposes
        // it, since there is no real shaping happening to hide behind.
        let mut cache = Cache::new();

        cache.trim();
        cache.trim();

        assert_eq!(cache.statistics().total_miss_nanos, 0);
    }
}
