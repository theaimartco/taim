//! Explicit test-device fixture for TAIM's Apple services (iOS and macOS).
//! Each check prints a `TAIM_APPLE_PROBE` line to standard output (see `xcrun simctl launch
//! --stdout`) and on screen. Launch arguments add interactive checks:
//! `--share` presents the share sheet, `--pick` the photo picker,
//! `--camera` the camera, `--notify` asks for notification permission,
//! `--ask-location` asks for location permission, `--late` delivers the test notification after 8 s instead of 3 s,
//! `--auth` shows the biometric prompt, `--secure-keychain` stores and reads
//! a Face ID-protected item.
//! Notification events (including a launch tap) are always reported.
//!
//! On other platforms it shows a note and does nothing. On a Mac, run it
//! from an app bundle: notifications and location need one.
use taim::prelude::*;

#[cfg(any(target_os = "ios", target_os = "macos"))]
async fn probe(report: impl Fn(&str, String) + Send + Sync + 'static) {
    use std::time::Duration;
    use taim::apple::{self as ios, *};

    let arguments: Vec<String> = std::env::args().collect();
    let wants = |flag: &str| arguments.iter().any(|argument| argument == flag);

    // Keychain: write, read back, overwrite, delete, read the absence.
    let keychain = (|| {
        let service = "com.theaimartco.taim.probe";
        ios::keychain_set(service, "token", b"first", KeychainAccess::AfterFirstUnlock)?;
        ios::keychain_set(
            service,
            "token",
            b"second",
            KeychainAccess::AfterFirstUnlock,
        )?;
        let read = ios::keychain_get(service, "token", None)?;
        let deleted = ios::keychain_delete(service, "token")?;
        let gone = ios::keychain_get(service, "token", None)?;
        let again = ios::keychain_delete(service, "token")?;
        Ok::<_, ServiceError>(
            read.as_deref() == Some(b"second") && deleted && gone.is_none() && !again,
        )
    })();
    report("keychain_roundtrip", format!("{keychain:?}"));
    report(
        "keychain_invalid",
        format!(
            "{:?}",
            ios::keychain_set("", "a", b"x", KeychainAccess::default())
        ),
    );

    if wants("--auth") {
        report(
            "authenticate",
            format!(
                "{:?}",
                ios::authenticate("Confirm the TAIM probe", BiometricPolicy::Biometrics).await
            ),
        );
    }
    if wants("--secure-keychain") {
        let service = "com.theaimartco.taim.probe.secure";
        let stored = ios::keychain_set(service, "pin", b"4321", KeychainAccess::BiometryCurrentSet);
        report("secure_keychain_set", format!("{stored:?}"));
        let read = ios::keychain_get(service, "pin", Some("Read the probe secret"));
        report(
            "secure_keychain_get",
            format!(
                "{:?}",
                read.map(|secret| secret.as_deref() == Some(b"4321"))
            ),
        );
        report(
            "secure_keychain_delete",
            format!("{:?}", ios::keychain_delete(service, "pin")),
        );
    }

    let status = ios::biometric_status(BiometricPolicy::Biometrics);
    report(
        "biometric_status",
        format!("{:?} {:?}", status.biometry, status.available),
    );
    let passcode = ios::biometric_status(BiometricPolicy::BiometricsOrPasscode);
    report("passcode_status", format!("{:?}", passcode.available));

    ios::haptic(Haptic::Success);
    ios::haptic(Haptic::Selection);
    ios::haptic(Haptic::Medium);
    report("haptics", "dispatched".into());

    report(
        "notification_permission",
        format!("{:?}", ios::notification_permission().await),
    );
    if wants("--notify") {
        report(
            "notification_request",
            format!(
                "{:?}",
                ios::request_notification_permission(NotificationOptions::default()).await
            ),
        );
    }
    let mut note = LocalNotification::new("probe.1", "TAIM probe", "Local notification test");
    note.delay = Some(Duration::from_secs(if wants("--late") { 8 } else { 3 }));
    note.payload = Some("route:probe".into());
    report(
        "notification_schedule",
        format!("{:?}", ios::schedule_notification(note).await),
    );
    report(
        "notification_invalid",
        format!(
            "{:?}",
            ios::schedule_notification(LocalNotification::new("", "x", "y")).await
        ),
    );
    report(
        "notification_badge",
        format!("{:?}", ios::set_badge(0).await),
    );
    report(
        "notification_cancel",
        format!("{:?}", ios::cancel_notifications(&["probe.0"])),
    );

    report(
        "location_permission",
        format!("{:?}", ios::location_permission().await),
    );
    if wants("--ask-location") {
        report(
            "location_request",
            format!("{:?}", ios::request_location_permission().await),
        );
    }
    let current = ios::current_location(LocationOptions {
        timeout: Duration::from_secs(15),
        maximum_age: Duration::ZERO,
        ..LocationOptions::default()
    })
    .await;
    report(
        "location_current",
        match current {
            Ok(fix) => format!(
                "Ok {:.4},{:.4} ±{:.0}m",
                fix.latitude, fix.longitude, fix.accuracy
            ),
            Err(error) => format!("Err({error:?})"),
        },
    );
    match ios::watch_location(LocationWatchOptions::default()).await {
        Ok(mut watch) => {
            use futures::StreamExt as _;
            let first = futures::future::select(
                watch.next(),
                Box::pin(futures_timer(Duration::from_secs(10))),
            )
            .await;
            let summary = match first {
                futures::future::Either::Left((Some(Ok(fix)), _)) => {
                    format!("Ok {:.4},{:.4}", fix.latitude, fix.longitude)
                }
                futures::future::Either::Left((other, _)) => format!("{other:?}"),
                futures::future::Either::Right(_) => "timeout".into(),
            };
            report("location_watch", summary);
        }
        Err(error) => report("location_watch", format!("Err({error:?})")),
    }

    report(
        "camera_permission",
        format!("{:?}", ios::camera_permission()),
    );
    if wants("--camera") {
        report(
            "camera_capture",
            format!("{:?}", ios::capture_media(CaptureKind::Photo).await),
        );
    }
    report(
        "media_invalid",
        format!(
            "{:?}",
            ios::pick_media(MediaPicker {
                limit: 0,
                ..MediaPicker::default()
            })
            .await
        ),
    );

    if wants("--share") {
        report(
            "share",
            format!("{:?}", taim::platform::share_text("Shared from TAIM")),
        );
    }
    if wants("--pick") {
        report(
            "pick_media",
            format!(
                "{:?}",
                ios::pick_media(MediaPicker {
                    kind: MediaKind::Any,
                    limit: 3,
                })
                .await
            ),
        );
    }
    report("done", "true".into());
}

/// A sleep that needs no runtime: TAIM tasks run on their own threads.
#[cfg(any(target_os = "ios", target_os = "macos"))]
async fn futures_timer(duration: std::time::Duration) {
    let (sender, receiver) = futures::channel::oneshot::channel::<()>();
    std::thread::spawn(move || {
        std::thread::sleep(duration);
        let _ = sender.send(());
    });
    let _ = receiver.await;
}

fn main() -> taim::Result {
    taim::launch("Apple services", |cx| {
        let lines = cx.signal(Vec::<String>::new());

        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            let report = move |name: &str, result: String| {
                println!("TAIM_APPLE_PROBE {name}={result}");
                lines.update(|lines| lines.push(format!("{name}: {result}")));
            };
            // Always listening, so a tap that launches the app is reported.
            taim::view::spawn(async move {
                use futures::StreamExt as _;
                let mut events = taim::apple::notification_events();
                while let Some(event) = events.next().await {
                    report("notification_event", format!("{event:?}"));
                }
            });
            taim::view::spawn(probe(report));
        }
        #[cfg(not(any(target_os = "ios", target_os = "macos")))]
        lines.set(vec!["This fixture runs on iOS and macOS only.".to_owned()]);

        container(
            column(views![
                text("TAIM Apple services").size(24),
                text(move || lines.get().join("\n")).size(13),
            ])
            .gap(12.0),
        )
        .padding(16.0)
        .fill()
    })
}
