# Signal batching

`taim::signal::batch` groups notification delivery on the calling thread. Signal
values and revisions change immediately. Memo dependencies invalidate immediately
as well, so a memo read after a write within the batch recomputes current data.
Unrelated memos remain cached; invalidation does not evaluate derived values.

```rust
use taim::{Memo, Signal, signal::batch};

let count = Signal::new(1);
let input = count.clone();
let doubled = Memo::new(move || input.get() * 2);
assert_eq!(doubled.get(), 2);
batch(|| {
    assert!(count.set(2));
    assert_eq!(doubled.get(), 4);
    assert!(count.set(3));
});
assert_eq!(doubled.get(), 6);
```

Nested batches defer public callbacks until the outermost scope exits. Repeated
writes replace the queued revision for each subscription. Queue entries hold weak
subscriber registries, so dropping a subscription before delivery cancels its entry
and dropping a source does not leave its callbacks retained by the queue.

Ordinary writes also complete memo invalidation before public callbacks. This
matters when one source feeds two memos and a third memo combines them: a callback
must not observe one stale branch while the other is current. Dependency listeners
run during propagation, while public listeners run after it.

Callbacks run without value or subscriber-registry locks. A callback may read
sources or write them again; reentrant writes enqueue further delivery rather than
recursively invoking callbacks. They can update a not-yet-delivered subscriber's
queued revision. Callback order across different sources is unspecified. Callback
cycles that continually change values will not converge; batching does not detect
or permit cyclic reactive computations.

A panic in the batch body does not roll back committed writes. RAII restores the
batch depth, drains committed notifications, and preserves the body's panic.
Listener panics do not prevent other queued listeners from receiving changes;
when the body is not already unwinding, the first listener panic is resumed after
delivery. This behavior applies to unwinding builds, not `panic = "abort"`.

Batches are thread-local, not cross-thread transactions. Another thread's writes
and callbacks may run during a batch. A callback already selected concurrently may
still finish after its subscription is dropped. Memo subscribers report
invalidation, which need not imply a different derived value.

Validation:

- `cargo test -p taim-signal --offline` covers nested reads, coalescing, diamond
  consistency, cancellation, reentrancy, panic recovery, and revision wraparound.
- `cargo test --test runtime --offline` includes a live retained interface where
  three writes produce one tagged wake and one update with the final value.
- `cargo clippy -p taim-signal --all-targets --offline -- -D warnings` checks the
  standalone library and its tests.

These tests establish behavior, not the roadmap's latency or million-row budget.
