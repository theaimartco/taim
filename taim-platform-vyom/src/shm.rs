//! Shared-memory buffers, and the attach that was missing.
//!
//! `Toplevel::commit_with_damage` submitted damage and committed — but never
//! attached a buffer, and a commit with no buffer presents nothing. That is
//! why nothing has ever appeared on a screen through this backend.
//!
//! This module supplies the buffer. It backs `wl_shm` with an ordinary file
//! whose contents the compositor maps, and writes frames through the file
//! descriptor rather than a memory mapping.
//!
//! # Why `write_all` and not `mmap`
//!
//! Mapping the file would be faster — one memcpy instead of a syscall per
//! frame — but `mmap` needs `unsafe`, and this workspace denies it
//! (`unsafe_code = "deny"`). A shm buffer is just file contents, so writing
//! through the descriptor is correct, and correctness on a path that has never
//! run matters more than a memcpy. Moving to `mmap` behind an audited
//! `allow(unsafe_code)` is a later, measurable change.
//!
//! # What is verified here, and what is not
//!
//! The size arithmetic and the pool's reuse rules are tested below and run
//! everywhere. **Nothing in this file has been executed against a compositor**
//! — this machine has no Wayland session. The protocol calls compile; whether
//! they present a frame is unknown, and no test here can tell you otherwise.

use std::fs::File;
use std::io::{Seek, SeekFrom, Write};

/// The pixel format written into every buffer.
///
/// `Argb8888` is the one format `wl_shm` is required to support, so it needs
/// no negotiation. Bytes are little-endian BGRA, which is what the software
/// renderer already produces.
pub const FORMAT: wayland_client::protocol::wl_shm::Format =
    wayland_client::protocol::wl_shm::Format::Argb8888;

/// Bytes per pixel in [`FORMAT`].
pub const BYTES_PER_PIXEL: u32 = 4;

/// A shared-memory allocation failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The backing file could not be created or resized.
    #[error("could not create the shared-memory backing file: {0}")]
    Backing(#[from] std::io::Error),
    /// The requested surface is too large to describe.
    #[error("a {width}x{height} surface does not fit in a shared-memory pool")]
    TooLarge {
        /// Requested width.
        width: u32,
        /// Requested height.
        height: u32,
    },
}

/// Returns the stride, in bytes, of a row of `width` pixels.
#[must_use]
pub fn stride(width: u32) -> u32 {
    width.saturating_mul(BYTES_PER_PIXEL)
}

/// Returns the byte size of one `width` by `height` buffer, if it fits.
pub fn buffer_bytes(width: u32, height: u32) -> Result<u64, Error> {
    let stride = u64::from(stride(width));
    let bytes = stride.checked_mul(u64::from(height));

    match bytes {
        // `wl_shm` sizes are `i32` on the wire, so anything past that cannot
        // be described to the compositor at all.
        Some(bytes) if bytes > 0 && bytes <= i32::MAX as u64 => Ok(bytes),
        _ => Err(Error::TooLarge { width, height }),
    }
}

/// The backing file for a surface's buffers.
///
/// Holds `depth` frames end to end, so one file serves the whole pool and a
/// buffer is identified by its offset.
#[derive(Debug)]
pub struct Backing {
    file: File,
    width: u32,
    height: u32,
    depth: usize,
    frame_bytes: u64,
}

impl Backing {
    /// Creates backing for `depth` frames of `width` by `height`.
    ///
    /// The file is unlinked immediately: the compositor receives it as a file
    /// descriptor, so it needs no name, and an unlinked file cannot be opened
    /// by anything else on the system. A window's pixels are not public.
    pub fn new(width: u32, height: u32, depth: usize) -> Result<Self, Error> {
        let frame_bytes = buffer_bytes(width, height)?;
        let depth = depth.max(1);

        let total = frame_bytes
            .checked_mul(depth as u64)
            .filter(|total| *total <= i32::MAX as u64)
            .ok_or(Error::TooLarge { width, height })?;

        let path = std::env::temp_dir().join(format!(
            "taim-shm-{}-{:x}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        ));

        let file = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)?;

        // Unlink now; the descriptor keeps it alive.
        let _ = std::fs::remove_file(&path);

        file.set_len(total)?;

        Ok(Self {
            file,
            width,
            height,
            depth,
            frame_bytes,
        })
    }

    /// The surface size these frames are sized for.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The total size of the backing file, in bytes.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.frame_bytes * self.depth as u64
    }

    /// The byte size of one frame.
    #[must_use]
    pub const fn frame_bytes(&self) -> u64 {
        self.frame_bytes
    }

    /// The byte offset of frame `index` within the file.
    ///
    /// `None` when the index is outside the pool, which is a caller error
    /// rather than something to clamp: writing frame 4 of a 3-frame pool at a
    /// clamped offset would silently corrupt frame 2.
    #[must_use]
    pub fn offset_of(&self, index: usize) -> Option<u64> {
        (index < self.depth).then(|| self.frame_bytes * index as u64)
    }

    /// Writes one frame's pixels at `index`.
    ///
    /// `pixels` must be exactly [`Backing::frame_bytes`] long. A short write
    /// would leave the tail of the previous frame visible, which reads as
    /// tearing and is very hard to attribute.
    pub fn write_frame(&mut self, index: usize, pixels: &[u8]) -> Result<(), Error> {
        let offset = self.offset_of(index).ok_or(Error::TooLarge {
            width: self.width,
            height: self.height,
        })?;

        if pixels.len() as u64 != self.frame_bytes {
            return Err(Error::Backing(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "frame is {} bytes, the buffer holds {}",
                    pixels.len(),
                    self.frame_bytes
                ),
            )));
        }

        let _ = self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(pixels)?;

        Ok(())
    }

    /// The descriptor to hand `wl_shm.create_pool`.
    #[must_use]
    pub const fn file(&self) -> &File {
        &self.file
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stride_and_size_are_consistent() {
        assert_eq!(stride(100), 400);
        assert_eq!(buffer_bytes(100, 50).unwrap(), 20_000);
    }

    #[test]
    fn a_surface_too_large_for_the_wire_is_refused() {
        // `wl_shm` sizes are `i32`. A pool larger than that cannot be
        // described to the compositor, and computing it as a wrapped `u32`
        // would ask for a pool far smaller than the frames written into it.
        assert!(buffer_bytes(u32::MAX, u32::MAX).is_err());
        assert!(buffer_bytes(100_000, 100_000).is_err());
        assert!(buffer_bytes(0, 0).is_err());
    }

    #[test]
    fn frames_do_not_overlap() {
        let backing = Backing::new(64, 64, 3).expect("temp dir is writable");
        let frame = backing.frame_bytes();

        assert_eq!(backing.offset_of(0), Some(0));
        assert_eq!(backing.offset_of(1), Some(frame));
        assert_eq!(backing.offset_of(2), Some(frame * 2));
        assert_eq!(backing.total_bytes(), frame * 3);
    }

    #[test]
    fn an_index_outside_the_pool_is_refused_rather_than_clamped() {
        // Clamping would write frame 3 on top of frame 2.
        let backing = Backing::new(16, 16, 3).expect("temp dir is writable");

        assert_eq!(backing.offset_of(3), None);
        assert_eq!(backing.offset_of(usize::MAX), None);
    }

    #[test]
    fn a_frame_of_the_wrong_size_is_refused() {
        // A short write leaves the tail of the previous frame visible, which
        // looks like tearing and is very hard to attribute.
        let mut backing = Backing::new(8, 8, 2).expect("temp dir is writable");
        let correct = backing.frame_bytes() as usize;

        assert!(backing.write_frame(0, &vec![0u8; correct]).is_ok());
        assert!(backing.write_frame(0, &vec![0u8; correct - 4]).is_err());
        assert!(backing.write_frame(0, &vec![0u8; correct + 4]).is_err());
    }

    #[test]
    fn frames_written_to_different_slots_do_not_disturb_each_other() {
        use std::io::Read;

        let mut backing = Backing::new(4, 4, 3).expect("temp dir is writable");
        let size = backing.frame_bytes() as usize;

        backing.write_frame(0, &vec![0xAA; size]).unwrap();
        backing.write_frame(1, &vec![0xBB; size]).unwrap();
        backing.write_frame(2, &vec![0xCC; size]).unwrap();

        let mut contents = Vec::new();
        let mut file = backing.file();
        let _ = file.seek(SeekFrom::Start(0)).unwrap();
        let _ = file.read_to_end(&mut contents).unwrap();

        assert!(contents[..size].iter().all(|byte| *byte == 0xAA));
        assert!(contents[size..size * 2].iter().all(|byte| *byte == 0xBB));
        assert!(contents[size * 2..].iter().all(|byte| *byte == 0xCC));
    }

    #[test]
    fn the_backing_file_is_not_reachable_by_name() {
        // A window's pixels are not public. The file is unlinked as soon as
        // it is created; only the descriptor keeps it alive.
        let backing = Backing::new(8, 8, 1).expect("temp dir is writable");
        let entries = std::fs::read_dir(std::env::temp_dir()).expect("temp dir is readable");

        let leaked = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("taim-shm-"))
            .count();

        assert_eq!(leaked, 0, "a backing file was left on disk");
        assert!(backing.total_bytes() > 0);
    }
}
