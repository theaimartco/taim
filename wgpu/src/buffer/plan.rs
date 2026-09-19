use std::ops::Range;

/// Validated byte capacity for a GPU buffer.
#[derive(Debug, Clone, Copy)]
pub(super) struct Capacity(u64);

impl Capacity {
    pub(super) fn minimum(device: &wgpu::Device) -> Self {
        let available = device.limits().max_buffer_size.max(1);
        Self(available.min(wgpu::COPY_BUFFER_ALIGNMENT))
    }

    pub(super) fn for_elements<T>(
        device: &wgpu::Device,
        count: usize,
        label: &'static str,
    ) -> Option<Self> {
        let bytes = byte_count::<T>(count)?;
        let geometric = bytes.checked_next_power_of_two().unwrap_or(bytes);
        let aligned = align_up(geometric.max(wgpu::COPY_BUFFER_ALIGNMENT))?;

        if aligned > device.limits().max_buffer_size {
            log::error!(
                "refusing GPU buffer allocation above device limit for {label}: requested={aligned}, limit={}",
                device.limits().max_buffer_size
            );
            None
        } else {
            Some(Self(aligned))
        }
    }

    #[inline]
    pub(super) const fn bytes(self) -> u64 {
        self.0
    }

    pub(super) fn element_range<T>(
        self,
        start: usize,
        end: usize,
        label: &'static str,
    ) -> Option<Range<u64>> {
        if start > end {
            log::error!("refusing reversed GPU element range for {label}: {start}..{end}");
            return None;
        }

        let width = u64::try_from(std::mem::size_of::<T>()).ok()?;
        let first = u64::try_from(start).ok()?.checked_mul(width)?;
        let last = u64::try_from(end).ok()?.checked_mul(width)?;

        if last > self.0 {
            log::error!(
                "refusing GPU element range beyond {label}: end={last}, capacity={}",
                self.0
            );
            None
        } else {
            Some(first..last)
        }
    }

    pub(super) fn checked_write(
        self,
        offset: usize,
        length: usize,
        label: &'static str,
    ) -> Option<Range<u64>> {
        let start = u64::try_from(offset).ok()?;
        let length = u64::try_from(length).ok()?;
        let end = start.checked_add(length)?;

        if start % wgpu::COPY_BUFFER_ALIGNMENT != 0 || end > self.0 {
            log::error!(
                "refusing out-of-range or unaligned GPU write to {label}: offset={start}, bytes={length}, capacity={}",
                self.0
            );
            None
        } else {
            Some(start..end)
        }
    }
}

#[inline]
fn byte_count<T>(count: usize) -> Option<u64> {
    let width = u64::try_from(std::mem::size_of::<T>()).ok()?;
    width.checked_mul(u64::try_from(count).ok()?)
}

#[inline]
fn align_up(bytes: u64) -> Option<u64> {
    let mask = wgpu::COPY_BUFFER_ALIGNMENT - 1;
    bytes.checked_add(mask).map(|value| value & !mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_counts_reject_overflow() {
        assert_eq!(byte_count::<u64>(usize::MAX), None);
    }

    #[test]
    fn alignment_never_rounds_below_one_copy_unit() {
        assert_eq!(align_up(1), Some(wgpu::COPY_BUFFER_ALIGNMENT));
        assert_eq!(
            align_up(wgpu::COPY_BUFFER_ALIGNMENT),
            Some(wgpu::COPY_BUFFER_ALIGNMENT)
        );
    }
}
