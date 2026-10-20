//! TAIM software-window framebuffer limits.
//!
//! Capturing a giant screenshot allocates a raster buffer, clip mask and
//! additional output vector. Refuse excessive frame sizes before allocation.
pub mod compositor;
pub use compositor::{Compositor, Surface};

/// Maximum RGBA output bytes per software screenshot.
pub const MAX_SCREENSHOT_RGBA_BYTES: u64 = 128 * 1024 * 1024;

/// Return the number of pixels for a safely bounded software screenshot.
pub fn screenshot_pixel_budget(width: u32, height: u32) -> Option<usize> {
    let bytes = u64::from(width).checked_mul(u64::from(height))?.checked_mul(4)?;
    if bytes == 0 || bytes > MAX_SCREENSHOT_RGBA_BYTES {
        return None;
    }
    usize::try_from(bytes / 4).ok()
}

#[cfg(test)]
mod taim_software_capture_tests {
    use super::*;

    #[test]
    fn rejects_unbounded_screenshot_allocations() {
        assert_eq!(screenshot_pixel_budget(1, 1), Some(1));
        assert_eq!(screenshot_pixel_budget(0, 1080), None);
        assert_eq!(screenshot_pixel_budget(u32::MAX, u32::MAX), None);
        assert_eq!(screenshot_pixel_budget(1920, 1080), Some(2_073_600));
    }
}
