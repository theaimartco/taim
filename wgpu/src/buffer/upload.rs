use std::num::NonZeroU64;

pub(super) const CHUNK_BYTES: usize = 100 * 1024;

pub(super) fn slice(
    target: &wgpu::Buffer,
    encoder: &mut wgpu::CommandEncoder,
    belt: &mut wgpu::util::StagingBelt,
    start: u64,
    bytes: &[u8],
) -> usize {
    for (index, chunk) in bytes.chunks(CHUNK_BYTES).enumerate() {
        let offset = start + (index * CHUNK_BYTES) as u64;
        let length = NonZeroU64::new(chunk.len() as u64).expect("chunks are never empty");
        belt.write_buffer(encoder, target, offset, length)
            .copy_from_slice(chunk);
    }

    bytes.len()
}

/// Copies a stream of disjoint slices into bounded staging mappings.
pub(super) fn scattered<'a, T: bytemuck::Pod + 'a>(
    target: &wgpu::Buffer,
    encoder: &mut wgpu::CommandEncoder,
    belt: &mut wgpu::util::StagingBelt,
    start: u64,
    byte_count: usize,
    sources: impl IntoIterator<Item = &'a [T]>,
) -> usize {
    let mut sources = sources.into_iter().map(bytemuck::cast_slice::<T, u8>);
    let mut source = sources.next().unwrap_or_default();
    let mut source_at = 0;
    let mut written = 0;

    while written < byte_count {
        let chunk_len = (byte_count - written).min(CHUNK_BYTES);
        let mut chunk = belt.write_buffer(
            encoder,
            target,
            start + written as u64,
            NonZeroU64::new(chunk_len as u64).expect("remaining byte count is non-zero"),
        );
        let mut chunk_at = 0;

        while chunk_at < chunk_len {
            while source_at == source.len() {
                let Some(next) = sources.next() else {
                    log::error!(
                        "scattered GPU upload declared {byte_count} bytes but supplied only {}",
                        written + chunk_at
                    );
                    return written + chunk_at;
                };
                source = next;
                source_at = 0;
            }

            let copied = (source.len() - source_at).min(chunk_len - chunk_at);
            chunk
                .slice(chunk_at..chunk_at + copied)
                .copy_from_slice(&source[source_at..source_at + copied]);
            source_at += copied;
            chunk_at += copied;
        }

        written += chunk_len;
    }

    debug_assert!(source[source_at..].is_empty());
    debug_assert!(sources.all(<[u8]>::is_empty));
    written
}
