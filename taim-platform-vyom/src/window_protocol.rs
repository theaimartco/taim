//! Window-role authorization and protected-content policy.

use crate::capabilities::{Capabilities, Capability};

/// Semantic compositor surface role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Ordinary xdg-shell window.
    Toplevel,
    /// Shell layer.
    Layer,
    /// Temporary privileged overlay.
    Overlay,
    /// Window-overview representation.
    Overview,
    /// Secure lock surface.
    Lock,
}

/// Whether a surface may appear in screenshots or overview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturePolicy {
    /// Pixels may be captured.
    Allowed,
    /// Captures must replace pixels with a safe representation.
    Redacted,
    /// Capture is prohibited.
    Forbidden,
}

/// Window-role authorization failure.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("the compositor does not support this window role")]
    /// Required standard protocol is unavailable.
    Unsupported,
    #[error("a privileged VYOM role was requested without compositor authorization")]
    /// Privileged role lacks authenticated compositor authority.
    Unauthorized,
}

/// Unforgeable outside this crate. Issued only when the compositor exposes
/// the VYOM secure-shell global in addition to the standard session-lock
/// protocol.
#[derive(Debug)]
pub struct SecureAuthorization {
    _private: (),
}

/// Issues secure authority only from an authenticated capability set.
pub fn secure_authorization(capabilities: &Capabilities) -> Option<SecureAuthorization> {
    (capabilities.supports(Capability::SessionLock)
        && capabilities.supports(Capability::VyomSecureShell))
    .then_some(SecureAuthorization { _private: () })
}

/// Validates a role and returns its mandatory capture policy.
pub fn authorize(
    role: Role,
    capabilities: &Capabilities,
    secure: Option<&SecureAuthorization>,
) -> Result<CapturePolicy, Error> {
    match role {
        Role::Toplevel => Ok(CapturePolicy::Allowed),
        Role::Layer | Role::Overlay => capabilities
            .supports(Capability::LayerShell)
            .then_some(CapturePolicy::Allowed)
            .ok_or(Error::Unsupported),
        Role::Overview => Ok(CapturePolicy::Redacted),
        Role::Lock => secure
            .is_some()
            .then_some(CapturePolicy::Forbidden)
            .ok_or(Error::Unauthorized),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::Global;

    #[test]
    fn standard_session_lock_without_vyom_authentication_is_not_enough() {
        let capabilities = Capabilities::new([Global {
            interface: "ext_session_lock_manager_v1".into(),
            version: 1,
        }]);
        assert!(secure_authorization(&capabilities).is_none());
        assert_eq!(
            authorize(Role::Lock, &capabilities, None),
            Err(Error::Unauthorized)
        );
    }
}
