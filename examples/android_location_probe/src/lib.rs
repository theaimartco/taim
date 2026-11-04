//! Foreground location example; automation injects synthetic OS test-provider samples.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(activity: taim::android::AndroidApp) {
    use taim::android::{
        LocationAccuracy, LocationFix, LocationOptions, LocationWatchOptions, Request, ServiceError,
    };
    use taim::prelude::*;
    let result = taim::app("Taim location", |cx| {
        let status = cx.signal("Location ready".to_owned());
        let permissions = cx.signal("Location permission: unknown".to_owned());
        let active = cx.signal(0_u64);
        let samples = cx.signal(0_u64);
        let mocked = cx.signal(false);
        let ended = cx.signal("Location ended: none".to_owned());
        let watch = cx.signal(None::<u64>);
        let tasks = cx.tasks();
        let refresh = tasks.clone();
        let cached = tasks.clone();
        let stop = tasks.clone();
        let drop_watch = tasks.clone();
        let drop_current = tasks.clone();
        let capacity = tasks.clone();
        let permission_request = tasks.clone();
        let current = move |timeout_ms| {
            let _ = status.set("Location running".into());
            let _ = tasks.spawn(async move {
                let result = async {
                    let client = taim::android::services().ok_or(ServiceError::Unavailable)?;
                    client
                        .current_location(LocationOptions {
                            accuracy: LocationAccuracy::High,
                            maximum_age_ms: 0,
                            timeout_ms,
                        })
                        .await
                }
                .await;
                let _ = status.set(match result {
                    Ok(fix) => {
                        let _ = mocked.set(fix.mocked);
                        if fix.precise_permission {
                            "Location current precise".into()
                        } else {
                            "Location current approximate".into()
                        }
                    }
                    Err(e) => format!("Location error: {e:?}"),
                });
            });
        };
        let timeout = current.clone();
        let watch_tasks = cx.tasks();
        let start_watch = move |duration_ms| {
            let _ = ended.set("Location ended: none".into());
            let _ = watch_tasks.spawn(async move {
                let result = async {
                    let client = taim::android::services().ok_or(ServiceError::Unavailable)?;
                    let mut subscription = client
                        .watch_location(LocationWatchOptions {
                            accuracy: LocationAccuracy::High,
                            interval_ms: 1000,
                            minimum_distance_m: 0.0,
                            duration_ms,
                        })
                        .await?;
                    let _ = watch.set(Some(subscription.id()));
                    let _ = status.set("Location watching".into());
                    while let Some(value) = subscription.next().await {
                        match value {
                            Ok(fix) => {
                                samples.update(|n| *n += 1);
                                let _ = mocked.set(fix.mocked);
                            }
                            Err(reason) => {
                                let _ = ended.set(format!("Location ended: {reason:?}"));
                            }
                        }
                    }
                    let _ = watch.set(None);
                    Ok::<_, ServiceError>(())
                }
                .await;
                if let Err(error) = result {
                    let _ = status.set(format!("Location error: {error:?}"));
                }
            });
        };
        let short = start_watch.clone();
        column(views![
            text("Foreground location").size(24),
            text(move || status.get()),
            text(move || permissions.get()),
            text(move || format!("Location active: {}", active.get())),
            text(move || format!("Location samples: {}", samples.get())),
            text(move || format!("Location mock: {}", mocked.get())),
            text(move || ended.get()),
            button("Refresh location status").on_press(move || {
                let _ = refresh.spawn(async move {
                    if let Some(client) = taim::android::services()
                        && let Ok(value) = client.request(Request::LocationStatus).await
                    {
                        let _ = active.set(value["active"].as_u64().unwrap_or(0));
                        let _ = permissions.set(format!(
                            "Location permission: {}",
                            if value["fineGranted"] == true {
                                "precise"
                            } else if value["coarseGranted"] == true {
                                "approximate"
                            } else {
                                "denied"
                            }
                        ));
                    }
                });
            }),
            button("Request location permission").on_press(move || {
                let _ = permission_request.spawn(async move {
                    if let Some(client) = taim::android::services() {
                        let _ = client
                            .request(Request::Permissions(vec![
                                "android.permission.ACCESS_COARSE_LOCATION".into(),
                                "android.permission.ACCESS_FINE_LOCATION".into(),
                            ]))
                            .await;
                    }
                });
            }),
            button("Cached location").on_press(move || {
                let _ = cached.spawn(async move {
                    let result = async {
                        let client = taim::android::services().ok_or(ServiceError::Unavailable)?;
                        client
                            .request(Request::LastLocation {
                                accuracy: LocationAccuracy::High,
                                maximum_age_ms: 1000,
                            })
                            .await
                    }
                    .await;
                    let _ = status.set(match result {
                        Ok(value) if value.is_null() => "Location cache empty".into(),
                        Ok(value) => match LocationFix::from_value(value) {
                            Some(fix) => {
                                let _ = mocked.set(fix.mocked);
                                "Location cached".into()
                            }
                            None => "Location error: Failed".into(),
                        },
                        Err(e) => format!("Location error: {e:?}"),
                    });
                });
            }),
            button("Current location").on_press(move || current(5000)),
            button("Location timeout").on_press(move || timeout(100)),
            button("Drop current location").on_press(move || {
                let _ = drop_current.spawn(async move {
                    let result = async {
                        let client = taim::android::services().ok_or(ServiceError::Unavailable)?;
                        let mut future = Box::pin(client.current_location(LocationOptions {
                            accuracy: LocationAccuracy::High,
                            maximum_age_ms: 0,
                            timeout_ms: 30_000,
                        }));
                        if futures::poll!(&mut future).is_ready() {
                            return Err(ServiceError::Failed);
                        }
                        drop(future);
                        Ok::<_, ServiceError>(())
                    }
                    .await;
                    let _ = status.set(match result {
                        Ok(()) => "Location current dropped".into(),
                        Err(e) => format!("Location error: {e:?}"),
                    });
                });
            }),
            button("Watch location").on_press(move || start_watch(30_000)),
            button("Watch short").on_press(move || short(200)),
            button("Stop location").on_press(move || {
                let _ = stop.spawn(async move {
                    if let Some(id) = watch.get()
                        && let Some(client) = taim::android::services()
                    {
                        let _ = client.request(Request::StopLocation(id)).await;
                    }
                });
            }),
            button("Drop location watch").on_press(move || {
                let _ = drop_watch.spawn(async move {
                    let result = async {
                        let client = taim::android::services().ok_or(ServiceError::Unavailable)?;
                        let subscription = client
                            .watch_location(LocationWatchOptions {
                                accuracy: LocationAccuracy::High,
                                interval_ms: 1000,
                                minimum_distance_m: 0.0,
                                duration_ms: 30_000,
                            })
                            .await?;
                        drop(subscription);
                        Ok::<_, ServiceError>(())
                    }
                    .await;
                    let _ = status.set(match result {
                        Ok(()) => "Location watch dropped".into(),
                        Err(e) => format!("Location error: {e:?}"),
                    });
                });
            }),
            button("Location capacity").on_press(move || {
                let _ = capacity.spawn(async move {
                    let result = async {
                        let client = taim::android::services().ok_or(ServiceError::Unavailable)?;
                        let options = LocationWatchOptions {
                            accuracy: LocationAccuracy::High,
                            interval_ms: 1000,
                            minimum_distance_m: 0.0,
                            duration_ms: 30_000,
                        };
                        let mut tokens = Vec::new();
                        for _ in 0..4 {
                            tokens.push(
                                client.request(Request::WatchLocation(options)).await?["watch"]
                                    .as_u64()
                                    .ok_or(ServiceError::Failed)?,
                            );
                        }
                        let full = client.request(Request::WatchLocation(options)).await
                            == Err(ServiceError::Busy);
                        for id in tokens {
                            let _ = client.request(Request::StopLocation(id)).await?;
                        }
                        if !full {
                            return Err(ServiceError::Failed);
                        }
                        Ok::<_, ServiceError>(())
                    }
                    .await;
                    let _ = status.set(match result {
                        Ok(()) => "Location capacity handled".into(),
                        Err(e) => format!("Location error: {e:?}"),
                    });
                });
            }),
        ])
        .padding(16.0)
        .gap(6.0)
    })
    .run_android(activity);
    if let Err(error) = result {
        eprintln!("Location probe failed: {error}");
    }
}
