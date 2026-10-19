//! TAIM-Native Normalized Event Pump and FrameGate.
//!
//! Replaces inherited event loop polling with an explicit dirty-flag
//! state machine, event routing, redraw request coalescing, and per-window
//! lifecycle tracking. Guarantees zero framework wakeups when idle.

use crate::core::window;

use crate::core::time::Instant;
use rustc_hash::FxHashMap;

/// Explicit dirty flags tracking frame pipeline invalidations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DirtyFlags(pub u16);

impl DirtyFlags {
    /// The window is completely up-to-date.
    pub const CLEAN: Self = Self(0);
    /// Messages or application state updated; requires update pass.
    pub const UPDATE_DIRTY: Self = Self(1 << 0);
    /// Widget layout or constraints changed; requires layout re-computation.
    pub const LAYOUT_DIRTY: Self = Self(1 << 1);
    /// Display list or geometry invalidated; requires re-recording and paint.
    pub const PAINT_DIRTY: Self = Self(1 << 2);
    /// Accessibility or semantics tree invalidated; requires delta propagation.
    pub const SEMANTICS_DIRTY: Self = Self(1 << 3);
    /// Swapchain buffer pending presentation.
    pub const PRESENT_DIRTY: Self = Self(1 << 4);
    /// Continuous animation active; forces next-frame scheduling.
    pub const ANIMATING: Self = Self(1 << 5);

    /// Returns `true` if all flags in `other` are contained in `self`.
    #[inline]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns `true` if any flags in `other` intersect with `self`.
    #[inline]
    pub const fn intersects(&self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    /// Returns `true` if no flags are set.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for DirtyFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for DirtyFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for DirtyFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::BitAndAssign for DirtyFlags {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl std::ops::Not for DirtyFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

/// Determine which frame work follows a batch of application messages.
///
/// The older Program contract continues to redraw automatically. Native
/// TAIM intent applications explicitly report layout/paint/animation changes.
/// This function is pure so those semantics are exercised without a GPU.
pub(crate) fn after_message_batch(
    native_invalidation: bool,
    ui_stale: bool,
    intent: crate::program::StateDelta,
) -> DirtyFlags {
    let mut dirty = if !native_invalidation || ui_stale {
        DirtyFlags::PAINT_DIRTY
    } else {
        DirtyFlags::CLEAN
    };

    if native_invalidation {
        if intent.layout_dirty {
            dirty |= DirtyFlags::LAYOUT_DIRTY | DirtyFlags::PAINT_DIRTY;
        } else if intent.paint_dirty {
            dirty |= DirtyFlags::PAINT_DIRTY;
        }
        if intent.frame != crate::program::FrameRequest::None {
            dirty |= DirtyFlags::PAINT_DIRTY;
        }
    }
    dirty
}

#[cfg(test)]
mod native_message_tests {
    use super::*;
    use crate::program::{FrameRequest, StateDelta};

    #[test]
    fn untouched_native_message_does_not_dirty_a_frame() {
        assert_eq!(after_message_batch(true, false, StateDelta::clean()), DirtyFlags::CLEAN);
    }

    #[test]
    fn legacy_message_keeps_automatic_redraw() {
        assert_eq!(
            after_message_batch(false, false, StateDelta::clean()),
            DirtyFlags::PAINT_DIRTY
        );
    }

    #[test]
    fn native_paint_does_not_invalidate_layout() {
        let flags = after_message_batch(true, false, StateDelta::redraw());
        assert!(flags.contains(DirtyFlags::PAINT_DIRTY));
        assert!(!flags.intersects(DirtyFlags::LAYOUT_DIRTY));
    }

    #[test]
    fn native_layout_invalidates_layout_and_paint() {
        let flags = after_message_batch(true, false, StateDelta::relayout());
        assert!(flags.contains(DirtyFlags::LAYOUT_DIRTY | DirtyFlags::PAINT_DIRTY));
    }

    #[test]
    fn stale_ui_requires_paint_even_for_clean_native_message() {
        assert!(after_message_batch(true, true, StateDelta::clean())
            .intersects(DirtyFlags::PAINT_DIRTY));
    }

    #[test]
    fn animation_request_produces_a_frame_without_other_changes() {
        let flags = after_message_batch(
            true,
            false,
            StateDelta {
                frame: FrameRequest::Animate,
                ..StateDelta::clean()
            },
        );
        assert!(flags.intersects(DirtyFlags::PAINT_DIRTY));
    }
}

/// Normalized platform event emitted from the OS layer.
#[derive(Debug, Clone, PartialEq)]
pub enum PlatformEvent {
    /// Window input focus changed.
    FocusChanged(bool),
    /// Window visibility / occlusion changed.
    OcclusionChanged(bool),
    /// Whether the drawable surface currently has a zero width or height.
    SurfaceSizeChanged {
        /// `true` when either physical dimension is zero.
        zero_sized: bool,
    },
    /// Window physical scale factor changed.
    ScaleFactorChanged(f32),
    /// Cursor moved to coordinates within window.
    CursorMoved {
        /// X-coordinate in logical points.
        x: f32,
        /// Y-coordinate in logical points.
        y: f32,
    },
    /// Keyboard key interaction.
    KeyInput {
        /// Platform-specific key code.
        key_code: u32,
        /// Whether the key was pressed (`true`) or released (`false`).
        is_pressed: bool,
    },
    /// System theme preference changed.
    ThemeChanged,
    /// OS requested window redraw.
    RedrawRequested,
}

/// Normalized TAIM internal event routed to widgets.
#[derive(Debug, Clone, PartialEq)]
pub enum TaimEvent {
    /// Routed interaction event targeting a specific window.
    Window {
        /// Target window identifier.
        window_id: window::Id,
        /// Platform event dispatched to window.
        event: PlatformEvent,
    },
    /// Application-wide environmental event.
    App(PlatformEvent),
}

/// Comprehensive state tracker for a single window.
#[derive(Debug, Clone)]
pub struct WindowStateTracker {
    /// Window identifier.
    pub id: window::Id,
    /// Monotonic count of redraw requests collapsed into frames.
    pub redraw_generation: u64,
    /// Monotonic count of successfully presented frames.
    pub frame_generation: u64,
    /// Whether the window has OS input focus.
    pub is_focused: bool,
    /// Whether the window is minimized or fully occluded.
    pub is_occluded: bool,
    /// Whether the window has no drawable surface area.
    pub is_zero_sized: bool,
    /// Display scale factor.
    pub scale_factor: f32,
    /// Current dirty flags accumulated for the pending frame.
    pub dirty_flags: DirtyFlags,
    /// Timestamp of the last successful frame presentation.
    pub last_present_instant: Option<Instant>,
}

impl WindowStateTracker {
    /// Creates a new [`WindowStateTracker`].
    pub fn new(id: window::Id, scale_factor: f32) -> Self {
        Self {
            id,
            redraw_generation: 0,
            frame_generation: 0,
            is_focused: true,
            is_occluded: false,
            is_zero_sized: false,
            scale_factor: valid_scale_factor(scale_factor).unwrap_or(1.0),
            dirty_flags: DirtyFlags::UPDATE_DIRTY
                | DirtyFlags::LAYOUT_DIRTY
                | DirtyFlags::PAINT_DIRTY,
            last_present_instant: None,
        }
    }

    /// Returns `true` if the window requires a render pass.
    pub fn should_render(&self) -> bool {
        if self.is_suspended() {
            return false;
        }

        self.dirty_flags
            .intersects(DirtyFlags::PAINT_DIRTY | DirtyFlags::PRESENT_DIRTY | DirtyFlags::ANIMATING)
    }

    #[inline]
    fn is_suspended(&self) -> bool {
        self.is_occluded || self.is_zero_sized
    }
}

/// Central frame gate and event router.
///
/// Ensures multiple invalidations before the next frame presentation coalesce,
/// prevents redundant redraw requests, and guarantees zero framework wakeups
/// when all managed windows are clean.
pub struct FrameGate {
    windows: FxHashMap<window::Id, WindowStateTracker>,
    total_wakeups: u64,
    collapsed_redraws: u64,
}

impl FrameGate {
    /// Creates a new empty [`FrameGate`].
    pub fn new() -> Self {
        Self {
            windows: FxHashMap::default(),
            total_wakeups: 0,
            collapsed_redraws: 0,
        }
    }

    /// Registers a window with the frame gate.
    pub fn track_window(&mut self, id: window::Id, scale_factor: f32) {
        let _ = self
            .windows
            .insert(id, WindowStateTracker::new(id, scale_factor));
    }

    /// Unregisters a closed window.
    pub fn untrack_window(&mut self, id: window::Id) {
        let _ = self.windows.remove(&id);
    }

    /// Records an invalidation flag for the given window, coalescing with existing dirty flags.
    pub fn invalidate(&mut self, id: window::Id, flags: DirtyFlags) {
        if let Some(tracker) = self.windows.get_mut(&id) {
            mark_dirty(tracker, flags, &mut self.collapsed_redraws);
        }
    }

    /// Ingests a raw platform event, normalizes it, and updates dirty flags.
    pub fn handle_event(
        &mut self,
        window_id: window::Id,
        event: PlatformEvent,
    ) -> Option<TaimEvent> {
        self.total_wakeups = self.total_wakeups.wrapping_add(1);

        let tracker = self.windows.get_mut(&window_id)?;

        match &event {
            PlatformEvent::FocusChanged(focused) => {
                tracker.is_focused = *focused;
                mark_dirty(
                    tracker,
                    DirtyFlags::UPDATE_DIRTY | DirtyFlags::PAINT_DIRTY,
                    &mut self.collapsed_redraws,
                );
            }
            PlatformEvent::OcclusionChanged(occluded) => {
                tracker.is_occluded = *occluded;
                if *occluded {
                    tracker.dirty_flags &= !DirtyFlags::ANIMATING;
                } else {
                    mark_dirty(
                        tracker,
                        DirtyFlags::PAINT_DIRTY,
                        &mut self.collapsed_redraws,
                    );
                }
            }
            PlatformEvent::SurfaceSizeChanged { zero_sized } => {
                tracker.is_zero_sized = *zero_sized;
                if *zero_sized {
                    tracker.dirty_flags &= !DirtyFlags::ANIMATING;
                } else {
                    mark_dirty(
                        tracker,
                        DirtyFlags::LAYOUT_DIRTY | DirtyFlags::PAINT_DIRTY,
                        &mut self.collapsed_redraws,
                    );
                }
            }
            PlatformEvent::ScaleFactorChanged(scale) => {
                if let Some(scale) = valid_scale_factor(*scale) {
                    tracker.scale_factor = scale;
                    mark_dirty(
                        tracker,
                        DirtyFlags::LAYOUT_DIRTY | DirtyFlags::PAINT_DIRTY,
                        &mut self.collapsed_redraws,
                    );
                }
            }
            PlatformEvent::RedrawRequested => {
                mark_dirty(
                    tracker,
                    DirtyFlags::PAINT_DIRTY | DirtyFlags::PRESENT_DIRTY,
                    &mut self.collapsed_redraws,
                );
            }
            PlatformEvent::CursorMoved { .. } | PlatformEvent::KeyInput { .. } => {
                mark_dirty(
                    tracker,
                    DirtyFlags::UPDATE_DIRTY,
                    &mut self.collapsed_redraws,
                );
            }
            PlatformEvent::ThemeChanged => {
                mark_dirty(
                    tracker,
                    DirtyFlags::LAYOUT_DIRTY | DirtyFlags::PAINT_DIRTY,
                    &mut self.collapsed_redraws,
                );
            }
        }

        Some(TaimEvent::Window { window_id, event })
    }

    /// Returns an iterator over window IDs that currently require rendering.
    ///
    /// Multi-window work scales with dirty windows, NOT all windows.
    pub fn dirty_windows(&self) -> impl Iterator<Item = window::Id> + '_ {
        self.windows
            .iter()
            .filter(|(_, tracker)| tracker.should_render())
            .map(|(id, _)| *id)
    }

    /// Returns whether a specific window currently requires rendering.
    #[inline]
    pub fn should_render(&self, id: window::Id) -> bool {
        self.windows
            .get(&id)
            .is_some_and(WindowStateTracker::should_render)
    }

    /// Returns the deadline owned by this gate.
    ///
    /// TAIM's runtime scheduler is the sole owner of animation deadlines, so
    /// the platform frame gate deliberately has no independent timer to wake.
    /// Keeping this query lets diagnostics assert that the event-loop adapter
    /// remains quiescent without reintroducing duplicate deadline state.
    #[inline]
    pub const fn next_wake_deadline(&self) -> Option<Instant> {
        None
    }

    /// Returns `true` if any visible window has immediate dirty work.
    pub fn has_immediate_work(&self) -> bool {
        self.windows.values().any(WindowStateTracker::should_render)
    }

    /// Returns `true` if the engine has no immediate work requiring execution this frame.
    pub fn is_idle_now(&self) -> bool {
        !self.has_immediate_work()
    }

    /// Returns `true` if all visible windows are clean.
    pub fn is_idle(&self) -> bool {
        self.windows
            .values()
            .all(|tracker| !tracker.should_render())
    }

    /// Completes presentation for the given window, clearing dirty flags.
    pub fn on_frame_presented(&mut self, id: window::Id) {
        if let Some(tracker) = self.windows.get_mut(&id) {
            tracker.frame_generation = tracker.frame_generation.wrapping_add(1);
            tracker.last_present_instant = Some(Instant::now());

            // Preserve ANIMATING flag if animation loop is continuous
            let was_animating = tracker.dirty_flags.contains(DirtyFlags::ANIMATING);
            tracker.dirty_flags = if was_animating {
                DirtyFlags::ANIMATING
            } else {
                DirtyFlags::CLEAN
            };
        }
    }

    /// Returns total wakeup events received.
    pub fn total_wakeups(&self) -> u64 {
        self.total_wakeups
    }

    /// Returns the number of redundant redraw requests that were coalesced.
    pub fn collapsed_redraws(&self) -> u64 {
        self.collapsed_redraws
    }

    /// Returns a reference to the tracker for a specific window.
    pub fn tracker(&self, id: window::Id) -> Option<&WindowStateTracker> {
        self.windows.get(&id)
    }
}

#[inline]
fn valid_scale_factor(scale_factor: f32) -> Option<f32> {
    (scale_factor.is_finite() && scale_factor > 0.0).then_some(scale_factor)
}

#[inline]
fn mark_dirty(tracker: &mut WindowStateTracker, flags: DirtyFlags, collapsed_redraws: &mut u64) {
    let previous = tracker.dirty_flags;
    let adds_work = (flags.0 & !previous.0) != 0;

    tracker.dirty_flags |= flags;

    if previous.contains(DirtyFlags::PAINT_DIRTY) && flags.contains(DirtyFlags::PAINT_DIRTY) {
        *collapsed_redraws = collapsed_redraws.wrapping_add(1);
    }

    if adds_work {
        tracker.redraw_generation = tracker.redraw_generation.wrapping_add(1);
    }
}

impl Default for FrameGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redraw_request_coalescing() {
        let mut gate = FrameGate::new();
        let win = window::Id::unique();
        gate.track_window(win, 1.0);
        gate.on_frame_presented(win); // Start from a clean frame

        // First paint invalidation sets flag
        gate.invalidate(win, DirtyFlags::PAINT_DIRTY);
        let gen1 = gate.tracker(win).unwrap().redraw_generation;

        // Send 10 redundant paint invalidations before presenting
        for _ in 0..10 {
            gate.invalidate(win, DirtyFlags::PAINT_DIRTY);
        }

        // Must have coalesced without advancing redraw generation
        assert_eq!(gate.tracker(win).unwrap().redraw_generation, gen1);
        assert_eq!(gate.collapsed_redraws(), 10);
        assert!(gate.tracker(win).unwrap().should_render());

        // Present frame: clears dirty flags
        gate.on_frame_presented(win);
        assert_eq!(gate.tracker(win).unwrap().dirty_flags, DirtyFlags::CLEAN);
        assert!(gate.is_idle());
    }

    #[test]
    fn test_occluded_window_does_not_render() {
        let mut gate = FrameGate::new();
        let win = window::Id::unique();
        gate.track_window(win, 1.0);

        // Mark occluded
        let _ = gate.handle_event(win, PlatformEvent::OcclusionChanged(true));
        gate.invalidate(win, DirtyFlags::PAINT_DIRTY);

        // Occluded window must not render
        assert!(!gate.tracker(win).unwrap().should_render());
        assert_eq!(gate.dirty_windows().count(), 0);

        // Un-occlude: now it renders
        let _ = gate.handle_event(win, PlatformEvent::OcclusionChanged(false));
        assert!(gate.tracker(win).unwrap().should_render());
        assert_eq!(gate.dirty_windows().count(), 1);
    }

    #[test]
    fn test_multi_window_scaling() {
        let mut gate = FrameGate::new();
        let win1 = window::Id::unique();
        let win2 = window::Id::unique();
        let win3 = window::Id::unique();

        gate.track_window(win1, 1.0);
        gate.track_window(win2, 1.0);
        gate.track_window(win3, 1.0);

        // Clear all initial flags
        gate.on_frame_presented(win1);
        gate.on_frame_presented(win2);
        gate.on_frame_presented(win3);
        assert!(gate.is_idle());

        // Invalidate ONLY win2
        gate.invalidate(win2, DirtyFlags::PAINT_DIRTY);

        // Dirty windows list contains ONLY win2, NOT win1 or win3!
        let dirty: Vec<_> = gate.dirty_windows().collect();
        assert_eq!(dirty, vec![win2]);
    }

    #[test]
    fn test_event_pump_benchmark() {
        let mut gate = FrameGate::new();
        let win = window::Id::unique();
        gate.track_window(win, 1.0);
        gate.on_frame_presented(win);

        let start = Instant::now();
        // Simulate burst of 10,000 mouse moves
        for i in 0..10_000 {
            let _ = gate.handle_event(
                win,
                PlatformEvent::CursorMoved {
                    x: (i % 500) as f32,
                    y: (i % 500) as f32,
                },
            );
        }
        let burst_duration = start.elapsed();

        assert_eq!(gate.total_wakeups(), 10_000);
        println!(
            "[EventPump Benchmark] 10,000 events processed in {:?} (avg={:?} per event)",
            burst_duration,
            burst_duration / 10_000
        );
    }

    #[test]
    fn test_frame_gate_controls_real_redraw_path() {
        let mut gate = FrameGate::new();
        let win = window::Id::unique();
        gate.track_window(win, 1.0);
        gate.on_frame_presented(win);

        assert!(!gate.tracker(win).unwrap().should_render());

        gate.invalidate(win, DirtyFlags::PAINT_DIRTY);
        assert!(gate.tracker(win).unwrap().should_render());

        gate.on_frame_presented(win);
        assert!(!gate.tracker(win).unwrap().should_render());
    }

    #[test]
    fn test_duplicate_redraws_result_in_one_render() {
        let mut gate = FrameGate::new();
        let win = window::Id::unique();
        gate.track_window(win, 1.0);
        gate.on_frame_presented(win);

        for _ in 0..10 {
            gate.invalidate(win, DirtyFlags::PAINT_DIRTY);
        }

        assert_eq!(gate.collapsed_redraws(), 9);
        assert!(gate.tracker(win).unwrap().should_render());

        gate.on_frame_presented(win);
        assert_eq!(gate.tracker(win).unwrap().dirty_flags, DirtyFlags::CLEAN);
        assert!(!gate.tracker(win).unwrap().should_render());
    }

    #[test]
    fn test_clean_window_performs_no_render_work() {
        let mut gate = FrameGate::new();
        let win = window::Id::unique();
        gate.track_window(win, 1.0);
        gate.on_frame_presented(win);

        assert_eq!(gate.tracker(win).unwrap().dirty_flags, DirtyFlags::CLEAN);
        assert!(!gate.tracker(win).unwrap().should_render());
        assert_eq!(gate.dirty_windows().count(), 0);
    }

    #[test]
    fn test_occluded_window_skips_render() {
        let mut gate = FrameGate::new();
        let win = window::Id::unique();
        gate.track_window(win, 1.0);
        gate.on_frame_presented(win);

        let _ = gate.handle_event(win, PlatformEvent::OcclusionChanged(true));
        gate.invalidate(win, DirtyFlags::PAINT_DIRTY);

        assert!(!gate.tracker(win).unwrap().should_render());
        assert_eq!(gate.dirty_windows().count(), 0);

        let _ = gate.handle_event(win, PlatformEvent::OcclusionChanged(false));
        assert!(gate.tracker(win).unwrap().should_render());
        assert_eq!(gate.dirty_windows().count(), 1);
    }

    #[test]
    fn test_idle_runtime_has_zero_internal_frame_generation() {
        let mut gate = FrameGate::new();
        let win = window::Id::unique();
        gate.track_window(win, 1.0);
        gate.on_frame_presented(win);

        let initial_generation = gate.tracker(win).unwrap().frame_generation;
        assert!(gate.is_idle());
        assert!(gate.is_idle_now());
        assert!(!gate.has_immediate_work());
        assert_eq!(
            gate.tracker(win).unwrap().frame_generation,
            initial_generation
        );
    }
}
