use aura_platform_vyom::capabilities::{Capabilities, Global};
use aura_platform_vyom::window_protocol::{
    CapturePolicy, Error, Role, authorize, secure_authorization,
};

#[test]
fn lock_role_requires_both_standard_and_authenticated_vyom_protocols() {
    let standard_only = Capabilities::new([global("ext_session_lock_manager_v1")]);
    assert_eq!(
        authorize(Role::Lock, &standard_only, None),
        Err(Error::Unauthorized)
    );

    let authenticated = Capabilities::new([
        global("ext_session_lock_manager_v1"),
        global("vyom_secure_shell_v1"),
    ]);
    let authority = secure_authorization(&authenticated).unwrap();
    assert_eq!(
        authorize(Role::Lock, &authenticated, Some(&authority)),
        Ok(CapturePolicy::Forbidden)
    );
}

#[test]
fn overview_is_always_redacted() {
    assert_eq!(
        authorize(Role::Overview, &Capabilities::default(), None),
        Ok(CapturePolicy::Redacted)
    );
}

fn global(interface: &str) -> Global {
    Global {
        interface: interface.into(),
        version: 1,
    }
}
