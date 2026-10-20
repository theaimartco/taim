use crate::runtime::Task;

use std::borrow::Cow;
use std::fmt;

/// A specific boot strategy for a [`Program`](crate::Program).
pub struct Preset<State, Message> {
    name: Cow<'static, str>,
    boot: Box<dyn Fn() -> (State, Task<Message>)>,
}

impl<State, Message> Preset<State, Message> {
    /// Creates a new [`Preset`] with the given name and boot strategy.
    pub fn new(
        name: impl Into<Cow<'static, str>>,
        boot: impl Fn() -> (State, Task<Message>) + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            boot: Box::new(boot),
        }
    }

    /// Constructs a preset with a name suitable for desktop menus/logging.
    /// Existing `new` remains backward compatible.
    pub fn try_new(
        name: impl Into<Cow<'static, str>>,
        boot: impl Fn() -> (State, Task<Message>) + 'static,
    ) -> Result<Self, PresetNameError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(PresetNameError::Empty);
        }
        if name.len() > 128 {
            return Err(PresetNameError::TooLong);
        }
        if name.chars().any(char::is_control) {
            return Err(PresetNameError::ControlCharacter);
        }
        Ok(Self { name, boot: Box::new(boot) })
    }

    /// Returns the name of the [`Preset`].
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Boots the [`Preset`], returning the initial [`Program`](crate::Program) state and
    /// a [`Task`] for concurrent booting.
    pub fn boot(&self) -> (State, Task<Message>) {
        (self.boot)()
    }
}

impl<State, Message> fmt::Debug for Preset<State, Message> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Preset")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// Validation error from a named TAIM startup preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetNameError {
    /// The name contains no visible characters.
    Empty,
    /// The UTF-8 name exceeds 128 encoded bytes.
    TooLong,
    /// The name contains a control character, such as a newline.
    ControlCharacter,
}

#[cfg(test)]
mod taim_preset_tests {
    use super::*;

    #[test]
    fn validates_names_without_running_the_boot_closure() {
        assert!(Preset::<u8, ()>::try_new("  ", || (0, Task::none())).is_err());
        assert!(Preset::<u8, ()>::try_new("bad\nname", || (0, Task::none())).is_err());
        let preset = Preset::<u8, ()>::try_new("TAIM", || (5, Task::none())).unwrap();
        assert_eq!(preset.name(), "TAIM");
        assert_eq!(preset.boot().0, 5);
    }
}
