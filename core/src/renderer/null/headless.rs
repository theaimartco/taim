use crate::renderer::{self, Headless};
use crate::{Color, Size};

impl Headless for () {
    async fn new(_: renderer::Settings, _: Option<&str>) -> Option<Self> {
        Some(())
    }

    fn name(&self) -> String {
        String::from("null renderer")
    }

    fn screenshot(&mut self, _: Size<u32>, _: f32, _: Color) -> Vec<u8> {
        Vec::new()
    }
}
