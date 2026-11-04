//! Device acceptance probe for the Gradle extensions (FCM, Maps, Billing).
//! Copy over `rust/src/lib.rs` of a project generated with
//! `project.py DEST --package ... --fcm google-services.json --maps --billing --notifications --location precise`.
//! Results are logged as `TAIM_PROBE <step> <result>`.
#[unsafe(no_mangle)]
pub fn android_main(activity: taim::android::AndroidApp) {
    use taim::android::{
        PlatformEvent, ProductKind, PurchaseUpdate, PushEvent, ServiceError, Services, services,
    };
    use taim::prelude::*;

    fn log(step: &str, result: impl std::fmt::Display) {
        eprintln!("TAIM_PROBE {step} {result}");
    }

    let result = taim::app("Taim extensions", |cx| {
        let last = cx.signal("Ready".to_owned());
        let camera = cx.signal(0_u64);
        let tasks = cx.tasks();
        let report = move |step: &'static str, value: String| {
            log(step, &value);
            let _ = last.set(format!("{step}: {value}"));
        };
        let _ = tasks.spawn(async move {
            use futures::StreamExt as _;
            let Some(client) = services() else { return };
            let Ok(mut events) = client.events() else { return };
            while let Some(event) = events.next().await {
                match event {
                    PlatformEvent::Plugin { channel, event } => {
                        if let Some(push) = PushEvent::decode(&channel, &event) {
                            report("push-event", format!("{push:?}"));
                        } else if let Some(update) = PurchaseUpdate::decode(&channel, &event) {
                            report("billing-update", format!("{} purchases={}", update.result, update.purchases.len()));
                        }
                    }
                    PlatformEvent::NativeView { id, event, .. } => log("event-view", format!("{id} {event}")),
                    _ => {}
                }
            }
        });
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
                        report($name, match outcome { Ok(v) => format!("Ok {v}"), Err(e) => format!("Err({e:?})") });
                    });
                }
            }};
        }
        let notifications = step!("notification-permission", |c| Ok(format!("{:?}",
            c.request_permissions(vec!["android.permission.POST_NOTIFICATIONS".into(), "android.permission.ACCESS_FINE_LOCATION".into()]).await?
                .iter().map(|s| s.granted).collect::<Vec<_>>())));
        let token = step!("push-token", |c| Ok(c.push_token().await?));
        let topic = step!("push-subscribe", |c| { c.push_subscribe("taim-probe").await?; Ok("taim-probe".into()) });
        let pending = step!("push-pending", |c| Ok(format!("{:?}", c.push_pending().await?)));
        let billing = step!("billing", |c| {
            c.billing_connect().await?;
            let owned = c.billing_purchases(ProductKind::Inapp).await?;
            // "android.test.purchased" is no longer served; use a real product id from your Play Console.
            let products = c.billing_products(ProductKind::Inapp, &["taim_probe_item"]).await?;
            Ok(format!("connected owned={} products={:?}", owned.len(), products.iter().map(|p| (&p.id, p.price.as_ref().map(|p| &p.formatted))).collect::<Vec<_>>()))
        });
        let buy = step!("billing-purchase", |c| { c.billing_purchase("taim_probe_item", None, None).await?; Ok("sheet shown".into()) });
        let map = android_map_view(move || {
            let mut m = AndroidMap::new(28.6139, 77.2090, 12.0);
            m.camera_token = camera.get();
            m.my_location = true;
            m.markers = vec![MapMarker { id: "delhi".into(), latitude: 28.6139, longitude: 77.2090, title: Some("New Delhi".into()) }];
            m
        })
        .expect("valid map")
        .height(320.0);
        scroll(column(views![
            text("Taim extensions probe").size(24),
            text(move || last.get()).size(16),
            row(views![button("Permissions").on_press(notifications), button("Push token").on_press(token)]).gap(8.0),
            row(views![button("Subscribe").on_press(topic), button("Pending").on_press(pending)]).gap(8.0),
            row(views![button("Billing").on_press(billing), button("Buy").on_press(buy)]).gap(8.0),
            button("Recenter map").on_press(move || camera.update(|t| *t += 1)),
            map,
        ]).gap(12.0).padding(20.0))
    })
    .run_android(activity);
    if let Err(error) = result {
        eprintln!("Taim Android failed: {error}");
    }
}
