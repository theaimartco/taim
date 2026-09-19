use super::Allocation;

pub(super) const RGBA_BYTES: usize = 4;
const BELT_ALIGNMENT: u64 = 32;

pub fn valid_rgba(width: u32, height: u32, pixels: &[u8]) -> bool {
    width > 0
        && height > 0
        && u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|value| value.checked_mul(RGBA_BYTES as u64))
            .and_then(|value| usize::try_from(value).ok())
            .is_some_and(|required| pixels.len() >= required)
}

#[allow(clippy::too_many_arguments)]
pub fn stage_rgba(
    texture: &wgpu::Texture,
    encoder: &mut wgpu::CommandEncoder,
    belt: &mut wgpu::util::StagingBelt,
    pixels: &[u8],
    image_width: u32,
    source_offset: usize,
    allocation: &Allocation,
) -> bool {
    let (x, y) = allocation.position();
    let size = allocation.size();
    let padding = allocation.padding();
    let width = size.width as usize;
    let height = size.height as usize;
    let pad_x = padding.width as usize;
    let pad_y = padding.height as usize;
    let row_pixels = width + 2 * pad_x;
    let copy_rows = height + 2 * pad_y;
    let bytes_per_row = match (row_pixels * RGBA_BYTES)
        .checked_next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize)
    {
        Some(bytes) => bytes,
        None => return false,
    };
    let Some(total_bytes) = bytes_per_row.checked_mul(copy_rows) else {
        return false;
    };
    let Some(buffer_size) = wgpu::BufferSize::new(total_bytes as u64) else {
        return false;
    };
    let Some(alignment) = wgpu::BufferSize::new(BELT_ALIGNMENT) else {
        return false;
    };
    let source_stride = image_width as usize * RGBA_BYTES;
    let content_bytes = width * RGBA_BYTES;
    let last_source = source_offset
        .checked_add((height - 1) * source_stride)
        .and_then(|value| value.checked_add(content_bytes));
    if last_source.is_none_or(|end| end > pixels.len()) {
        return false;
    }

    let slice = belt.allocate(buffer_size, alignment);
    let mut mapped = slice.get_mapped_range_mut();

    for row in 0..height {
        let source = source_offset + row * source_stride;
        let target = (row + pad_y) * bytes_per_row;
        let content_start = target + pad_x * RGBA_BYTES;
        mapped
            .slice(content_start..content_start + content_bytes)
            .copy_from_slice(&pixels[source..source + content_bytes]);

        let first_pixel = &pixels[source..source + RGBA_BYTES];
        let last_pixel = &pixels[source + content_bytes - RGBA_BYTES..source + content_bytes];
        for padding_pixel in 0..pad_x {
            let left = target + padding_pixel * RGBA_BYTES;
            let right = target + (pad_x + width + padding_pixel) * RGBA_BYTES;
            mapped
                .slice(left..left + RGBA_BYTES)
                .copy_from_slice(first_pixel);
            mapped
                .slice(right..right + RGBA_BYTES)
                .copy_from_slice(last_pixel);
        }
    }

    let top_source = source_offset;
    let bottom_source = source_offset + (height - 1) * source_stride;
    for padding_row in 0..pad_y {
        let top = padding_row * bytes_per_row;
        let bottom = (pad_y + height + padding_row) * bytes_per_row;
        let top_content = top + pad_x * RGBA_BYTES;
        let bottom_content = bottom + pad_x * RGBA_BYTES;
        mapped
            .slice(top_content..top_content + content_bytes)
            .copy_from_slice(&pixels[top_source..top_source + content_bytes]);
        mapped
            .slice(bottom_content..bottom_content + content_bytes)
            .copy_from_slice(&pixels[bottom_source..bottom_source + content_bytes]);

        let top_first = &pixels[top_source..top_source + RGBA_BYTES];
        let top_last = &pixels[top_source + content_bytes - RGBA_BYTES..top_source + content_bytes];
        let bottom_first = &pixels[bottom_source..bottom_source + RGBA_BYTES];
        let bottom_last =
            &pixels[bottom_source + content_bytes - RGBA_BYTES..bottom_source + content_bytes];

        for padding_pixel in 0..pad_x {
            let left = padding_pixel * RGBA_BYTES;
            let right = (pad_x + width + padding_pixel) * RGBA_BYTES;
            mapped
                .slice(top + left..top + left + RGBA_BYTES)
                .copy_from_slice(top_first);
            mapped
                .slice(top + right..top + right + RGBA_BYTES)
                .copy_from_slice(top_last);
            mapped
                .slice(bottom + left..bottom + left + RGBA_BYTES)
                .copy_from_slice(bottom_first);
            mapped
                .slice(bottom + right..bottom + right + RGBA_BYTES)
                .copy_from_slice(bottom_last);
        }
    }
    drop(mapped);

    encoder.copy_buffer_to_texture(
        wgpu::TexelCopyBufferInfo {
            buffer: slice.buffer(),
            layout: wgpu::TexelCopyBufferLayout {
                offset: slice.offset(),
                bytes_per_row: Some(bytes_per_row as u32),
                rows_per_image: Some(copy_rows as u32),
            },
        },
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: x - padding.width,
                y: y - padding.height,
                z: allocation.layer() as u32,
            },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::Extent3d {
            width: size.width + 2 * padding.width,
            height: size.height + 2 * padding.height,
            depth_or_array_layers: 1,
        },
    );
    true
}
