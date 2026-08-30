//! Buffer lifecycle for a VYOM surface.
//!
//! A Wayland client does not own its pixels once it commits them. The
//! compositor may be scanning out or sampling a committed buffer for an
//! unbounded time, and it says so by sending `wl_buffer.release` — which may
//! arrive one frame later, several frames later, or (for a buffer still on a
//! display plane) not until something else replaces it.
//!
//! Rendering into a buffer the compositor still holds is the single most
//! common defect in a hand-written Wayland backend. It does not fail: it
//! tears, or shows one frame's content inside another's, intermittently, on
//! some compositors and not others. This module exists so that mistake is
//! impossible to express rather than merely discouraged.
//!
//! # What is verified here, and what is not
//!
//! Everything in this file is a state machine over identifiers, and every
//! transition is covered by the tests below. **None of it has been run
//! against a real compositor**, because that needs a Wayland session and this
//! is where the protocol calls will be wired. The state machine being correct
//! is necessary, not sufficient.

use std::num::NonZeroU64;

/// A buffer's identity within one pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BufferId(NonZeroU64);

impl BufferId {
    /// Creates an identity. `0` is not a valid buffer.
    #[must_use]
    pub const fn new(raw: u64) -> Option<Self> {
        match NonZeroU64::new(raw) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the raw identity.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Where a buffer is in the attach/commit/release cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferState {
    /// Owned by the client. The only state in which writing pixels is safe.
    Free,
    /// Attached to a surface, not yet committed. Still client-owned: an
    /// attach without a commit has no effect and can be replaced.
    Attached,
    /// Committed. The compositor may be reading it, and it stays here until
    /// `wl_buffer.release` arrives.
    InFlight,
}

/// A protocol misuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The buffer does not belong to this pool.
    #[error("buffer {0:?} is not in this pool")]
    Unknown(BufferId),
    /// The surface has not been configured, so nothing may be attached.
    #[error("no buffer may be attached before the first configure is acknowledged")]
    NotConfigured,
    /// The transition is not legal from the buffer's current state.
    #[error("buffer {id:?} is {from:?} and cannot become {to:?}")]
    Illegal {
        /// The buffer.
        id: BufferId,
        /// Its current state.
        from: BufferState,
        /// The state that was requested.
        to: BufferState,
    },
}

/// One buffer and its state.
#[derive(Debug, Clone, Copy)]
struct Buffer {
    id: BufferId,
    state: BufferState,
    /// The generation this buffer was created for. A resize bumps the pool's
    /// generation and strands every older buffer: it is the wrong size, so it
    /// must never be handed out again even after the compositor releases it.
    generation: u64,
}

/// The buffers backing one surface.
///
/// Sized rather than unbounded. A pool that grows whenever every buffer is in
/// flight will keep growing under a compositor that holds buffers, turning a
/// pacing problem into a memory leak; refusing to allocate instead makes the
/// frame skip, which is visible and correct.
#[derive(Debug)]
pub struct BufferPool {
    width: u32,
    height: u32,
    generation: u64,
    depth: usize,
    next_id: u64,
    buffers: Vec<Buffer>,
}

impl BufferPool {
    /// The default number of buffers.
    ///
    /// Three: one being scanned out, one queued, one being drawn. Two is
    /// enough only if the compositor releases promptly, and it usually does
    /// not; more than three adds latency without adding throughput, because
    /// the extra buffer is always the oldest queued frame.
    pub const DEFAULT_DEPTH: usize = 3;

    /// Creates a pool for a surface of `width` by `height`.
    #[must_use]
    pub fn new(width: u32, height: u32, depth: usize) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            generation: 0,
            depth: depth.max(1),
            next_id: 1,
            buffers: Vec::new(),
        }
    }

    /// The surface size these buffers are sized for.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// How many buffers the compositor is currently holding.
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.buffers
            .iter()
            .filter(|buffer| buffer.state == BufferState::InFlight)
            .count()
    }

    /// How many buffers exist, in any state.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    /// Whether the pool has allocated nothing yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }

    /// Returns a buffer's state, if it belongs to this pool.
    #[must_use]
    pub fn state(&self, id: BufferId) -> Option<BufferState> {
        self.buffers
            .iter()
            .find(|buffer| buffer.id == id)
            .map(|buffer| buffer.state)
    }

    /// Returns a buffer that is safe to render into.
    ///
    /// Reuses a free buffer of the current generation, or allocates one if
    /// the pool has room. Returns `None` when every buffer is in flight —
    /// the frame must be skipped rather than drawn into a buffer the
    /// compositor is reading.
    pub fn acquire(&mut self) -> Option<BufferId> {
        let generation = self.generation;

        if let Some(buffer) = self
            .buffers
            .iter_mut()
            .find(|buffer| buffer.state == BufferState::Free && buffer.generation == generation)
        {
            return Some(buffer.id);
        }

        // Retire anything free but stale before growing: a resized surface
        // would otherwise allocate a full new set alongside the old one.
        self.buffers
            .retain(|buffer| buffer.state != BufferState::Free || buffer.generation == generation);

        if self.buffers.len() >= self.depth {
            return None;
        }

        let id = BufferId::new(self.next_id)?;
        self.next_id += 1;

        self.buffers.push(Buffer {
            id,
            state: BufferState::Free,
            generation,
        });

        Some(id)
    }

    /// Attaches a buffer to the surface.
    ///
    /// `configured` is the surface's readiness, from
    /// [`crate::surface::SurfaceLifecycle::may_attach_buffer`]. Attaching
    /// before the first configure is acknowledged is a protocol error that
    /// ends the connection, not a request the compositor ignores.
    pub fn attach(&mut self, id: BufferId, configured: bool) -> Result<(), Error> {
        if !configured {
            return Err(Error::NotConfigured);
        }

        let buffer = self.find_mut(id)?;

        match buffer.state {
            // Re-attaching an already attached buffer is legal and idempotent:
            // an attach with no commit has not happened yet.
            BufferState::Free | BufferState::Attached => {
                buffer.state = BufferState::Attached;
                Ok(())
            }
            BufferState::InFlight => Err(Error::Illegal {
                id,
                from: BufferState::InFlight,
                to: BufferState::Attached,
            }),
        }
    }

    /// Commits an attached buffer. The compositor now owns it.
    pub fn commit(&mut self, id: BufferId) -> Result<(), Error> {
        let buffer = self.find_mut(id)?;

        match buffer.state {
            BufferState::Attached => {
                buffer.state = BufferState::InFlight;
                Ok(())
            }
            from => Err(Error::Illegal {
                id,
                from,
                to: BufferState::InFlight,
            }),
        }
    }

    /// Records a `wl_buffer.release`.
    ///
    /// A buffer of a stale generation is dropped rather than returned to the
    /// free list: it is the wrong size for the surface it would be drawn into.
    pub fn release(&mut self, id: BufferId) -> Result<(), Error> {
        let generation = self.generation;
        let buffer = self.find_mut(id)?;

        match buffer.state {
            BufferState::InFlight => {
                if buffer.generation == generation {
                    buffer.state = BufferState::Free;
                } else {
                    self.buffers.retain(|buffer| buffer.id != id);
                }

                Ok(())
            }
            from => Err(Error::Illegal {
                id,
                from,
                to: BufferState::Free,
            }),
        }
    }

    /// Resizes the surface.
    ///
    /// Buffers already in flight are left alone — the compositor is still
    /// reading them and they must not be freed underneath it — but they are
    /// marked stale, so they are dropped on release rather than reused at the
    /// wrong size.
    pub fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));

        if (width, height) == (self.width, self.height) {
            return;
        }

        self.width = width;
        self.height = height;
        self.generation += 1;

        self.buffers
            .retain(|buffer| buffer.state != BufferState::Free);
    }

    fn find_mut(&mut self, id: BufferId) -> Result<&mut Buffer, Error> {
        self.buffers
            .iter_mut()
            .find(|buffer| buffer.id == id)
            .ok_or(Error::Unknown(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool() -> BufferPool {
        BufferPool::new(64, 64, BufferPool::DEFAULT_DEPTH)
    }

    /// Drives one buffer all the way round.
    fn cycle(pool: &mut BufferPool) -> BufferId {
        let id = pool.acquire().expect("a fresh pool has a buffer");
        pool.attach(id, true).expect("attach after configure");
        pool.commit(id).expect("commit an attached buffer");
        id
    }

    #[test]
    fn nothing_may_be_attached_before_the_first_configure() {
        // A protocol error that ends the connection, not a no-op. The whole
        // reason `SurfaceLifecycle::may_attach_buffer` exists.
        let mut pool = pool();
        let id = pool.acquire().unwrap();

        assert_eq!(pool.attach(id, false), Err(Error::NotConfigured));
        assert_eq!(pool.state(id), Some(BufferState::Free));
    }

    #[test]
    fn a_buffer_the_compositor_holds_is_never_handed_back_out() {
        // The defect this module exists to prevent: rendering into a buffer
        // that is still being scanned out. It does not fail, it tears.
        let mut pool = pool();
        let first = cycle(&mut pool);

        for _ in 0..8 {
            if let Some(next) = pool.acquire() {
                assert_ne!(next, first, "handed out a buffer that is in flight");
            }
        }

        assert_eq!(pool.state(first), Some(BufferState::InFlight));
    }

    #[test]
    fn a_full_pool_refuses_rather_than_growing() {
        // An unbounded pool turns a compositor that holds buffers into a
        // memory leak. Refusing makes the frame skip, which is visible.
        let mut pool = pool();

        for _ in 0..BufferPool::DEFAULT_DEPTH {
            let _ = cycle(&mut pool);
        }

        assert_eq!(pool.in_flight(), BufferPool::DEFAULT_DEPTH);
        assert_eq!(pool.acquire(), None);
        assert_eq!(pool.len(), BufferPool::DEFAULT_DEPTH);
    }

    #[test]
    fn a_released_buffer_is_reused_rather_than_reallocated() {
        let mut pool = pool();
        let id = cycle(&mut pool);

        pool.release(id).expect("the compositor released it");

        assert_eq!(pool.state(id), Some(BufferState::Free));
        assert_eq!(pool.acquire(), Some(id));
        assert_eq!(pool.len(), 1, "a steady state must not allocate");
    }

    #[test]
    fn a_steady_state_allocates_nothing() {
        // The property that matters for a frame loop: after warm-up, cycling
        // buffers forever must not grow the pool.
        let mut pool = pool();

        for _ in 0..200 {
            let Some(id) = pool.acquire() else {
                // Every buffer in flight; release the oldest and continue.
                let held: Vec<_> = (1..=pool.next_id)
                    .filter_map(BufferId::new)
                    .filter(|id| pool.state(*id) == Some(BufferState::InFlight))
                    .collect();

                pool.release(held[0]).unwrap();
                continue;
            };

            pool.attach(id, true).unwrap();
            pool.commit(id).unwrap();
        }

        assert!(
            pool.len() <= BufferPool::DEFAULT_DEPTH,
            "pool grew to {} buffers",
            pool.len()
        );
    }

    #[test]
    fn committing_without_attaching_is_refused() {
        let mut pool = pool();
        let id = pool.acquire().unwrap();

        assert!(matches!(pool.commit(id), Err(Error::Illegal { .. })));
    }

    #[test]
    fn releasing_a_buffer_that_was_never_committed_is_refused() {
        // A release for a buffer the compositor never had means the two sides
        // disagree about who owns what, which is worth failing loudly on.
        let mut pool = pool();
        let id = pool.acquire().unwrap();

        assert!(matches!(pool.release(id), Err(Error::Illegal { .. })));
    }

    #[test]
    fn an_unknown_buffer_is_refused_in_every_transition() {
        let mut pool = pool();
        let stranger = BufferId::new(9_999).unwrap();

        assert_eq!(pool.attach(stranger, true), Err(Error::Unknown(stranger)));
        assert_eq!(pool.commit(stranger), Err(Error::Unknown(stranger)));
        assert_eq!(pool.release(stranger), Err(Error::Unknown(stranger)));
    }

    #[test]
    fn resizing_never_frees_a_buffer_the_compositor_is_reading() {
        // Freeing under the compositor is a use-after-free on its side.
        let mut pool = pool();
        let held = cycle(&mut pool);

        pool.resize(128, 128);

        assert_eq!(
            pool.state(held),
            Some(BufferState::InFlight),
            "a resize dropped a buffer the compositor still holds"
        );
        assert_eq!(pool.size(), (128, 128));
    }

    #[test]
    fn a_stale_buffer_is_retired_on_release_rather_than_reused() {
        // Reusing it would render a frame at the old size into a surface of
        // the new one.
        let mut pool = pool();
        let stale = cycle(&mut pool);

        pool.resize(128, 128);
        pool.release(stale).expect("release still succeeds");

        assert_eq!(pool.state(stale), None, "a stale buffer was kept");

        let fresh = pool.acquire().expect("a new buffer at the new size");
        assert_ne!(fresh, stale);
    }

    #[test]
    fn resizing_to_the_same_size_changes_nothing() {
        let mut pool = pool();
        let id = cycle(&mut pool);
        pool.release(id).unwrap();

        pool.resize(64, 64);

        assert_eq!(
            pool.acquire(),
            Some(id),
            "an identical resize churned buffers"
        );
    }

    #[test]
    fn attaching_twice_before_a_commit_is_idempotent() {
        // An attach that was never committed has not happened, so replacing
        // it is legal.
        let mut pool = pool();
        let id = pool.acquire().unwrap();

        pool.attach(id, true).unwrap();
        pool.attach(id, true)
            .expect("re-attach before commit is legal");

        assert_eq!(pool.state(id), Some(BufferState::Attached));
    }
}
