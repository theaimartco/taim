use aura_platform_vyom::{Capabilities, Capability, Global};

pub fn complete_standard_globals() -> Capabilities {
    Capabilities::new([
        global("wl_compositor", 6),
        global("xdg_wm_base", 6),
        global("wl_seat", 9),
        global("wl_shm", 1),
        global("wl_data_device_manager", 3),
        global("zwp_text_input_manager_v3", 1),
        global("wp_presentation", 1),
        global("wp_fractional_scale_manager_v1", 1),
        global("wp_viewporter", 1),
        global("zwlr_layer_shell_v1", 4),
        global("ext_session_lock_manager_v1", 1),
    ])
}

pub fn global(interface: &str, version: u32) -> Global {
    Global {
        interface: interface.to_owned(),
        version,
    }
}

#[test]
fn fake_compositor_has_every_required_core_global() {
    let globals = complete_standard_globals();
    assert!(globals.missing_core().next().is_none());
    assert!(globals.supports(Capability::PresentationFeedback));
    assert!(globals.supports(Capability::FractionalScale));
}
