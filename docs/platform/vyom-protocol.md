# VYOM native platform protocol

## Supported baseline

Taim connects directly to the compositor through Wayland. The production
baseline requires `wl_compositor` v4+, `xdg_wm_base` v1+, and `wl_seat`
v5+. Optional functionality is enabled only when its advertised protocol
and minimum version are present:

| Function | Protocol |
|---|---|
| presentation timing | `wp_presentation` |
| fractional scale | `wp_fractional_scale_manager_v1` + `wp_viewporter` |
| shell panels | `zwlr_layer_shell_v1` v4+ |
| text input | `zwp_text_input_manager_v3` |
| clipboard and drag/drop | `wl_data_device_manager` v3+ |
| lock lifecycle | `ext_session_lock_manager_v1` |

Globals may appear or disappear while the client runs. The backend updates
its immutable capability snapshot and emits `GlobalsChanged`; callers must
stop using a removed optional facility. Core-global removal is treated as
connection loss, never as permission to continue with stale proxies.

## VYOM privilege boundary

Standard session-lock support alone does not authorize an Taim process to
become the system lock screen. The compositor must additionally advertise
the authenticated `vyom_secure_shell_v1` global. Only then can the backend
issue an unforgeable `SecureAuthorization`. Lock surfaces are always marked
capture-forbidden.

The XML definition and compositor-side authentication policy for
`vyom_secure_shell_v1`, `vyom_power_manager_v1`, and
`vyom_accessibility_manager_v1` are owned by the VYOM compositor repository.
They are deliberately not fabricated here. Until those bindings are supplied,
the native backend fails closed for privileged roles and uses standard
Wayland protocols for ordinary windows.

## Lifecycle invariants

- No buffer attaches before the first `xdg_surface.configure` is acknowledged.
- A configure serial is acknowledged exactly once.
- Zero-sized untrusted configurations are sanitized to one logical pixel.
- Suspended surfaces submit no frames and resume through a fresh configure.
- Closed identities never resurrect.
- Device or seat removal cancels every live pointer contact exactly once.
- Clipboard and drag/drop reject secret content and bound resident bytes.
- A blocking Wayland dispatch is the idle path; there is no polling loop.
