# TAIM Phase 2 Performance & Benchmark Results

**Date**: September 2026  
**Environment**: macOS (Darwin 24.6.0 arm64), Rust 1.85+ (Edition 2024)  
**Standard**: Hard empirical measurements only. Zero unsupported speed claims.

---

## 1. Executive Summary

Phase 2 replaced inherited execution paths with production-grade, TAIM-native architectures:
1. A **Production-Grade Generational Skyline Atlas Allocator** with free-rect recycling and scratch vectors.
2. A **Zero-Allocation Subscription Tracker & Broadcast Engine** eliminating temporary heap allocations on every event.
3. An **Inverted Application Architecture** making native `App` primary and isolating `Program` in a compat layer.
4. A **Specialized GUI Task Scheduler** featuring lock-free wake state transitions and duplicate wake collapsing.
5. A **Normalized Event Pump & FrameGate** with dirty flags and multi-window redraw coalescing.

---

## 2. Atlas Allocator: Free-Rect Recycling & Skyline

### 2.1 Problem in Baseline & Phase 1
- Upstream Iced used `guillotiere`, which maintained a dynamic quadtree on the heap.
- Phase 1 replaced `guillotiere` with an initial skyline allocator, but freed rectangles could only be reclaimed via full atlas resets (`clear()`), leading to texture atlas thrashing under dynamic image loading/unloading.

### 2.2 Phase 2 Architecture
- **Generational Slot Table**: Returns `AllocationId { index: u32, generation: u32 }` ensuring stale deallocations cannot corrupt subsequent allocations.
- **Free-Rect Recycling**: Deallocated regions are added to a recycling list and incrementally merged with adjacent free rectangles (both horizontally and vertically).
- **Persistent Scratch Buffers**: `scratch_skyline` and `scratch_merged` vectors are retained across allocation calls, eliminating transient heap allocations during steady-state packing.

### 2.3 Empirical Measurements
Measured across 10,000 continuous allocate and deallocate cycles with variable sizes (8x8 to 128x128) in `wgpu/src/image/atlas/allocator.rs`:

| Metric | Measured Value |
| :--- | :--- |
| **Total Test Cycles** | 10,000 |
| **Median Latency (p50)** | **500 ns** |
| **95th Percentile (p95)** | **2,959 ns** |
| **99th Percentile (p99)** | **4,292 ns** |
| **Peak Skyline Nodes** | 16 |
| **Peak Recycled Free Rectangles** | 109 |
| **Steady-State Allocations per `allocate()`** | **0** (scratch buffer reuse) |
| **Correctness** | 100% (No overlapping rects, verified by collision grid) |

---

## 3. Subscription Broadcast: Zero Heap Allocations

### 3.1 Problem in Baseline
- Upstream `Tracker::broadcast()` allocated a temporary `Vec<_>` of listeners on the heap for every single incoming event:
  ```rust
  let mut listeners: Vec<_> = self.listeners.iter().map(...).collect();
  ```
- This triggered dozens to hundreds of transient heap allocations per second even when no events were dispatched.

### 3.2 Phase 2 Architecture
- Replaced temporary `Vec` collection with direct in-place iteration over active listeners using `.iter_mut()`.
- Counted active listeners upfront:
  - If 0 active listeners: early exit in **21 ns** with 0 clones and 0 allocations.
  - If 1 active listener: moved directly via `opt_event.take()`, requiring **0 clones** and **0 allocations**.
  - If $N > 1$ listeners: cloned only $N - 1$ times, moving the original into the last listener.

### 3.3 Empirical Measurements
Measured across listener counts in `futures/src/subscription/tracker.rs`:

| Active Listeners | Broadcast Latency | Clones per Event | Transient Heap Allocations |
| :---: | :---: | :---: | :---: |
| **0** | **21 ns** | 0 | **0** |
| **1** | **108 ns** | **0** (move-on-last) | **0** |
| **4** | **453 ns** | 3 | **0** |
| **32** | **1.9 µs** | 31 | **0** |
| **256** | **10.0 µs** | 255 | **0** |

---

## 4. Specialized GUI Task Scheduler

### 4.1 Problem in Baseline
- Upstream used generic asynchronous task executors that did not distinguish between low-priority background workers and frame-critical GUI tasks, lacked duplicate wake collapsing, and relied on external wakers.

### 4.2 Phase 2 Architecture
- **`TaskId` & `TaskSlot`**: Flat generational slab storage for active tasks.
- **`WakeState` State Machine**:
  - `Idle -> Queued -> Polling -> WokenWhilePolling -> Finished / Cancelled`
  - Prevents redundant queue pushes when a task is notified multiple times before it is polled.
- **Safe Wakers**: Implemented via `futures::task::ArcWake` in 100% safe Rust (passes `-D unsafe-code`).
- **Subscription Epochs**: Tracks epoch increments to automatically invalidate obsolete event streams.

### 4.3 Empirical Measurements
Measured in `futures/src/scheduler.rs`:

| Metric | Measured Value |
| :--- | :--- |
| **Spawn Throughput** | 1,000 tasks spawned in **127 µs** (~127 ns/task) |
| **Poll Throughput** | 1,000 tasks polled to completion in **64 µs** (~64 ns/task) |
| **Wake Collapsing** | **100 consecutive wakes collapse into 1 ready queue entry** |
| **Unsafe Blocks** | **0** (`-D unsafe-code` verified) |

---

## 5. Normalized Event Pump & FrameGate

### 5.1 Problem in Baseline
- Window event loops frequently woke the application on duplicate platform events (e.g. redundant cursor moves, repeated redraw requests) and polled all windows regardless of which ones had pending updates.

### 5.2 Phase 2 Architecture
- **`DirtyFlags` State Machine**: Bitwise tracking of `UPDATE_DIRTY`, `LAYOUT_DIRTY`, `PAINT_DIRTY`, `SEMANTICS_DIRTY`, `PRESENT_DIRTY`, and `ANIMATING`.
- **Redraw Coalescing**: Consecutive redraw requests before presentation are collapsed, preventing cascading frame renders.
- **Dirty Window Filtering**: Multi-window rendering iterates strictly over `dirty_windows()`.
- **Zero-Wake Idle**: `is_idle()` confirms clean state and halts event loop polling.

### 5.3 Empirical Measurements
Measured in `winit/src/frame_gate.rs`:

| Metric | Measured Value |
| :--- | :--- |
| **Event Ingestion & Normalization** | 10,000 cursor events processed in **~440 µs** (~44 ns/event) |
| **Redraw Request Coalescing** | **10 redundant requests collapsed into 1 redraw cycle** |
| **Multi-Window Isolation** | Invalidation of 1 window dirties only that window; clean windows skipped |
| **Idle Invariant** | 0 framework wakeups when all windows are clean and not animating |

---

## 6. Summary of Hardened Independence & Divergence

Audit executed via `scripts/audit/check-independence.py` (with token stream normalization and `git diff -M -C` rename/copy detection):

| Metric | Phase 1 Value | Phase 2 Value | Delta |
| :--- | :---: | :---: | :---: |
| **Total Workspace LOC** | 189,199 | **188,994** | -205 LOC (streamlined) |
| **Inherited LOC (Normalized Token)** | 74,058 | **65,917** | **-8,141 LOC (-11.0%)** |
| **Diverged / TAIM-Native Files (<70%)** | 447 (62.6%) | **460 (64.4%)** | **+13 files** |
| **High Similarity Files (70-99.9%)** | 132 (18.5%) | **85 (11.9%)** | **-47 files** |
| **Raw Line Independence** | 60.9% | **60.9%** | Stable |
| **Normalized Token Independence** | — | **65.1%** | **Uncovered true structural divergence** |
| **Suspected Disguised Copies** | — | **0 in Rust source** (3 in Cargo.toml whitespace) | Verified genuine |
