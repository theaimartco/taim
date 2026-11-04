//! Observable permission state. Android rationale alone never proves permanent denial.
use serde::Deserialize;
use serde_json::Value;
/// Current public-API permission observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionState {
    /// Currently granted.
    Granted,
    /// Declared runtime permission, no request history.
    NotRequested,
    /// Denied/revoked or a prior request did not produce a denial result.
    Denied,
    /// The latest runtime dialog returned without a decision.
    Cancelled,
    /// Completed denial plus no rationale; direct the user to settings.
    SettingsRequired,
    /// Not declared in this application's manifest.
    NotDeclared,
    /// Unknown on this Android release.
    Unavailable,
    /// Non-runtime permission that cannot be granted by a runtime dialog.
    Restricted,
}
/// Permission status, refreshed from the OS rather than a cached grant bit.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionStatus {
    /// Android permission name.
    pub name: String,
    /// Current OS grant.
    pub granted: bool,
    /// App manifest contains the permission.
    pub declared: bool,
    /// Permission exists on this OS.
    pub available: bool,
    /// Uses runtime permission dialogs.
    pub runtime: bool,
    /// This installation has dispatched a request before.
    pub requested_before: bool,
    /// Last callback omitted this permission result.
    pub last_request_cancelled: bool,
    /// Native rationale observation.
    pub should_show_rationale: bool,
    /// A runtime request remains appropriate; the OS still decides whether to show UI.
    pub can_ask_again: bool,
    /// Last completed denial had no rationale; not proof of an irrevocable/admin grant.
    pub settings_required: bool,
    /// Observed category.
    pub state: PermissionState,
}
/// OS-managed special access, separate from runtime permission dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialAccess {
    /// Draw over other applications.
    Overlay,
    /// Modify system settings.
    WriteSettings,
    /// Install packages from this source.
    InstallPackages,
    /// Broad file management, API30+.
    AllFiles,
    /// Exact alarm exemption/settings, API31+.
    ExactAlarms,
    /// Battery optimization exemption settings.
    BatteryOptimization,
    /// Full-screen intent access, API34+.
    FullScreenIntent,
}
impl SpecialAccess {
    #[cfg(target_os = "android")]
    pub(super) fn wire(self) -> &'static str {
        match self {
            Self::Overlay => "overlay",
            Self::WriteSettings => "writeSettings",
            Self::InstallPackages => "installPackages",
            Self::AllFiles => "allFiles",
            Self::ExactAlarms => "exactAlarms",
            Self::BatteryOptimization => "batteryOptimization",
            Self::FullScreenIntent => "fullScreenIntent",
        }
    }
}
pub(super) fn decode(value: Value) -> Option<Vec<PermissionStatus>> {
    let statuses: Vec<PermissionStatus> = serde_json::from_value(value).ok()?;
    if statuses.len() > 128
        || statuses.iter().any(|s| {
            !s.name.starts_with("android.permission.")
                || s.name.len() > 256
                || s.granted != (s.state == PermissionState::Granted)
                || (s.granted && (s.can_ask_again || s.settings_required))
        })
    {
        return None;
    }
    Some(statuses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn initial() -> Value {
        json!([{"name":"android.permission.CAMERA","state":"notRequested","granted":false,"declared":true,"available":true,"runtime":true,"requestedBefore":false,"lastRequestCancelled":false,"shouldShowRationale":false,"canAskAgain":true,"settingsRequired":false}])
    }
    #[test]
    fn first_request_without_rationale_remains_requestable() {
        let status = decode(initial()).unwrap().remove(0);
        assert_eq!(status.state, PermissionState::NotRequested);
        assert!(status.can_ask_again);
        assert!(!status.should_show_rationale);
    }
    #[test]
    fn permission_wire_rejects_contradictory_grants_and_unbounded_names() {
        let mut value = initial();
        value[0]["granted"] = json!(true);
        assert!(decode(value).is_none());
        let mut value = initial();
        value[0]["name"] = json!("android.permission.".to_owned() + &"x".repeat(257));
        assert!(decode(value).is_none());
        assert!(decode(json!([{"state":"invented"}])).is_none());
    }
    #[test]
    fn cancelled_callback_is_not_a_permanent_denial() {
        let mut value = initial();
        value[0]["state"] = json!("cancelled");
        value[0]["requestedBefore"] = json!(true);
        value[0]["lastRequestCancelled"] = json!(true);
        let state = decode(value).unwrap().remove(0);
        assert_eq!(state.state, PermissionState::Cancelled);
        assert!(state.can_ask_again);
        assert!(!state.settings_required);
    }
}
