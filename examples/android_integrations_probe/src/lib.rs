//! Physical-device acceptance fixture for the framework-only Android integrations:
//! biometric, BLE, NFC, geofencing, foreground service, inline video and camera preview.
//! Every result is logged as `TAIM_PROBE <step> <result>` for scripts/android automation.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(activity: taim::android::AndroidApp) {
    use taim::android::{
        BiometricKey, BiometricKeyKind, BiometricPrompt, BleScanOptions, BluetoothEvent,
        ForegroundService, ForegroundServiceType, Geofence, GeofenceTransition, LocationAccuracy,
        LocationOptions, NfcTag, NotificationChannel, NotificationImportance, PlatformEvent,
        Request, ServiceError, Services, services,
    };
    use taim::prelude::*;

    fn log(step: &str, result: impl std::fmt::Display) {
        eprintln!("TAIM_PROBE {step} {result}");
    }

    let result = taim::app("Taim integrations", |cx| {
        let last = cx.signal("Ready".to_owned());
        let devices = cx.signal(Vec::<String>::new());
        let video_playing = cx.signal(false);
        let camera_on = cx.signal(false);
        let camera_front = cx.signal(false);
        let tasks = cx.tasks();
        let report = move |step: &'static str, value: String| {
            log(step, &value);
            let _ = last.set(format!("{step}: {value}"));
        };

        // One long-lived listener for plugin and native-view events.
        {
            let _ = tasks.spawn(async move {
                use futures::StreamExt as _;
                let Some(client) = services() else { return };
                let Ok(mut events) = client.events() else {
                    return;
                };
                while let Some(event) = events.next().await {
                    match event {
                        PlatformEvent::Plugin { channel, event } => {
                            if let Some(e) = BluetoothEvent::decode(&channel, &event) {
                                log("event-bluetooth", format!("{e:?}"));
                            } else if let Some(tag) = NfcTag::decode(&channel, &event) {
                                report(
                                    "nfc-tag",
                                    format!(
                                        "id={} techs={:?} ndef={}",
                                        tag.id,
                                        tag.techs,
                                        tag.ndef.map_or(0, |n| n.records.len())
                                    ),
                                );
                            } else if let Some(t) = GeofenceTransition::decode(&channel, &event) {
                                report(
                                    "geofence-transition",
                                    format!("{} entering={}", t.id, t.entering),
                                );
                            } else {
                                log("event-plugin", format!("{channel} {event}"));
                            }
                        }
                        PlatformEvent::NativeView { id, event, .. } => {
                            log("event-view", format!("{id} {event}"))
                        }
                        _ => {}
                    }
                }
            });
        }

        // Run an async step with the Services client and report its Debug result.
        macro_rules! step {
            ($name:literal, |$client:ident| $body:expr) => {{
                let tasks = tasks.clone();
                move || {
                    let _ = tasks.spawn(async move {
                        let outcome: Result<String, ServiceError> = async {
                            let $client: Services = services().ok_or(ServiceError::Unavailable)?;
                            $body
                        }
                        .await;
                        report(
                            $name,
                            match outcome {
                                Ok(v) => format!("Ok {v}"),
                                Err(e) => format!("Err({e:?})"),
                            },
                        );
                    });
                }
            }};
        }

        let permissions = step!("permissions", |c| {
            let names = [
                "android.permission.BLUETOOTH_SCAN",
                "android.permission.BLUETOOTH_CONNECT",
                "android.permission.CAMERA",
                "android.permission.ACCESS_FINE_LOCATION",
                "android.permission.POST_NOTIFICATIONS",
            ];
            let statuses = c
                .request_permissions(names.iter().map(|n| n.to_string()).collect())
                .await?;
            Ok(statuses
                .iter()
                .map(|s| {
                    format!(
                        "{}={}",
                        s.name.trim_start_matches("android.permission."),
                        s.granted
                    )
                })
                .collect::<Vec<_>>()
                .join(" "))
        });
        let background = step!("background-location", |c| {
            let s = c
                .request_permissions(vec!["android.permission.ACCESS_BACKGROUND_LOCATION".into()])
                .await?;
            Ok(format!("{:?}", s.first().map(|s| s.granted)))
        });
        let bio_sign = step!("biometric-sign", |c| {
            let public = c
                .biometric_key_create(&BiometricKey {
                    alias: "probe.login".into(),
                    kind: BiometricKeyKind::Sign,
                    device_credential: false,
                    invalidate_on_enrollment: true,
                })
                .await?;
            let signature = c
                .biometric_sign(
                    "probe.login",
                    b"server-challenge-123",
                    &BiometricPrompt::new("Sign in", "Cancel"),
                )
                .await?;
            Ok(format!(
                "public_key={}B signature={}B",
                public.map_or(0, |k| k.len()),
                signature.len()
            ))
        });
        let bio_vault = step!("biometric-vault", |c| {
            c.biometric_key_create(&BiometricKey {
                alias: "probe.vault".into(),
                kind: BiometricKeyKind::Encrypt,
                device_credential: false,
                invalidate_on_enrollment: true,
            })
            .await?;
            let sealed = c
                .biometric_encrypt(
                    "probe.vault",
                    b"secret token",
                    &BiometricPrompt::new("Lock secret", "Cancel"),
                )
                .await?;
            let opened = c
                .biometric_decrypt(
                    "probe.vault",
                    &sealed,
                    &BiometricPrompt::new("Unlock secret", "Cancel"),
                )
                .await?;
            Ok(format!("roundtrip={}", opened == b"secret token"))
        });
        let biometric = step!("biometric", |c| Ok(format!(
            "{:?}",
            c.authenticate(BiometricPrompt::new("Taim probe", "Cancel"))
                .await?
        )));
        let ble_status = step!("ble-status", |c| Ok(format!(
            "{:?}",
            c.bluetooth_status().await?
        )));
        let ble_scan = step!("ble-scan", |c| {
            let found = c
                .ble_scan(&BleScanOptions {
                    duration_ms: 8000,
                    services: vec![],
                    name_prefix: None,
                })
                .await?;
            let _ = devices.set(
                found
                    .iter()
                    .filter(|d| d.connectable)
                    .map(|d| d.address.clone())
                    .collect(),
            );
            Ok(format!(
                "{} devices; first: {:?}",
                found.len(),
                found
                    .iter()
                    .take(3)
                    .map(|d| (&d.address, &d.name, d.rssi))
                    .collect::<Vec<_>>()
            ))
        });
        let ble_connect = step!("ble-connect", |c| {
            let address = devices
                .get()
                .first()
                .cloned()
                .ok_or(ServiceError::NotFound)?;
            let connection = c.ble_connect(&address, 15000).await?;
            let summary = format!(
                "{address} handle={} services={}",
                connection.handle,
                connection.services.len()
            );
            // Device Name (0x2A00) in Generic Access (0x1800) is readable on most peripherals.
            let name = c
                .ble_read(
                    connection.handle,
                    "00001800-0000-1000-8000-00805f9b34fb",
                    "00002a00-0000-1000-8000-00805f9b34fb",
                )
                .await;
            c.ble_disconnect(connection.handle).await?;
            Ok(format!(
                "{summary} name={:?}",
                name.map(|b| String::from_utf8_lossy(&b).into_owned())
            ))
        });
        let nfc = step!("nfc-start", |c| {
            let status = c.nfc_status().await?;
            if status.supported && status.enabled {
                c.nfc_start_reader().await?;
            }
            Ok(format!("{status:?}"))
        });
        let geofence = step!("geofence-add", |c| {
            let fix = c
                .current_location(LocationOptions {
                    accuracy: LocationAccuracy::High,
                    maximum_age_ms: 60_000,
                    timeout_ms: 20_000,
                })
                .await?;
            c.geofence_add(&Geofence {
                id: "probe.here".into(),
                latitude: fix.latitude,
                longitude: fix.longitude,
                radius_m: 100.0,
                expiration_ms: Some(3_600_000),
            })
            .await?;
            let listed = c.geofence_list().await?;
            Ok(format!(
                "{:?} regions={} pending={:?}",
                c.geofence_status().await?,
                listed.len(),
                c.geofence_pending().await?
            ))
        });
        let geofence_remove = step!("geofence-remove", |c| {
            c.geofence_remove("probe.here").await?;
            Ok(format!("regions={}", c.geofence_list().await?.len()))
        });
        let foreground = step!("foreground", |c| {
            c.request(Request::NotificationChannel(NotificationChannel {
                id: "probe.sync".into(),
                name: "Probe sync".into(),
                description: "Foreground service probe".into(),
                importance: NotificationImportance::Low,
            }))
            .await?;
            let service = ForegroundService {
                channel: "probe.sync".into(),
                title: "Taim probe".into(),
                text: "Foreground service running".into(),
                service_type: ForegroundServiceType::DataSync,
            };
            c.foreground_start(&service).await?;
            // startForeground runs asynchronously in the service; poll briefly.
            let mut running = false;
            for _ in 0..20 {
                if c.foreground_running().await? {
                    running = true;
                    break;
                }
                let _ = futures_timer(100).await;
            }
            Ok(format!("running={running}"))
        });
        let foreground_stop = step!("foreground-stop", |c| {
            c.foreground_stop().await?;
            Ok("stopped".into())
        });

        let video = android_video_view(move || {
            let mut v = AndroidVideo::new(VideoSource::Https(
                "https://storage.googleapis.com/exoplayer-test-media-0/BigBuckBunny_320x180.mp4"
                    .into(),
            ));
            v.playing = video_playing.get();
            v.muted = true;
            v
        })
        .expect("valid video")
        .height(200.0);
        let camera = android_camera_preview(move || {
            (
                if camera_front.get() {
                    CameraLens::Front
                } else {
                    CameraLens::Back
                },
                camera_on.get(),
            )
        })
        .expect("valid camera")
        .height(260.0);

        scroll(
            column(views![
                text("Taim integrations probe").size(24),
                text(move || last.get()).size(16),
                row(views![
                    button("Permissions").on_press(permissions),
                    button("Bg location").on_press(background)
                ])
                .gap(8.0),
                row(views![
                    button("Biometric").on_press(biometric),
                    button("NFC").on_press(nfc)
                ])
                .gap(8.0),
                row(views![
                    button("Bio sign").on_press(bio_sign),
                    button("Bio vault").on_press(bio_vault)
                ])
                .gap(8.0),
                row(views![
                    button("BLE status").on_press(ble_status),
                    button("BLE scan").on_press(ble_scan),
                    button("BLE connect").on_press(ble_connect)
                ])
                .gap(8.0),
                row(views![
                    button("Geofence add").on_press(geofence),
                    button("Geofence remove").on_press(geofence_remove)
                ])
                .gap(8.0),
                row(views![
                    button("FGS start").on_press(foreground),
                    button("FGS stop").on_press(foreground_stop)
                ])
                .gap(8.0),
                row(views![
                    button("Video play/pause").on_press(move || video_playing.update(|p| *p = !*p))
                ])
                .gap(8.0),
                video,
                row(views![
                    button("Camera on/off").on_press(move || camera_on.update(|c| *c = !*c)),
                    button("Flip").on_press(move || camera_front.update(|f| *f = !*f)),
                ])
                .gap(8.0),
                camera,
            ])
            .gap(12.0)
            .padding(20.0),
        )
    })
    .run_android(activity);
    if let Err(error) = result {
        eprintln!("Taim Android failed: {error}");
    }

    /// Minimal sleep without pulling in a timer crate: a thread signals a oneshot.
    fn futures_timer(ms: u64) -> futures::channel::oneshot::Receiver<()> {
        let (tx, rx) = futures::channel::oneshot::channel();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(ms));
            let _ = tx.send(());
        });
        rx
    }
}
