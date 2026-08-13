use crate::effects::Graph;

#[derive(Clone)]
pub struct Pipeline {
}

impl Pipeline {
    pub fn new(_device: &wgpu::Device, _format: wgpu::TextureFormat) -> Self {
        Self {
        }
    }

    pub fn prepare(&mut self, _graph: &Graph) {
        // Stub
    }

    pub fn render(&mut self, _encoder: &mut wgpu::CommandEncoder) {
        // Stub implementation for rendering effects
    }

    pub fn trim(&mut self) {
    }
}
