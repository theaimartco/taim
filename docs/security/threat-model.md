# Threat Model

## Trust Boundaries
- **App ↔ Framework**: Applications cannot corrupt framework state.
- **Framework ↔ Compositor**: Secure Wayland protocol usage.
- **Framework ↔ GPU**: Validate all shaders and render commands to prevent GPU hangs.

## Protected Assets
- **Secure Text**: Password fields bypass standard caching and clipboard.
- **Screenshots**: Secured buffers to prevent unauthorized capture.
- **Clipboard**: Explicit user intent required for access.

## Attack Surface and Mitigations
- **Malicious input events**: Strict validation in the gesture arena.
- **Resource exhaustion**: Hard limits on texture allocations and layout depths.
