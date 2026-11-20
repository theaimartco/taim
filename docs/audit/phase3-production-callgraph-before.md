# Phase 3 Production Call Graph Audit: Current Reality (Before Phase 3)

**Baseline Commit**: `3eaaa0c0cf952e2b9c91466a8d641ed8f1ed9282`  
**Date**: September 2026  
**Auditor**: TAIM Architectural Independence Verification

---

## 1. Executive Summary of Reality

An exhaustive code audit of `taim_winit`, `taim_program`, and `taim_futures` reveals that despite the Phase 2 creation of `App`, `AppRunner`, `TaskScheduler`, and `FrameGate`, **the active production execution path in TAIM remains inherited from Iced**.

Specifically:
1. `run_app(app)` is a thin facade: it immediately converts `App` into `AppToProgram<A>`, which implements `Program`, and calls `run(program)`.
2. `run(program)` constructs `program::Instance<P>`, which in turn creates `AppRunner<ProgramAdapter<P>>`. The application ends up wrapped in a double adapter layer:
   $$\text{App} \longrightarrow \text{AppToProgram} \longrightarrow \text{Program} \longrightarrow \text{ProgramAdapter} \longrightarrow \text{AppRunner}$$
3. **`FrameGate` is not in the production execution path**. It is only re-exported in `winit/src/lib.rs` (line 39) and tested in isolation in `winit/src/frame_gate.rs`. The production event loop in `run_instance` does not instantiate or query `FrameGate`.
4. **`TaskScheduler` is not in the production execution path**. It is only exported in `futures/src/lib.rs` (line 24) and tested in `futures/src/scheduler.rs`. All GUI tasks from `App::update` or `Program::update` are converted into streams and dispatched directly to the background `futures::Executor` via `runtime.run(stream)`.
5. **Redraws are uncoalesced**. Redraws in `run_instance` rely on OS `WindowEvent::RedrawRequested` without dirty-flag gating.

---

## 2. Detailed Production Call Graph (Today)

### 2.1 Entry Point: `run_app`
```text
taim_winit::run_app<A: App>(app: A)   [winit/src/lib.rs:72-78]
    │
    └──> app.into_program()           [program/src/app.rs:259]
            │ Returns AppToProgram<A> (implements Program)
            ▼
         taim_winit::run<AppToProgram<A>>(program) [winit/src/lib.rs:81]
```

### 2.2 Orchestration: `run`
```text
taim_winit::run<P: Program>(program: P)  [winit/src/lib.rs:81-250]
    │
    ├── 1. Read settings: program.settings(), program.window()
    ├── 2. Build Winit EventLoop: EventLoop::with_user_event().build()
    ├── 3. Create Proxy & Worker: Proxy::new(event_loop.create_proxy())
    ├── 4. Initialize legacy Runtime: Runtime::new(executor, proxy) [futures/src/runtime.rs]
    ├── 5. Construct Instance: program::Instance::new(program) [program/src/compat.rs:237]
    │       │
    │       └──> AppRunner::new(ProgramAdapter::new(program))
    │               (Nested adapter wrapping ProgramAdapter wrapping AppToProgram!)
    ├── 6. Initial task: runtime::task::into_stream(task) -> runtime.run(stream)
    ├── 7. Initial subscriptions: runtime.track(...)
    ├── 8. Spawn run_instance future: Box::pin(run_instance::<P>(...)) [winit/src/lib.rs:483]
    └── 9. EventLoop::run_app(runner): Runner implements ApplicationHandler
```

### 2.3 Production Event Loop & Redraw: `run_instance`
```text
Runner::window_event / user_event / new_events
    │ (Sends event over unbounded mpsc channel: event_sender)
    ▼
run_instance::<P> async loop          [winit/src/lib.rs:548-1350]
    │
    ├── On WindowEvent::CursorMoved / MouseInput / etc.:
    │     ├── Maps event to taim_core::Event
    │     ├── user_interfaces[window_id].update(...)
    │     ├── Collects messages
    │     └── Dispatches messages:
    │           └── runtime.enter(|| program.update(message))
    │                 └── Returns Task<Message>
    │                       └── runtime.run(stream) -> executor.spawn(...)
    │
    ├── On Subscriptions:
    │     └── runtime.track(subscription::into_recipes(program.subscription()))
    │
    └── On RedrawRequested / Redraw:
          ├── Unconditionally checks window states
          ├── UserInterface::draw(...)
          └── compositor.present(...)
```

---

## 3. Subsystem Status Matrix (Today)

| Subsystem | File Location | Used in Production Path? | Current Behavior |
| :--- | :--- | :---: | :--- |
| **`App` / `AppRunner`** | `program/src/app.rs` | **Partial (Indirect)** | Wrapped inside `AppToProgram` then `ProgramAdapter` then `Instance`. Not driven natively by `winit`. |
| **`Program`** | `program/src/compat.rs` | **YES (Primary)** | `run()` requires `P: Program`. `winit::run_instance` is generic over `P: Program`. |
| **`FrameGate`** | `winit/src/frame_gate.rs` | **NO** | Re-exported in `winit/src/lib.rs` line 39. Zero calls in `run` or `run_instance`. |
| **`DirtyFlags`** | `winit/src/frame_gate.rs` | **NO** | Never checked in pipeline; redraws and layouts trigger based on raw event presence. |
| **`TaskScheduler`** | `futures/src/scheduler.rs` | **NO** | Re-exported in `futures/src/lib.rs` line 24. Zero calls in `Runtime` or `run_instance`. |
| **`CancelToken` / `TaskId`**| `futures/src/scheduler.rs`| **NO** | Not connected to `Task<Message>` or UI lifecycle. |
| **`SubscriptionTracker`** | `futures/src/subscription/tracker.rs` | **YES** | Used by `Runtime::track`, but receives recipes converted from `program.subscription()`. |
| **Generational Atlas Allocator** | `wgpu/src/image/atlas/allocator.rs` | **YES** | Directly used by `taim_wgpu` image cache pipeline. |

---

## 4. Architectural Inversion Required in Phase 3

To achieve true technical independence and performance:

1. **Native Execution Inversion**:
   `run_app<A: App>` must construct and drive `AppRunner<A>` directly in `winit`, completely eliminating `AppToProgram` from the native path.
2. **Compatibility Path Inversion**:
   `run<P: Program>` must adapt `Program` into `App` via `ProgramAdapter<P>`, and forward to `run_app(ProgramAdapter(program))`.
   Legacy `Program` becomes a client of native `App`, NOT the other way around.
3. **TaskScheduler Wiring**:
   `Task<Message>` produced by `App::boot`, `App::update`, and subscriptions must be registered with `TaskScheduler`, producing `TaskId`s and utilizing duplicate wake collapsing before delivering messages to the event loop.
4. **FrameGate Production Integration**:
   `FrameGate` must be owned by the Winit runner, ingesting normalized platform events into `DirtyFlags`, coalescing redraw requests, and filtering render passes to dirty windows only.
5. **Generational Safety Fix**:
   `CancelToken` and `TaskSlotHeader` wakers must enforce generation checks against `expected_generation` to prevent stale operations on recycled slots.
