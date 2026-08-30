#![allow(missing_docs)]

use super::{ComponentSpec, Density, InputMethods, StateSupport};
use crate::tokens::Radius;

use taim_core::Size;
use taim_core::semantics::{Actions, Role};

/// How many lines of text a tile reserves room for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListLines {
    #[default]
    One,
    Two,
    Three,
}

impl ListLines {
    /// The tile height, in logical pixels, this line count needs. Each
    /// step adds one line's worth of height rather than a magic number.
    pub const fn height(self) -> f32 {
        match self {
            Self::One => 48.0,
            Self::Two => 64.0,
            Self::Three => 80.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ListTile {
    pub lines: ListLines,
    pub interactive: bool,
}

impl ComponentSpec for ListTile {
    fn role(&self) -> Role {
        if self.interactive {
            Role::ListItem
        } else {
            Role::StaticText
        }
    }

    fn actions(&self) -> Actions {
        if self.interactive {
            Actions::ACTIVATE | Actions::FOCUS
        } else {
            Actions::empty()
        }
    }

    fn min_size(&self, density: Density) -> Size {
        let width = match density {
            Density::Compact => 48.0,
            Density::Comfortable | Density::Spacious => 64.0,
        };

        Size::new(width, self.lines.height())
    }

    fn supported_states(&self) -> StateSupport {
        if self.interactive {
            StateSupport::INTERACTIVE | StateSupport::SELECTED
        } else {
            StateSupport::empty()
        }
    }

    fn input_methods(&self) -> InputMethods {
        if self.interactive {
            InputMethods::ALL
        } else {
            InputMethods::empty()
        }
    }

    fn radius(&self) -> Radius {
        // Tiles sit flush inside a list; rounding them would break the
        // continuous edge a scannable list depends on.
        Radius::None
    }
}
