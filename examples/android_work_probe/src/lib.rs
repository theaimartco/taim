//! Durable headless work fixture. No outbound network requests are made.
#[cfg(target_os = "android")]
taim::android::background_worker!(work);

#[cfg(target_os = "android")]
fn work(context: &mut taim::android::WorkContext<'_>) -> taim::android::WorkResult {
    use serde_json::json;
    use taim::android::WorkResult;
    let _ = context.set_progress(json!({"phase":"running"}));
    let worker = context.input.worker.clone();
    if worker == "retry" && context.input.attempt == 1 || worker == "always_retry" {
        return WorkResult::Retry;
    }
    if worker == "failure" {
        return WorkResult::Failure(json!({"code":"fixtureFailure"}));
    }
    if worker == "slow" {
        for _ in 0..500 {
            if context.is_stopped() {
                let path = std::path::Path::new(&context.input.files_dir).join(format!(
                    "work-fixture-stopped-{}-{}",
                    context.input.id, context.input.generation
                ));
                let _ = std::fs::write(path, b"stopped");
                return WorkResult::Failure(json!({"code":"stopped"}));
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    // Deliberately noncooperative fixture: late output must be fenced after replacement.
    if worker == "stubborn" {
        std::thread::sleep(std::time::Duration::from_millis(2000));
    }
    let socket_bound = if context.input.network_handle.is_some() {
        use std::os::fd::AsRawFd as _;
        std::net::UdpSocket::bind("0.0.0.0:0")
            .ok()
            .map(|socket| context.bind_socket(socket.as_raw_fd()).is_ok())
    } else {
        None
    };
    let output = json!({"label":context.input.input["label"],"attempt":context.input.attempt,"pid":std::process::id(),
        "headless":taim::android::services().is_none(),"socket_bound":socket_bound});
    let path = std::path::Path::new(&context.input.files_dir).join(format!(
        "work-fixture-result-{}-{}.json",
        context.input.id, context.input.generation
    ));
    if std::fs::write(path, output.to_string()).is_err() {
        return WorkResult::Failure(json!({"code":"fixtureWriteFailed"}));
    }
    WorkResult::Success(output)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(activity: taim::android::AndroidApp) {
    use serde_json::json;
    use taim::android::{
        Request, ServiceError, WorkBackoff, WorkNetwork, WorkPolicy, WorkSchedule, WorkSpec,
    };
    use taim::prelude::*;
    let result = taim::app("Taim work", |cx| {
        let status = cx.signal("Work ready".to_owned());
        let selected = cx.signal(1_u16);
        let info = cx.signal(json!({}));
        let revision = cx.signal(0_u64);
        let tasks = cx.tasks();
        let refresh = tasks.clone();
        let cancel = tasks.clone();
        let prune = tasks.clone();
        let reconcile = tasks.clone();
        let enqueue = move |id: u16,
                            worker: &'static str,
                            label: &'static str,
                            replace: bool,
                            periodic: bool,
                            attempts: u8,
                            timeout: u32| {
            let _ = status.set("Work busy".into());
            let _ = selected.set(id);
            let _ = tasks.spawn(async move {
                let result = async {
                    let client = taim::android::services().ok_or(ServiceError::Unavailable)?;
                    let mut spec = WorkSpec::once(id, worker, json!({"label":label}));
                    spec.schedule = if periodic {
                        WorkSchedule::Periodic {
                            interval_ms: 900_000,
                            flex_ms: 300_000,
                        }
                    } else {
                        WorkSchedule::Once { delay_ms: 600_000 }
                    };
                    spec.policy = if replace {
                        WorkPolicy::Replace
                    } else {
                        WorkPolicy::Keep
                    };
                    spec.maximum_attempts = attempts;
                    spec.execution_timeout_ms = timeout;
                    spec.backoff = WorkBackoff::Linear(10_000);
                    if periodic {
                        spec.constraints.network = WorkNetwork::Connected;
                        spec.constraints.charging = true;
                        spec.constraints.battery_not_low = true;
                        spec.constraints.storage_not_low = true;
                    }
                    client.request(Request::EnqueueWork(spec)).await
                }
                .await;
                let _ = status.set(match result {
                    Ok(value) => {
                        let _ = info.set(value);
                        format!("Work enqueued: {label}")
                    }
                    Err(e) => format!("Work error: {e:?}"),
                });
            });
        };
        let kept = enqueue.clone();
        let retry = enqueue.clone();
        let exhausted = enqueue.clone();
        let fail = enqueue.clone();
        let stubborn = enqueue.clone();
        let replace = enqueue.clone();
        let slow = enqueue.clone();
        let timeout = enqueue.clone();
        let periodic = enqueue.clone();
        let recover = enqueue.clone();
        let headless = enqueue.clone();
        let prestart = enqueue.clone();
        let os_stop = enqueue.clone();
        scroll(
            column(views![
                text("Durable background work").size(24),
                text(move || status.get()),
                text(move || format!(
                    "Work state: {}",
                    info.get()["state"].as_str().unwrap_or("none")
                )),
                text(move || format!("Work revision: {}", revision.get())),
                button("Enqueue success")
                    .on_press(move || enqueue(1, "success", "first", false, false, 3, 10_000)),
                button("Keep existing")
                    .on_press(move || kept(1, "failure", "kept", false, false, 3, 10_000)),
                button("Enqueue retry")
                    .on_press(move || retry(2, "retry", "retry", true, false, 3, 10_000)),
                button("Exhaust attempts").on_press(move || exhausted(
                    3,
                    "always_retry",
                    "exhausted",
                    true,
                    false,
                    2,
                    10_000
                )),
                button("Permanent failure")
                    .on_press(move || fail(4, "failure", "failure", true, false, 3, 10_000)),
                button("Start old worker")
                    .on_press(move || stubborn(1, "stubborn", "old", true, false, 3, 10_000)),
                button("Replace worker")
                    .on_press(move || replace(1, "success", "new", true, false, 3, 10_000)),
                button("Enqueue slow")
                    .on_press(move || slow(5, "slow", "slow", true, false, 3, 20_000)),
                button("Enqueue timeout")
                    .on_press(move || timeout(6, "slow", "timeout", true, false, 1, 1000)),
                button("Enqueue periodic")
                    .on_press(move || periodic(7, "success", "periodic", true, true, 3, 10_000)),
                button("Enqueue recovery")
                    .on_press(move || recover(8, "success", "recovery", true, false, 3, 10_000)),
                button("Enqueue prestart stop")
                    .on_press(move || prestart(9, "success", "prestart", true, false, 3, 10_000)),
                button("Enqueue OS stop")
                    .on_press(move || os_stop(10, "slow", "osstop", true, false, 3, 20_000)),
                button("Prepare headless")
                    .on_press(move || headless(17, "success", "headless", true, false, 3, 10_000)),
                button("Refresh work").on_press(move || {
                    let _ = refresh.spawn(async move {
                        if let Some(client) = taim::android::services() {
                            match client.request(Request::WorkInfo(selected.get())).await {
                                Ok(value) => {
                                    let _ = info.set(value);
                                    revision.update(|n| *n += 1);
                                }
                                Err(e) => {
                                    let _ = status.set(format!("Work error: {e:?}"));
                                }
                            }
                        }
                    });
                }),
                button("Cancel current work").on_press(move || {
                    let _ = cancel.spawn(async move {
                        if let Some(client) = taim::android::services() {
                            let result = client.request(Request::CancelWork(selected.get())).await;
                            let _ = status.set(if result.is_ok() {
                                "Work cancelled".into()
                            } else {
                                "Work cancellation failed".into()
                            });
                        }
                    });
                }),
                button("Prune history").on_press(move || {
                    let _ = prune.spawn(async move {
                        if let Some(client) = taim::android::services() {
                            let result = client.request(Request::PruneWork).await;
                            let _ = status.set(if result.is_ok() {
                                "Work pruned".into()
                            } else {
                                "Work prune failed".into()
                            });
                        }
                    });
                }),
                button("Reconcile work").on_press(move || {
                    let _ = reconcile.spawn(async move {
                        if let Some(client) = taim::android::services() {
                            let result = client.request(Request::ReconcileWork).await;
                            let _ = status.set(if result.is_ok() {
                                "Work reconciled".into()
                            } else {
                                "Work reconcile failed".into()
                            });
                        }
                    });
                }),
            ])
            .padding(12.0)
            .gap(3.0),
        )
    })
    .run_android(activity);
    if let Err(error) = result {
        eprintln!("Work probe failed: {error}");
    }
}
