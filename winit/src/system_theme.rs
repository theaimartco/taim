//! Desktop portal color preference, owned and cancelled by the native shell.
//! Bus setup never blocks window creation or the UI thread.

use crate::core::theme;
use futures::StreamExt as _;
use futures::future::{AbortHandle, Abortable};
use zbus::{OwnedVariant, proxy};

const NAMESPACE: &str = "org.freedesktop.appearance";
const KEY: &str = "color-scheme";

#[proxy(
    interface = "org.freedesktop.portal.Settings",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait Settings {
    fn read(&self, namespace: &str, key: &str) -> zbus::Result<OwnedVariant>;

    #[zbus(signal)]
    fn setting_changed(&self, namespace: &str, key: &str, value: OwnedVariant) -> zbus::Result<()>;
}

fn color_mode(value: OwnedVariant) -> Option<theme::Mode> {
    match u32::try_from(value).ok()? {
        0 => Some(theme::Mode::None),
        1 => Some(theme::Mode::Dark),
        2 => Some(theme::Mode::Light),
        _ => None,
    }
}

pub(crate) struct Watcher(AbortHandle);

impl Watcher {
    pub(crate) fn start(send: impl Fn(theme::Mode) -> bool + Send + 'static) -> Option<Self> {
        let (abort, registration) = AbortHandle::new_pair();
        match std::thread::Builder::new()
            .name("taim-theme".into())
            .spawn(move || {
                let _ = zbus::block_on(Abortable::new(follow(send), registration));
            }) {
            Ok(_) => Some(Self(abort)),
            Err(error) => {
                log::warn!("could not start desktop theme watcher: {error}");
                None
            }
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn follow(send: impl Fn(theme::Mode) -> bool) {
    if let Err(error) = follow_portal(send).await {
        // A portal is optional. Keep the window system's theme on failure.
        log::debug!("desktop color preference unavailable: {error}");
    }
}

async fn follow_portal(send: impl Fn(theme::Mode) -> bool) -> zbus::Result<()> {
    let connection = zbus::Connection::session().await?;
    let settings = SettingsProxy::new(&connection).await?;
    // Subscribe before reading so a concurrent desktop change cannot be lost.
    let mut changes = settings.receive_setting_changed().await?;
    if let Some(mode) = color_mode(settings.read(NAMESPACE, KEY).await?)
        && !send(mode)
    {
        return Ok(());
    }
    while let Some(change) = changes.next().await {
        let Ok(args) = change.args() else { continue };
        if args.namespace == NAMESPACE
            && args.key == KEY
            && let Some(mode) = color_mode(args.value.try_clone()?)
            && !send(mode)
        {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_preferences_use_the_documented_numeric_values() {
        assert_eq!(
            color_mode(OwnedVariant::from(0_u32)),
            Some(theme::Mode::None)
        );
        assert_eq!(
            color_mode(OwnedVariant::from(1_u32)),
            Some(theme::Mode::Dark)
        );
        assert_eq!(
            color_mode(OwnedVariant::from(2_u32)),
            Some(theme::Mode::Light)
        );
        assert_eq!(color_mode(OwnedVariant::from(3_u32)), None);
        assert_eq!(color_mode(OwnedVariant::from(-1_i32)), None);
    }
    struct Portal;

    #[zbus::interface(name = "org.freedesktop.portal.Settings")]
    impl Portal {
        async fn read(
            &self,
            namespace: &str,
            key: &str,
            #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
        ) -> zbus::fdo::Result<OwnedVariant> {
            assert_eq!((namespace, key), (NAMESPACE, KEY));
            // An unrelated setting must not restyle the application. A change
            // racing with Read must survive until after its initial result.
            Self::setting_changed(&emitter, NAMESPACE, "contrast", OwnedVariant::from(1_u32))
                .await?;
            Self::setting_changed(&emitter, NAMESPACE, KEY, OwnedVariant::from(2_u32)).await?;
            Ok(OwnedVariant::from(1_u32))
        }

        #[zbus(signal)]
        async fn setting_changed(
            emitter: &zbus::object_server::SignalEmitter<'_>,
            namespace: &str,
            key: &str,
            value: OwnedVariant,
        ) -> zbus::Result<()>;
    }

    #[test]
    #[ignore = "run on a private bus with dbus-run-session"]
    fn portal_initial_read_and_racing_changes_are_delivered_in_order() {
        zbus::block_on(async {
            let _service = zbus::connection::Builder::session()
                .name("org.freedesktop.portal.Desktop")
                .serve_at("/org/freedesktop/portal/desktop", Portal)
                .build()
                .await
                .expect("private portal service");
            let modes = std::sync::Mutex::new(Vec::new());
            let follow = follow_portal(|mode| {
                let mut modes = modes.lock().unwrap();
                modes.push(mode);
                modes.len() < 2
            });
            // A broken signal subscription must fail the test instead of hang.
            let timeout = async {
                let (sender, receiver) = futures::channel::oneshot::channel();
                let _timer = std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    let _ = sender.send(());
                });
                let _ = receiver.await;
            };
            futures::pin_mut!(follow, timeout);
            match futures::future::select(follow, timeout).await {
                futures::future::Either::Left((result, _)) => result.unwrap(),
                futures::future::Either::Right(_) => panic!("portal watcher timed out"),
            }
            assert_eq!(
                *modes.lock().unwrap(),
                [theme::Mode::Dark, theme::Mode::Light]
            );
        });
    }
}
