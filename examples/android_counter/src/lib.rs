//! Minimal Android consumer of Taim's persistent-view application API.

/// Entry point called by android-activity on its native application thread.
// SAFETY: android-activity requires this uniquely named Rust-ABI symbol.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(activity: taim::android::AndroidApp) {
    use taim::prelude::*;

    struct AndroidLog;
    impl log::Log for AndroidLog {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Info
        }
        fn log(&self, record: &log::Record<'_>) {
            if self.enabled(record.metadata()) {
                // android-activity forwards stderr to Android's logcat.
                eprintln!("{} {}: {}", record.level(), record.target(), record.args());
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: AndroidLog = AndroidLog;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
    log::info!("Taim Android counter starting");

    let result = taim::app("Taim Android", |cx| {
        let count = cx
            .restorable("counter", 1, 0_i64)
            .expect("unique counter key");
        let message = cx
            .restorable("message", 1, String::new())
            .expect("unique message key");
        let password = cx.signal(String::new());
        let notes = Buffer::new(cx, "Multiline notes\nदूसरी पंक्ति");
        let panel = column(views![
            text("Taim on Android").size(28),
            // Keep the status readable over every coloured glass backdrop.
            container(
                text(move || format!("Count: {}", count.get()))
                    .size(24)
                    .color(taim::Color::from_packed_rgba8(0x172033, 1.0))
            )
            .background(taim::Color::from_packed_rgba8(0xF6F8FC, 1.0))
            .border(taim::Border::default().rounded(12.0))
            .padding(8.0),
            button("Increment").on_press(move || {
                count.update(|n| *n += 1);
                log::info!("counter={}", count.get());
            }),
            text("System fonts: नमस्ते · مرحبا · emoji 🌍"),
            text_field("Type here / यहाँ लिखें", message),
            text(move || format!("Message: {}", message.get())),
            text_field("Password (private input)", password)
                .secret()
                .autofill(taim::semantics::Autofill::Password),
            container(text_editor(&notes)).height(100.0),
            text("Background and reopen: the count should survive surface recreation."),
        ])
        .gap(16.0)
        .padding(24.0);
        layers(views![
            column(views![
                container(text("TAIM / GLASS").size(32))
                    .background(taim::Color::from_rgb(0.20, 0.30, 0.85))
                    .padding(32.0)
                    .fill_width(),
                container(text("Native Rust · Light in motion").size(20))
                    .background(taim::Color::from_rgb(0.05, 0.65, 0.65))
                    .padding(48.0)
                    .fill_width(),
                container(text("Persistent views"))
                    .background(taim::Color::from_rgb(0.65, 0.25, 0.60))
                    .padding(64.0)
                    .fill_width(),
            ])
            .fill_width()
            .fill_height(),
            container(container(panel).frosted_glass().fill_width())
                .padding(20.0)
                .fill(),
        ])
    })
    .run_android(activity);

    if let Err(error) = result {
        eprintln!("Taim Android failed: {error}");
    }
}
