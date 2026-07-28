//! Surface/material contracts.
#![allow(missing_docs)]
use crate::tokens::{Elevation, Radius};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Surface {
    pub elevation: Elevation,
    pub radius: Radius,
    pub interactive: bool,
}
impl Surface {
    pub const fn card() -> Self {
        Self {
            elevation: Elevation::Raised,
            radius: Radius::Medium,
            interactive: false,
        }
    }
    pub const fn interactive() -> Self {
        Self {
            elevation: Elevation::Raised,
            radius: Radius::Medium,
            interactive: true,
        }
    }
}
