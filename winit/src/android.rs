//! Android process setup: logging to logcat and reporting panics there.

use std::sync::Once;

/// Sends `log` records to logcat under the `taim` tag, and panics too,
/// which otherwise vanish: an Android process has no terminal, so a panic
/// on the `android_main` thread would just look like the app closing.
///
/// Leaves an application's own logger in place if it installed one first.
pub(crate) fn init_logging() {
    static ONCE: Once = Once::new();

    ONCE.call_once(|| {
        android_logger::init_once(
            android_logger::Config::default()
                .with_tag("taim")
                .with_max_level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                }),
        );

        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            log::error!("panic: {info}");
            previous(info);
        }));
    });
}
