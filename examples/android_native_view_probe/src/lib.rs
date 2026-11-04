//! Native hierarchy and SurfaceTexture integration fixture.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(activity: taim::android::AndroidApp) {
    use serde_json::json;
    use taim::prelude::*;
    let result = taim::app("Native embedding", |cx| {
        let enabled = cx.signal(true);
        let red = cx.signal(false);
        let message = cx.signal(String::new());
        let open = cx.signal(false);
        let events = cx.signal(0_u64);
        let sensor_status = cx.signal("Sensor not requested");
        let _ = cx.tasks().spawn(async move {
            if let Some(client) = taim::android::services() {
                let result = client
                    .read_sensor(taim::android::SensorOptions {
                        sensor: taim::android::SensorKind::Accelerometer,
                        timeout_ms: 5_000,
                    })
                    .await;
                let _ = sensor_status.set(if result.is_ok() {
                    "Sensor sample ready"
                } else {
                    "Sensor sample unavailable"
                });
            }
        });
        let controls = android_view("fixture.controls", json!({"label":"Native button"}))
            .unwrap()
            .height(140.0)
            .enabled(enabled);
        let controls_id = controls.id();
        let _ = cx.tasks().spawn(async move {
            use futures::StreamExt as _;
            if let Some(client) = taim::android::services()
                && let Ok(mut stream) = client.events()
            {
                while let Some(event) = stream.next().await {
                    if let taim::android::PlatformEvent::NativeView {
                        id,
                        generation,
                        event,
                    } = event
                        && id == controls_id
                        && generation > 0
                        && event["type"] == "click"
                    {
                        events.update(|n| *n += 1);
                    }
                }
            }
        });
        column(views![
            text("Native embedding").size(24),
            text(move || format!("Native events: {}", events.get())),
            button("Show modal").on_press(move || {
                let _ = open.set(true);
            }),
            button("Toggle native input").on_press(move || enabled.update(|v| *v = !*v)),
            button("Change texture color").on_press(move || red.update(|v| *v = !*v)),
            text_field("Taim editor", message),
            controls,
            android_view(
                "taim.texture",
                move || json!({"color":if red.get() {-65536_i32} else {-16776961_i32}})
            )
            .unwrap()
            .height(120.0),
            text(move || sensor_status.get().to_owned()),
            dialog(
                open,
                button("Close modal").on_press(move || {
                    let _ = open.set(false);
                })
            ),
        ])
        .gap(8.0)
        .padding(16.0)
    })
    .run_android(activity);
    if let Err(error) = result {
        eprintln!("Native embedding probe failed: {error}");
    }
}
