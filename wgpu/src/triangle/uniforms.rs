use crate::core::Transformation;

/// Per-mesh transform padded to WebGPU's dynamic-uniform stride.
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Uniforms {
    transform: [f32; 16],
    opacity: f32,
    _padding: [f32; 47],
}

impl Uniforms {
    pub fn new(transform: Transformation, opacity: f32) -> Self {
        Self {
            transform: transform.into(),
            opacity: if opacity.is_finite() {
                opacity.clamp(0.0, 1.0)
            } else {
                0.0
            },
            _padding: [0.0; 47],
        }
    }

    pub(super) fn layout_entry() -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: Self::binding_size(),
            },
            count: None,
        }
    }

    pub(super) fn binding_size() -> Option<wgpu::BufferSize> {
        wgpu::BufferSize::new(std::mem::size_of::<Self>() as u64)
    }

    pub(super) fn dynamic_offset(index: usize) -> u32 {
        let bytes = index
            .checked_mul(std::mem::size_of::<Self>())
            .expect("triangle uniform offset fits usize");
        u32::try_from(bytes).expect("triangle uniform offset fits WebGPU u32")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_keeps_webgpu_dynamic_offset_stride() {
        assert_eq!(std::mem::size_of::<Uniforms>(), 256);
    }

    #[test]
    fn opacity_is_bounded_before_gpu_upload() {
        assert_eq!(Uniforms::new(Transformation::IDENTITY, 2.0).opacity, 1.0);
        assert_eq!(Uniforms::new(Transformation::IDENTITY, -1.0).opacity, 0.0);
        assert_eq!(
            Uniforms::new(Transformation::IDENTITY, f32::NAN).opacity,
            0.0
        );
    }
}
