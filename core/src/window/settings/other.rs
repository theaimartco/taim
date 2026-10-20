//! TAIM native-window capabilities on targets without a platform adapter.
///
/// Failing closed on unsupported native effects is preferable to silently
/// advertising capabilities that the generic windowing backend cannot supply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlatformSpecific;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWindowEffect {
    TransparentTitlebar,
    BackgroundBlur,
    CustomWindowShadow,
}

impl PlatformSpecific {
    /// No OS-specific effects are guaranteed on the generic fallback target.
    pub const fn supports(self, _effect: NativeWindowEffect) -> bool {
        false
    }
}

#[cfg(test)]
mod taim_fallback_window_tests {
    use super::*;

    #[test]
    fn unsupported_native_effects_fail_closed() {
        let platform = PlatformSpecific;
        assert!(!platform.supports(NativeWindowEffect::TransparentTitlebar));
        assert!(!platform.supports(NativeWindowEffect::BackgroundBlur));
        assert!(!platform.supports(NativeWindowEffect::CustomWindowShadow));
    }
}
