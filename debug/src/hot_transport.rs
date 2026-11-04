//! Debug-only cargo-hot wire client. Decode only the JumpTable message used
//! by the existing tool, with explicit frame/string/map bounds. No generic
//! serializer, server listener, or release-build patch loader is included.
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::Once;
use std::time::Duration;

const MAX_FRAME: usize = 8 * 1024 * 1024;
const MAX_PATH: usize = 4096;
const MAX_ENTRIES: usize = 65_536;

pub(super) fn connect() {
    static START: Once = Once::new();
    START.call_once(|| {
        let reference = subsecond::aslr_reference();
        let _ = std::thread::Builder::new()
            .name("taim-hot".into())
            .spawn(move || {
                loop {
                    if let Err(error) = receive(reference) {
                        log::trace!("Taim hot-reload connection closed: {error}");
                    }
                    std::thread::sleep(Duration::from_secs(5));
                }
            });
    });
}

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid or oversized hot patch")
}

fn receive(reference: usize) -> io::Result<()> {
    let address = SocketAddr::from(([127, 0, 0, 1], 1100));
    let mut socket = TcpStream::connect_timeout(&address, Duration::from_secs(1))?;
    socket.set_read_timeout(Some(Duration::from_secs(30)))?;
    socket.set_write_timeout(Some(Duration::from_secs(5)))?;
    socket.write_all(&reference.to_be_bytes())?;
    socket.flush()?;
    loop {
        let patch = read_patch(&mut socket)?;
        // SAFETY: This is the explicitly enabled local development hot-patch
        // mechanism. The subsecond API loads the supplied native library; it
        // is intentionally never compiled into a release-build call path.
        #[allow(unsafe_code)]
        unsafe { subsecond::apply_patch(patch) }.map_err(io::Error::other)?;
    }
}

fn read_patch(input: &mut impl Read) -> io::Result<subsecond::JumpTable> {
    let mut header = [0; size_of::<usize>()];
    input.read_exact(&mut header)?;
    let length = usize::from_be_bytes(header);
    if length == 0 || length > MAX_FRAME {
        return Err(invalid());
    }
    let mut bytes = vec![0; length];
    input.read_exact(&mut bytes)?;
    decode(&bytes).ok_or_else(invalid)
}

struct Decoder<'a>(&'a [u8]);
impl<'a> Decoder<'a> {
    fn take(&mut self, length: usize) -> Option<&'a [u8]> {
        let (bytes, rest) = self.0.split_at_checked(length)?;
        self.0 = rest;
        Some(bytes)
    }

    // The cargo-hot standard wire format uses little-endian unsigned varints.
    // u128 and reserved tags are invalid for every field of this message.
    fn integer(&mut self) -> Option<u64> {
        match self.take(1)?[0] {
            value @ 0..=250 => Some(u64::from(value)),
            251 => Some(u64::from(u16::from_le_bytes(
                self.take(2)?.try_into().ok()?,
            ))),
            252 => Some(u64::from(u32::from_le_bytes(
                self.take(4)?.try_into().ok()?,
            ))),
            253 => Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?)),
            _ => None,
        }
    }
}

fn decode(bytes: &[u8]) -> Option<subsecond::JumpTable> {
    let mut input = Decoder(bytes);
    let length = usize::try_from(input.integer()?).ok()?;
    if length == 0 || length > MAX_PATH {
        return None;
    }
    let path = std::str::from_utf8(input.take(length)?).ok()?;
    if path.contains('\0') {
        return None;
    }
    let count = usize::try_from(input.integer()?).ok()?;
    if count > MAX_ENTRIES || count > input.0.len() / 2 {
        return None;
    }
    let mut map = std::collections::HashMap::default();
    for _ in 0..count {
        let old = input.integer()?;
        let new = input.integer()?;
        if map.insert(old, new).is_some() {
            return None;
        }
    }
    let patch = subsecond::JumpTable {
        lib: PathBuf::from(path),
        map,
        aslr_reference: input.integer()?,
        new_base_address: input.integer()?,
        ifunc_count: input.integer()?,
    };
    input.0.is_empty().then_some(patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_existing_tool_message_and_all_address_widths() {
        // Path, map length, one old/new address, then the three base fields.
        let mut wire = vec![6];
        wire.extend_from_slice(b"/p.so\0");
        assert!(decode(&wire).is_none()); // NUL cannot reach a native loader.
        let mut wire = vec![5];
        wire.extend_from_slice(b"/p.so");
        wire.extend_from_slice(&[1, 251, 0, 1, 252, 0, 0, 1, 0, 253]);
        wire.extend_from_slice(&u64::MAX.to_le_bytes());
        wire.extend_from_slice(&[0, 1]);
        let patch = decode(&wire).expect("valid cargo-hot message");
        assert_eq!(patch.lib, PathBuf::from("/p.so"));
        assert_eq!(patch.map.get(&256), Some(&65_536));
        assert_eq!(patch.aslr_reference, u64::MAX);
        assert_eq!(patch.new_base_address, 0);
        assert_eq!(patch.ifunc_count, 1);
        for length in 0..wire.len() {
            assert!(decode(&wire[..length]).is_none());
        }
        wire.push(0);
        assert!(decode(&wire).is_none());
    }
    #[test]
    fn rejects_duplicate_addresses_invalid_tags_and_oversized_counts() {
        for suffix in [
            vec![2, 1, 2, 1, 3, 0, 0, 0],
            vec![255],
            vec![252, 1, 0, 1, 0],
            vec![0, 254, 0, 0],
        ] {
            let mut wire = vec![1, b'/'];
            wire.extend(suffix);
            assert!(decode(&wire).is_none());
        }
        let mut path = vec![251, 1, 16];
        path.resize(4100, b'a');
        assert!(decode(&path).is_none());
    }
    #[test]
    fn frame_size_is_rejected_before_reading_or_allocating_the_payload() {
        for size in [0, MAX_FRAME + 1, usize::MAX] {
            assert_eq!(
                read_patch(&mut size.to_be_bytes().as_slice())
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }
        let mut truncated = 5usize.to_be_bytes().to_vec();
        truncated.push(1);
        assert_eq!(
            read_patch(&mut truncated.as_slice()).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn malformed_input_cannot_panic_or_escape_decoder_bounds() {
        let mut seed = 0x6a09_e667_f3bc_c909_u64;
        for length in 0..512 {
            let mut bytes = vec![0; length];
            for byte in &mut bytes {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                *byte = seed as u8;
            }
            let _ = decode(&bytes);
            for tag in 250..=255 {
                if let Some(first) = bytes.first_mut() {
                    *first = tag;
                }
                let _ = decode(&bytes);
            }
        }
    }
}
