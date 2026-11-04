# Durable Android background work

This milestone implements deferrable JobScheduler work with an app-private SQLite
ledger and a separate Rust worker entry point. Android can load and execute the
worker after the app process dies, without starting a NativeActivity, renderer,
view tree or UI event loop. This is a native JobScheduler implementation, not a
WorkManager adapter or a push transport.

## Generate and implement a worker

```bash
python3 scripts/android/project.py /path/to/my-app \
  --package com.example.myapp --background-work
```

The generator registers TaimWorkService with the system-only BIND_JOB_SERVICE
permission, declares RECEIVE_BOOT_COMPLETED and ACCESS_NETWORK_STATE, and adds
the worker export. It does not grant INTERNET, background location or foreground
service permissions. Add INTERNET explicitly if your handler performs network I/O.
The generated dispatcher deliberately returns `unconfiguredWorker` until you
replace it with application logic.

```rust,ignore
taim::android::background_worker!(handle_work);

fn handle_work(cx: &mut taim::android::WorkContext<'_>) -> taim::android::WorkResult {
    use serde_json::json;
    use taim::android::WorkResult;
    if cx.is_stopped() { return WorkResult::Retry; }
    match cx.input.worker.as_str() {
        "sync" => {
            let _ = cx.set_progress(json!({"phase":"syncing"}));
            // Perform bounded, cancellation-aware, idempotent application I/O.
            if cx.is_stopped() { return WorkResult::Retry; }
            WorkResult::Success(json!({"complete":true}))
        }
        _ => WorkResult::Failure(json!({"code":"unknownWorker"})),
    }
}
```

Export exactly one dispatcher per cdylib. For shared host/Android sources, gate
the invocation and handler with `#[cfg(target_os = "android")]`. Handlers are
synchronous and execute on bounded native worker threads, not the Android main
thread. They can use ordinary Rust libraries, filesystem paths and application
I/O. Activity-bound `taim::android::services()` is unavailable in a purely
headless process; the worker contract does not fake an Activity or UI context.

In a hand-maintained application include all Java host classes, register the
same service in the application's default process, and provide its
`com.theaimartco.taim.work.lib_name` metadata. The service must retain its
BIND_JOB_SERVICE permission. Separate worker-process overrides are rejected;
the ledger manager and execution registry currently share the app process.

## Enqueue and inspect

```rust,ignore
use taim::android::{Request, ServiceError, WorkNetwork, WorkSpec};
use serde_json::json;
let client = taim::android::services().ok_or(ServiceError::Unavailable)?;
let mut spec = WorkSpec::once(42, "sync", json!({"account_id":"example"}));
spec.constraints.network = WorkNetwork::Connected;
let queued = client.request(Request::EnqueueWork(spec)).await?;
let info = client.request(Request::WorkInfo(42)).await?;
```

Logical job IDs are 1–4095. The implementation reserves Android scheduler IDs
`0x54410000 | id`; do not use that range for another scheduler integration. It
checks the service component before replacing/cancelling scheduler entries, so
it will reject a collision with another component rather than cancel it.

| Request | Result |
| --- | --- |
| EnqueueWork(WorkSpec) | Durable job info for the kept/new generation |
| WorkInfo(id) | Full job info, or null if missing/pruned |
| WorkList { include_finished } | Bounded metadata list; excludes payloads |
| CancelWork(id) | null; cancels active work and fences late progress/output |
| PruneWork | `{removed}` for terminal history only |
| ReconcileWork | `{rescheduled}` for eligible ledger entries missing from JobScheduler |

Full info includes id, generation, worker, state, attempt, runs, maximum_attempts,
updated_ms, last_error, scheduled, input, progress and output. Metadata lists
omit input/progress/output. States are queued, running, retry, succeeded, failed
and cancelled; WorkState is also exported for typed state handling. OS state and
the ledger are observed across separate services, so short transitions are
eventually consistent.

Keep returns an existing active generation unchanged. Replace allocates a new
generation, stops the previous execution and updates the OS schedule. Completion,
progress and scheduler cancellation from an older generation cannot overwrite or
cancel the replacement. A terminal ID may be enqueued again with a new generation.
CancelWork does not undo an already completed job or its external effects.

At most 64 active jobs and 128 retained terminal records are kept. Enqueue and
reconciliation prune older terminal history. Inputs/outputs are JSON objects
limited to 16 KiB each; progress objects are limited to 4 KiB. Data is stored in
the application's private ledger, without automatic encryption. Use small
references to application data rather than treating the job ledger as a database
for large documents or secrets.

## Scheduling and retry semantics

One-off delay is 0–7 days. Periodic intervals are 15 minutes–7 days, with flex at
least five minutes or 5% of the interval, whichever is larger. Charging, battery-
not-low, storage-not-low and network class constraints are supported. Network
classes are None, Connected, Unmetered, NotRoaming and Metered. These are OS
eligibility constraints, not guaranteed start times or exact alarms. Android
quotas, Doze, standby buckets, power policy and OEM behavior can delay execution.
See [JobInfo.Builder](https://developer.android.com/reference/android/app/job/JobInfo.Builder).

Retry uses linear or exponential backoff, initially 10 seconds–5 hours, with a
1–16 delivery-attempt cap. Success ends a one-off job; periodic success preserves
its OS schedule and resets the attempt count for the next cycle. Failure is
terminal, including for periodic jobs. Retry reschedules under OS backoff until
the attempt limit, then fails. Native runtime execution/setup errors fail closed
with bounded error codes rather than exposing exception text or inputs.

Workers have a cooperative 1–120 second delivery budget, including time spent
waiting for an available worker slot. Two worker threads and two queued deliveries
are allowed. The serialized ledger queue is also bounded. Progress admission is
throttled to four updates per second; Busy lets callers coalesce updates instead
of accumulating an unbounded queue. Success acknowledges stored output; progress
acknowledges queue admission and may be discarded after cancellation.

## Cancellation, lifecycle and durability

WorkContext exposes the immutable logical ID/generation, handler name, attempt,
input and live application files/cache paths. `is_stopped()` becomes true on OS
stop, explicit cancellation/replacement, timeout or service destruction.
`set_progress()` and `bind_socket()` fail after stop. Check cancellation around
bounded I/O chunks and return promptly. The framework cannot forcibly terminate
a Rust thread safely. If application code ignores cancellation, its late output
is rejected, but its own filesystem/network side effects cannot be rolled back.

Delivery is **at least once**. Process death can occur after a side effect and
before completion is committed. Make operations idempotent and use durable
application fencing/deduplication where needed. Attempt limits do not make
external effects exactly once. Dropping an enqueue future does not cancel a
durable job; query its logical ID or explicitly CancelWork if acknowledgement was
lost.

JobService callbacks stay short; ledger access, native library loading and Rust
execution happen off the main thread. OS stop before the ledger claim preserves
a retryable job without consuming an attempt. OS stop after a claim terminates
the execution token and records retry/failure. Late native returns are ignored.
An abandoned process's running row can be claimed on redelivery without an
Activity-side worker registration step.

JobScheduler persists opted-in jobs across reboot. A process-local reconciliation
pass runs when the registered feature initializes, repairing active records
whose scheduler entry disappeared, including the store/schedule crash window.
ReconcileWork performs that repair explicitly too. Queries do not silently
re-enqueue jobs. Force-stop, package removal, OS quota/restriction and user-unlock
rules still apply; this does not bypass Android's stopped-app restrictions.
See [JobScheduler](https://developer.android.com/reference/android/app/job/JobScheduler)
and [JobService lifecycle](https://developer.android.com/reference/android/app/job/JobService).

On API 28+, WorkInput optionally includes the OS-assigned network handle.
`cx.bind_socket(fd)` binds an unconnected socket to that network using the NDK
without changing global process/UI routing. No assigned handle returns
NetworkUnavailable. DNS and connection setup must also use appropriate network-
aware application code; socket binding alone does not implement a HTTP client.
Before API 28, no assigned job-network handle is exposed by this adapter.

## Verification and boundaries

```bash
python3 scripts/android/counter.py build --app work-probe --abi x86_64
python3 scripts/android/work-probe.py --serial YOUR_TEST_DEVICE \
  --apk target/android/x86_64/debug/work-probe/taim-work-probe.apk \
  --output target/android/runtime-smoke/work-new
```

The isolated suite checks keep/success/progress, retry/exhaustion, permanent
failure, replacement fencing, cooperative cancel/timeout, periodic OS constraints,
missing-schedule reconciliation, OS stop before/after claim and history pruning.
It then terminates only the fixture process, forces its scheduled OS job and
verifies a new process executes Rust with no Activity. A separate instrumentation
phase checks the committed output and that the one-off scheduler entry is gone.
No outbound network request is made; an assigned-network socket is only bound.

The API 35 x86_64/16 KiB guest passed 14 scheduler/lifecycle checks plus three
headless process-restart checks. Host tests passed 58 cases. Physical/OEM policy,
natural periodic/Doze timing, real reboot/unlock and API 26 runtime remain separate
acceptance gates. Forced test execution validates integration, not that Android
will honor an exact wall-clock time or an otherwise unmet constraint in normal use.

Targeted host/ARM64 strict Clippy and a generated ARM64/x86_64 debug project passed.
Worker JNI exports, the protected service manifest, APK signature and 16 KiB ZIP
alignment verified. Existing platform/notification regression passed 14 checks
plus one cold-intent check. Source/artifact hashes, worker PIDs and reports are
captured in the [acceptance manifest](work-acceptance-20261030.json).

This milestone does not implement WorkManager-compatible DAGs, expedited/user-
initiated jobs, foreground services, exact alarms, push/FCM, background location
or a general Activity-independent native plugin registry. Those are separate
capabilities; the worker dispatcher can execute application-defined Rust work now.
