//! Interactive media example and isolated native protocol fixture.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(activity: taim::android::AndroidApp) {
    use taim::android::{MediaCapture, MediaKind, MediaPicker, Request, ServiceError, Services};
    use taim::prelude::*;
    let result = taim::app("Taim media", |cx| {
        let status = cx.signal("Media ready".to_owned());
        let selected = cx.signal(None::<String>);
        let tasks = cx.tasks();
        let request = move |request: Request, label: &'static str| {
            let _ = status.set("Media running".into());
            let _ = tasks.spawn(async move {
                let result = async {
                    let client = taim::android::services().ok_or(ServiceError::Unavailable)?;
                    client.request(request).await
                }
                .await;
                let _ = status.set(match result {
                    Ok(value) => {
                        if let Some(items) = value["items"].as_array() {
                            let _ = selected.set(
                                items
                                    .first()
                                    .and_then(|v| v["uri"].as_str())
                                    .map(str::to_owned),
                            );
                            format!("{label}: {}", items.len())
                        } else {
                            label.into()
                        }
                    }
                    Err(error) => format!("Media error: {error:?}"),
                });
            });
        };
        let image = request.clone();
        let visual = request.clone();
        let video = request.clone();
        let one = request.clone();
        let photo = request.clone();
        let capture_video = request.clone();
        let preview = request.clone();
        let retain = request.clone();
        let release = request.clone();
        let list = request.clone();
        let inspector = cx.tasks();
        let busy = cx.tasks();
        column(views![
            text("Android media").size(24),
            text(move || status.get()),
            text(move || format!("Media URI: {}", selected.get().unwrap_or_default())),
            button("Pick images").on_press(move || image(
                Request::PickMedia(MediaPicker {
                    kind: MediaKind::Images,
                    maximum: 2
                }),
                "Media selected"
            )),
            button("Pick visual").on_press(move || visual(
                Request::PickMedia(MediaPicker {
                    kind: MediaKind::ImagesAndVideos,
                    maximum: 2
                }),
                "Media selected"
            )),
            button("Pick videos").on_press(move || video(
                Request::PickMedia(MediaPicker {
                    kind: MediaKind::Videos,
                    maximum: 2
                }),
                "Media selected"
            )),
            button("Pick one").on_press(move || one(
                Request::PickMedia(MediaPicker {
                    kind: MediaKind::Images,
                    maximum: 1
                }),
                "Media selected"
            )),
            button("Capture photo").on_press(move || photo(
                Request::CaptureMedia(MediaCapture::Photo),
                "Media captured"
            )),
            button("Capture video").on_press(move || capture_video(
                Request::CaptureMedia(MediaCapture::Video {
                    duration_seconds: 15,
                    high_quality: true
                }),
                "Media captured"
            )),
            button("Inspect media").on_press(move || {
                let _ = inspector.spawn(async move {
                    let result = async {
                        let uri = selected.get().ok_or(ServiceError::Invalid)?;
                        let client = taim::android::services().ok_or(ServiceError::Unavailable)?;
                        let info = client.request(Request::MediaInfo(uri.clone())).await?;
                        let bytes = Services::bytes(
                            &client
                                .request(Request::ReadDocument {
                                    uri,
                                    limit: 4 * 1024 * 1024,
                                })
                                .await?,
                        )?;
                        if bytes.is_empty() || info["size"].as_u64() != Some(bytes.len() as u64) {
                            return Err(ServiceError::Failed);
                        }
                        Ok::<_, ServiceError>(())
                    }
                    .await;
                    let _ = status.set(match result {
                        Ok(()) => "Media inspected".into(),
                        Err(e) => format!("Media error: {e:?}"),
                    });
                });
            }),
            button("Preview media").on_press(move || {
                if let Some(uri) = selected.get() {
                    preview(Request::PreviewMedia(uri), "Media preview dispatched");
                }
            }),
            button("Retain media").on_press(move || {
                if let Some(uri) = selected.get() {
                    retain(Request::RetainMedia(uri), "Media retained");
                }
            }),
            button("Release media").on_press(move || {
                if let Some(uri) = selected.get() {
                    release(Request::ReleaseMedia(uri), "Media released");
                }
            }),
            button("Delete capture").on_press(move || {
                if let Some(uri) = selected.get() {
                    request(Request::DeleteCapturedMedia(uri), "Media deleted");
                }
            }),
            button("List captures").on_press(move || list(Request::CapturedMedia, "Media owned")),
            button("Check busy").on_press(move || {
                let _ = busy.spawn(async move {
                    let result = async {
                        let client = taim::android::services().ok_or(ServiceError::Unavailable)?;
                        let (picked, captured) = futures::join!(
                            client.request(Request::PickMedia(MediaPicker {
                                kind: MediaKind::Images,
                                maximum: 2
                            })),
                            client.request(Request::CaptureMedia(MediaCapture::Photo))
                        );
                        picked?;
                        if captured != Err(ServiceError::Busy) {
                            return Err(ServiceError::Failed);
                        }
                        Ok::<_, ServiceError>(())
                    }
                    .await;
                    let _ = status.set(match result {
                        Ok(()) => "Media busy handled".into(),
                        Err(e) => format!("Media error: {e:?}"),
                    });
                });
            }),
        ])
        .padding(16.0)
        .gap(6.0)
    })
    .run_android(activity);
    if let Err(error) = result {
        eprintln!("Media probe failed: {error}");
    }
}
