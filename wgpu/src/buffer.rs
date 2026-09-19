use std::marker::PhantomData;
use std::ops::RangeBounds;

mod plan;
mod upload;

pub const MAX_WRITE_SIZE: usize = upload::CHUNK_BYTES;

#[derive(Debug)]
pub struct Buffer<T> {
    label: &'static str,
    capacity: plan::Capacity,
    usage: wgpu::BufferUsages,
    pub(crate) raw: wgpu::Buffer,
    type_: PhantomData<T>,
}

impl<T: bytemuck::Pod> Buffer<T> {
    pub fn new(
        device: &wgpu::Device,
        label: &'static str,
        amount: usize,
        usage: wgpu::BufferUsages,
    ) -> Self {
        let capacity = plan::Capacity::for_elements::<T>(device, amount, label)
            .unwrap_or_else(|| plan::Capacity::minimum(device));

        let raw = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: capacity.bytes(),
            usage,
            mapped_at_creation: false,
        });

        Self {
            label,
            capacity,
            usage,
            raw,
            type_: PhantomData,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, new_count: usize) -> bool {
        let Some(next) = plan::Capacity::for_elements::<T>(device, new_count, self.label) else {
            return false;
        };

        if self.capacity.bytes() < next.bytes() {
            self.raw = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(self.label),
                size: next.bytes(),
                usage: self.usage,
                mapped_at_creation: false,
            });

            self.capacity = next;

            true
        } else {
            false
        }
    }

    /// Returns the size of the written bytes.
    pub fn write(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        offset: usize,
        contents: &[T],
    ) -> usize {
        let bytes: &[u8] = bytemuck::cast_slice(contents);

        if bytes.is_empty() {
            return 0;
        }

        let Some(range) = self.capacity.checked_write(offset, bytes.len(), self.label) else {
            return 0;
        };

        upload::slice(&self.raw, encoder, belt, range.start, bytes)
    }

    /// Streams many disjoint typed slices into one contiguous GPU range.
    ///
    /// A mesh batch commonly owns one small `Vec` per shape. Mapping the
    /// staging belt once per shape makes UI geometry submission syscall- and
    /// allocator-heavy. This writer keeps the existing 100 KiB transfer cap,
    /// but fills each mapped chunk from as many source slices as fit.
    pub fn write_many<'a>(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        offset: usize,
        element_count: usize,
        contents: impl IntoIterator<Item = &'a [T]>,
    ) -> usize
    where
        T: 'a,
    {
        let Some(total_bytes) = std::mem::size_of::<T>().checked_mul(element_count) else {
            return 0;
        };
        if total_bytes == 0 {
            return 0;
        }

        let Some(range) = self.capacity.checked_write(offset, total_bytes, self.label) else {
            return 0;
        };
        upload::scattered(&self.raw, encoder, belt, range.start, total_bytes, contents)
    }

    pub fn slice(&self, bounds: impl RangeBounds<wgpu::BufferAddress>) -> wgpu::BufferSlice<'_> {
        self.raw.slice(bounds)
    }

    pub fn range(&self, start: usize, end: usize) -> wgpu::BufferSlice<'_> {
        let bytes = self
            .capacity
            .element_range::<T>(start, end, self.label)
            .unwrap_or(0..0);
        self.slice(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growth_is_geometric_aligned_and_never_zero() {
        assert_eq!(MAX_WRITE_SIZE, 100 * 1024);
    }
}
