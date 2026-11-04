//! Explicit test-device fixture; production hosts do not run these operations.
#[cfg(target_os = "android")]
async fn notifications(
    client: taim::android::Services,
) -> Result<bool, taim::android::ServiceError> {
    use taim::android::{
        Notification, NotificationChannel, NotificationImportance, NotificationVisibility, Request,
        ServiceError,
    };
    let channel = "probe.notifications.v1";
    for importance in [NotificationImportance::Low, NotificationImportance::High] {
        let result = client
            .request(Request::NotificationChannel(NotificationChannel {
                id: channel.into(),
                name: "Taim fixture".into(),
                description: "Test-device notifications".into(),
                importance,
            }))
            .await?;
        if result["importance"] != 2 {
            return Err(ServiceError::Failed);
        }
    }
    let permission = client
        .request(Request::PermissionStatus(vec![
            "android.permission.POST_NOTIFICATIONS".into(),
        ]))
        .await?;
    let granted = permission[0]["granted"] == true;
    for id in [4101, 4101, 4102] {
        let result = client
            .request(Request::NotificationPost(Notification {
                id,
                channel: channel.into(),
                title: format!("Taim fixture {id}"),
                body: "Local notification test".into(),
                small_icon: None,
                payload: Some(format!("route:{id}")),
                visibility: NotificationVisibility::Private,
                auto_cancel: false,
                only_alert_once: true,
                timeout_ms: Some(120_000),
            }))
            .await;
        if !granted {
            return if result == Err(ServiceError::Denied) {
                Ok(false)
            } else {
                Err(ServiceError::Failed)
            };
        }
        result?;
    }
    let status = client
        .request(Request::NotificationStatus {
            channel: Some(channel.into()),
        })
        .await?;
    // Posting acknowledges dispatch; Android publishes active shade entries asynchronously.
    // The external driver waits for and checks those actual entries separately.
    if status["enabled"] != true {
        return Err(ServiceError::Failed);
    }
    Ok(true)
}

#[cfg(target_os = "android")]
async fn probe(client: taim::android::Services) -> Result<(), taim::android::ServiceError> {
    use futures::StreamExt as _;
    use taim::android::{Haptic, PlatformEvent, Request, Services};
    let mut events = client.events()?;
    let info = client.request(Request::DeviceInfo).await?;
    if info["sdk"].as_u64().unwrap_or(0) < 26 {
        return Err(taim::android::ServiceError::Failed);
    }
    let directories = client.request(Request::Directories).await?;
    if directories["files"].as_str().is_none() {
        return Err(taim::android::ServiceError::Failed);
    }
    let _ = client
        .request(Request::ClipboardWrite {
            text: "Taim platform fixture".into(),
            html: None,
            sensitive: true,
        })
        .await?;
    if client
        .request(Request::ClipboardRead { html: false })
        .await?
        .as_str()
        != Some("Taim platform fixture")
    {
        return Err(taim::android::ServiceError::Failed);
    }
    let bytes = b"test-only secure-store fixture".to_vec();
    let _ = client
        .request(Request::SecurePut {
            key: "probe.fixture".into(),
            bytes: bytes.clone(),
        })
        .await?;
    if Services::bytes(
        &client
            .request(Request::SecureGet("probe.fixture".into()))
            .await?,
    )? != bytes
    {
        return Err(taim::android::ServiceError::Failed);
    }
    let _ = client
        .request(Request::SecureDelete("probe.fixture".into()))
        .await?;
    if !client
        .request(Request::SecureGet("probe.fixture".into()))
        .await?
        .is_null()
    {
        return Err(taim::android::ServiceError::Failed);
    }
    if Services::bytes(
        &client
            .request(Request::ReadAsset {
                name: "probe.txt".into(),
                limit: 1024,
            })
            .await?,
    )? != b"Taim packaged asset\n"
    {
        return Err(taim::android::ServiceError::Failed);
    }
    let permissions = client
        .request(Request::Permissions(vec![
            "android.permission.INTERNET".into(),
        ]))
        .await?;
    if permissions[0]["granted"] != true {
        return Err(taim::android::ServiceError::Failed);
    }
    let states = client
        .permission_status(vec![
            "android.permission.INTERNET".into(),
            "android.permission.RECORD_AUDIO".into(),
        ])
        .await?;
    if !states[0].granted || states[1].state != taim::android::PermissionState::NotDeclared {
        return Err(taim::android::ServiceError::Failed);
    }
    let states = client
        .request_permissions(vec![
            "android.permission.INTERNET".into(),
            "android.permission.RECORD_AUDIO".into(),
        ])
        .await?;
    if !states[0].granted || states[1].can_ask_again {
        return Err(taim::android::ServiceError::Failed);
    }
    for kind in [
        taim::android::SpecialAccess::Overlay,
        taim::android::SpecialAccess::WriteSettings,
        taim::android::SpecialAccess::AllFiles,
        taim::android::SpecialAccess::ExactAlarms,
        taim::android::SpecialAccess::BatteryOptimization,
        taim::android::SpecialAccess::FullScreenIntent,
    ] {
        let _ = client.request(Request::SpecialAccessStatus(kind)).await?;
    }
    let _ = client.request(Request::Haptic(Haptic::Selection)).await?;
    let _ = client.request(Request::LaunchIntent).await?;
    if client
        .request(Request::ReadDocument {
            uri: "file:///not-a-content-uri".into(),
            limit: 32,
        })
        .await
        .is_ok()
    {
        return Err(taim::android::ServiceError::Failed);
    }
    let reply = client
        .request(Request::Plugin {
            channel: "probe.echo".into(),
            method: "echo".into(),
            arguments: serde_json::json!({"fixture":7}),
        })
        .await?;
    if reply["fixture"] != 7 {
        return Err(taim::android::ServiceError::Failed);
    }
    while let Some(event) = events.next().await {
        if matches!(event,PlatformEvent::Plugin{channel,..} if channel=="probe.echo") {
            return Ok(());
        }
    }
    Err(taim::android::ServiceError::Unavailable)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(activity: taim::android::AndroidApp) {
    use taim::prelude::*;
    let result = taim::app("Taim platform probe", |cx| {
        let status = cx.signal("Ready".to_owned());
        let notification_status = cx.signal("Notifications ready".to_owned());
        let notification_route = cx.signal("No notification route".to_owned());
        let page = cx.signal(false);
        cx.on_back(page, move || {
            let _ = page.set(false);
        });
        let count = cx
            .restorable("count", 1, 0_u64)
            .expect("fixed restoration key");
        let permission_seen=cx.signal(false);
        let message = cx.signal(String::new());
        let secret = cx.signal(String::new());
        let tasks = cx.tasks();
        let notification_tasks = tasks.clone();
        let replay_tasks = tasks.clone();
        let cancellation_tasks = tasks.clone();
        let _ = tasks.spawn(async move {
            use futures::StreamExt as _;
            if let Some(client) = taim::android::services()
                && let Ok(mut events) = client.events()
            {
                while let Some(event) = events.next().await {
                    if matches!(&event,taim::android::PlatformEvent::PermissionsChanged(statuses) if statuses.iter().any(|s|s.name=="android.permission.CAMERA")){let _=permission_seen.set(true);}
                    if let taim::android::PlatformEvent::Intent {
                        notification: Some(tap),
                        ..
                    } = event
                    {
                        let _ = notification_route.set(format!(
                            "Notification route: {}:{}",
                            tap.id,
                            tap.payload.unwrap_or_default()
                        ));
                    }
                }
            }
        });
        column(views![
            text(move || (if permission_seen.get(){"Permissions observed"}else{"Android platform probe"}).to_owned()).size(24),
            text(move || format!("Count: {}", count.get())),
            button("Increment").on_press(move || count.update(|count| *count += 1)),
            button("Run services")
                .enabled(move || status.get() != "Running")
                .on_press(move || {
                    let _ = status.set("Running".into());
                    let _ = tasks.spawn(async move {
                        let result = match taim::android::services() {
                            Some(client) => probe(client).await,
                            None => Err(taim::android::ServiceError::Unavailable),
                        };
                        let _ = status.set(if result.is_ok() {
                            "Services passed".into()
                        } else {
                            format!("Services failed: {:?}", result.err())
                        });
                    });
                }),
            text(move || status.get()),
            button("Open page").on_press(move || {
                let _ = page.set(true);
            }),
            text(move || if page.get() {
                "Page open".to_owned()
            } else {
                "Page closed".to_owned()
            }),
            text_field("Probe input", message),
            text(move || format!("Input: {}", message.get())),
            text_field("Probe password", secret)
                .secret()
                .autofill(taim::semantics::Autofill::Password),
            text(move || if secret.get() == "test-only-autofill" {
                "Secure autofill passed".to_owned()
            } else {
                "Secure field ready".to_owned()
            }),
            button("Run notifications").on_press(move || {
                let _ = notification_status.set("Notifications running".into());
                let _ = notification_tasks.spawn(async move {
                    let result = match taim::android::services() {
                        Some(client) => notifications(client).await,
                        None => Err(taim::android::ServiceError::Unavailable),
                    };
                    let _ = notification_status.set(match result {
                        Ok(true) => "Notifications posted".into(),
                        Ok(false) => "Notifications denied passed".into(),
                        Err(error) => format!("Notifications failed: {error}"),
                    });
                });
            }),
            text(move || notification_status.get()),
            text(move || notification_route.get()),
            button("Replay notification").on_press(move || {
                let _ = replay_tasks.spawn(async move {
                    use futures::StreamExt as _;
                    if let Some(client) = taim::android::services()
                        && let Ok(mut events) = client.events()
                    {
                        while let Some(event) = events.next().await {
                            if let taim::android::PlatformEvent::Intent {
                                notification: Some(tap),
                                ..
                            } = event
                            {
                                let _ = notification_route.set(format!(
                                    "Notification replay: {}:{}",
                                    tap.id,
                                    tap.payload.unwrap_or_default()
                                ));
                                break;
                            }
                        }
                    }
                });
            }),
            button("Cancel notifications").on_press(move || {
                let _ = cancellation_tasks.spawn(async move {
                    let result = async {
                        use taim::android::{Request, ServiceError};
                        let client = taim::android::services().ok_or(ServiceError::Unavailable)?;
                        for id in [4101, 4102] {
                            client.request(Request::NotificationCancel(id)).await?;
                        }
                        client
                            .request(Request::NotificationChannelDelete(
                                "probe.notifications.v1".into(),
                            ))
                            .await?;
                        let status = client
                            .request(Request::NotificationStatus {
                                channel: Some("probe.notifications.v1".into()),
                            })
                            .await?;
                        if !status["channel"].is_null() {
                            return Err(ServiceError::Failed);
                        }
                        Ok::<_, ServiceError>(())
                    }
                    .await;
                    let _ = notification_status.set(if result.is_ok() {
                        "Notifications cancelled".into()
                    } else {
                        "Notification cancellation failed".into()
                    });
                });
            }),
        ])
        .padding(16.0)
        .gap(6.0)
    })
    .run_android(activity);
    if let Err(error) = result {
        eprintln!("Platform probe failed: {error}");
    }
}
