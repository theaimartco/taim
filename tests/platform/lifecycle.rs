use aura_platform_vyom::{SurfaceId, SurfaceLifecycle, SurfaceState};

#[test]
fn repeated_configure_suspend_resume_destroy_is_terminal_safe() {
    for cycle in 1..=1_000 {
        let id = SurfaceId::new(cycle).unwrap();
        let mut surface = SurfaceLifecycle::new(id);
        assert!(surface.configure(cycle as u32, 1280, 720));
        assert!(surface.may_attach_buffer());
        surface.suspend();
        assert!(!surface.may_attach_buffer());
        surface.resume();
        assert_eq!(surface.state(), SurfaceState::AwaitingConfigure);
        assert!(surface.configure(cycle as u32 + 1, 720, 1280));
        surface.close();
        assert!(!surface.configure(cycle as u32 + 2, 1, 1));
    }
}

#[test]
fn fractional_scale_is_finite_and_rejects_zero() {
    let mut surface = SurfaceLifecycle::new(SurfaceId::new(1).unwrap());
    assert!(surface.set_scale_120(150));
    assert_eq!(surface.scale_factor(), 1.25);
    assert!(!surface.set_scale_120(0));
}
