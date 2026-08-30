//! Optical-grid icon metadata.
#![allow(missing_docs)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icon {
    pub name: &'static str,
    pub grid: u8,
    pub mirror_rtl: bool,
}
impl Icon {
    pub const fn new(name: &'static str, grid: u8, mirror_rtl: bool) -> Self {
        Self {
            name,
            grid,
            mirror_rtl,
        }
    }
}
pub fn valid_manifest_entry(icon: Icon) -> bool {
    !icon.name.is_empty()
        && icon
            .name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && matches!(icon.grid, 16 | 20 | 24 | 32)
}
